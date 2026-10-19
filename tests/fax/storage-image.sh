#!/usr/bin/env bash
# Run against the server and worker images as their configured service user.
set -euo pipefail
image=${1:?usage: storage-image.sh IMAGE}

# Reproduce an old stack that updates images but omits the new fax mount.
docker run --rm --read-only --entrypoint /bin/sh "$image" -ec '
  test "$(id -u)" = 10001
  test -d /var/lib/talkops/fax
  if probe=$(mktemp /var/lib/talkops/fax/.fax-storage-XXXXXX); then
    rm -f "$probe"
    echo "Expected fax storage without a writable mount to fail" >&2
    exit 1
  fi
'

# The root filesystem stays read-only. Only the fax mount permits writes.
docker run --rm --read-only \
  --tmpfs /var/lib/talkops/fax:uid=10001,gid=10001,mode=0700 \
  --entrypoint /bin/sh "$image" -ec '
    test "$(id -u)" = 10001
    probe=$(mktemp /var/lib/talkops/fax/.fax-storage-XXXXXX)
    trap '\''rm -f "$probe"'\'' EXIT
    printf "fax storage smoke test\n" > "$probe"
    test "$(cat "$probe")" = "fax storage smoke test"
  '
