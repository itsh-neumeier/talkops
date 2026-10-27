#!/usr/bin/env bash
# Checks the coral-stt image without an Edge TPU: the TensorFlow Lite and
# libedgetpu libraries resolve all their dependencies, and the service
# starts (mock backend), answers /health and passes its own health check.
set -euo pipefail
image="$1"
docker run --rm --entrypoint /bin/sh "$image" -c '
  set -e
  for lib in /usr/local/lib/libtensorflowlite_c.so /usr/local/lib/libedgetpu.so.1; do
    test -e "$lib"
    if ldd "$lib" | grep "not found"; then echo "missing dependency of $lib" >&2; exit 1; fi
  done
  test "$(id -u)" = 10001'
name="coral-stt-ci-$$"
docker run -d --name "$name" --network host --read-only --tmpfs /tmp \
  -e TALKOPS_CORAL_LISTEN=127.0.0.1:18095 "$image" --backend mock > /dev/null
trap 'docker logs "$name" | tail -20; docker rm -f "$name" > /dev/null' EXIT
for _ in $(seq 1 30); do
  curl -fsS http://127.0.0.1:18095/health > /dev/null 2>&1 && break
  sleep 1
done
curl -fsS http://127.0.0.1:18095/health | grep -q '"status":"ok"'
curl -fsS http://127.0.0.1:18095/v1/models | grep -q coral-auto
docker exec "$name" talkops-coral-stt healthcheck
# The TPU backend without a device reports unavailable, but runs.
docker run --rm --network host --read-only --tmpfs /tmp -e TALKOPS_CORAL_LISTEN=127.0.0.1:18096 \
  --entrypoint /bin/sh "$image" -c 'talkops-coral-stt & pid=$!; sleep 3;
  talkops-coral-stt --listen 127.0.0.1:18096 healthcheck && exit 1; kill $pid; exit 0'
echo "coral-stt image checks passed"
