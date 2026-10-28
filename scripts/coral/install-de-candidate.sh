#!/bin/sh
# Install the separately supplied 2026-10-25 German candidate on a Docker host.
# Does not download weights or change any other TalkOps service.
set -eu

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
    echo "Usage: sh install-de-candidate.sh PACKAGE.tar.gz [CORAL_CONTAINER]" >&2
    exit 1
fi
archive=$(realpath "$1")
expected=d8f2924d6c826bd0d449b981084315195c18a2cd74f59524e97a186ff1133423
actual=$(sha256sum "$archive" | cut -d ' ' -f 1)
if [ "$actual" != "$expected" ]; then
    echo "Package checksum mismatch. No files were changed." >&2
    exit 1
fi
container=${2:-}
if [ -z "$container" ]; then
    candidates=$(docker ps -q --filter label=com.docker.compose.service=coral-stt)
    # Container IDs cannot contain shell metacharacters or whitespace.
    set -- $candidates
    if [ "$#" -ne 1 ]; then
        echo "Expected one running coral-stt container. Pass its name as the second argument." >&2
        exit 1
    fi
    container=$1
fi
volume=$(docker inspect --format '{{range .Mounts}}{{if eq .Destination "/var/lib/talkops/models"}}{{if eq .Type "volume"}}{{.Name}}{{end}}{{end}}{{end}}' "$container")
if [ -z "$volume" ]; then
    echo "No named models volume at /var/lib/talkops/models. Install the files in the custom mount instead." >&2
    exit 1
fi
configured=$(docker inspect --format '{{range .Config.Env}}{{if eq (index (split . "=") 0) "TALKOPS_CORAL_MODELS_DIR"}}{{println .}}{{end}}{{end}}' "$container")
case "$configured" in
    ''|TALKOPS_CORAL_MODELS_DIR=/var/lib/talkops/models/coral) ;;
    *) echo "Custom model directory: $configured. No files were changed." >&2; exit 1 ;;
esac
docker volume inspect "$volume" >/dev/null
image=$(docker inspect --format '{{.Config.Image}}' "$container")
printf 'Coral container: %s\nModels volume: %s\n' "$container" "$volume"

docker run --rm --network none --read-only --user 0 --entrypoint /bin/sh \
    --mount "type=volume,source=$volume,target=/models" \
    --mount "type=bind,source=$archive,target=/input/package.tar.gz,readonly" \
    "$image" -c '
set -eu
for path in /models/coral /models/coral/de /models/coral/de/model_edgetpu.tflite /models/coral/de/model.json; do
    if [ -L "$path" ]; then echo "Refusing a symbolic model path: $path" >&2; exit 1; fi
done
if [ -e /models/coral/de/model_edgetpu.tflite ] || [ -e /models/coral/de/model.json ]; then
    backup="/models/coral/de-backup-$(date -u +%Y%m%dT%H%M%SZ)-$$"
    mkdir -p "$backup"
    for name in model_edgetpu.tflite model.json; do
        if [ -f "/models/coral/de/$name" ]; then cp -p "/models/coral/de/$name" "$backup/"; fi
    done
    printf "Previous model files saved in %s\n" "$backup"
fi
mkdir -p /models/coral/de
tar -xzf /input/package.tar.gz -C /models coral/de/model_edgetpu.tflite coral/de/model.json
chown 10001:10001 /models/coral /models/coral/de /models/coral/de/model_edgetpu.tflite /models/coral/de/model.json
chmod 644 /models/coral/de/model_edgetpu.tflite /models/coral/de/model.json
printf "Installed German model files.\n"
'
docker restart "$container" >/dev/null
attempt=0
while [ "$attempt" -lt 20 ]; do
    if docker exec "$container" /usr/local/bin/talkops-coral-stt healthcheck >/dev/null 2>&1; then
        echo "Coral is healthy. Select coral-ctc-de and test the connection in TalkOps."
        exit 0
    fi
    attempt=$((attempt + 1))
    sleep 1
done
echo "Model files are installed, but Coral is not healthy yet. Download the Coral diagnostic report or check its container logs." >&2
exit 1
