#!/usr/bin/env bash
# Checks the cuda-stt image without a GPU: sherpa-onnx, ONNX Runtime and its
# CUDA provider resolve all dependencies (except libcuda.so.1 of the host
# driver), cuDNN is installed, and the service starts (mock backend),
# answers /health and passes its own health check.
set -euo pipefail
image="$1"
docker run --rm --entrypoint /bin/sh "$image" -c '
  set -e
  for f in /usr/local/bin/talkops-cuda-stt /usr/local/lib/libonnxruntime_providers_cuda.so; do
    test -e "$f"
    if ldd "$f" | grep "not found" | grep -v libcuda.so.1; then echo "missing dependency of $f" >&2; exit 1; fi
  done
  ldconfig -p | grep -q libcudnn.so.9
  test "$(id -u)" = 10001'
name="cuda-stt-ci-$$"
docker run -d --name "$name" --network host --read-only --tmpfs /tmp \
  -e TALKOPS_CUDA_LISTEN=127.0.0.1:18097 "$image" --backend mock > /dev/null
trap 'docker logs "$name" | tail -20; docker rm -f "$name" > /dev/null' EXIT
for _ in $(seq 1 30); do
  curl -fsS http://127.0.0.1:18097/health > /dev/null 2>&1 && break
  sleep 1
done
curl -fsS http://127.0.0.1:18097/health | grep -q '"status":"ok"'
curl -fsS http://127.0.0.1:18097/v1/models | grep -q cuda-auto
docker exec "$name" talkops-cuda-stt healthcheck
# The CUDA backend without a GPU and without models reports unavailable, but runs.
docker run --rm --network host --read-only --tmpfs /tmp -e TALKOPS_CUDA_LISTEN=127.0.0.1:18098 \
  --entrypoint /bin/sh "$image" -c 'talkops-cuda-stt & pid=$!; sleep 3;
  talkops-cuda-stt --listen 127.0.0.1:18098 healthcheck && exit 1; kill $pid; exit 0'
echo "cuda-stt image checks passed"
