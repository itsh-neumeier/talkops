#!/usr/bin/env bash
# Test command selection, confirmation and failure behavior without Docker.
set -euo pipefail
root="$(cd -- "$(dirname -- "$0")/../.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf -- "$tmp"' EXIT
export MOCK_LOG="$tmp/docker.log"
mkdir "$tmp/bin"
cat > "$tmp/bin/docker" <<'MOCK'
#!/usr/bin/env bash
printf '<%s>' "$@" >> "$MOCK_LOG"
printf '\n' >> "$MOCK_LOG"
if [[ " $* " == *" talkops restore --yes "* && "${MOCK_FAIL:-0}" == 1 ]]; then exit 7; fi
MOCK
chmod +x "$tmp/bin/docker"
export PATH="$tmp/bin:$PATH"
touch "$tmp/backup with spaces.age"
printf '%s\n' 'never-print-this-test-password' > "$tmp/password"
: > "$MOCK_LOG"
bash "$root/scripts/recover.sh" prepare --archive "$tmp/backup with spaces.age" --password-file "$tmp/password" --output "$tmp/new output" --image test-image >/dev/null
log="$(< "$MOCK_LOG")"
[[ "$log" == *'<test-image><inspect></restore.archive><--output></output/new output>'* ]]
[[ "$log" != *'never-print-this-test-password'* ]]

: > "$MOCK_LOG"
if printf 'CANCEL\n' | bash "$root/scripts/recover.sh" restore --archive "$tmp/backup with spaces.age" >/dev/null; then exit 1; fi
log="$(< "$MOCK_LOG")"
[[ "$log" != *'<stop>'* ]]

: > "$MOCK_LOG"
bash "$root/scripts/recover.sh" restore --archive "$tmp/backup with spaces.age" --password-file "$tmp/password" -f 'compose file.yml' --env-file 'custom env' --project-name custom --profile caddy --yes >/dev/null
log="$(< "$MOCK_LOG")"
[[ "$log" == *'<-f><compose file.yml><--env-file><custom env><--project-name><custom><--profile><caddy><stop><freeswitch><talkops><media-worker>'* ]]
[[ "$log" == *'<talkops><restore><--yes></restore.archive>'* ]]
[[ "$log" == *'<up><-d><--wait>'* ]]
[[ "$log" != *'never-print-this-test-password'* ]]

: > "$MOCK_LOG"
export MOCK_FAIL=1
if bash "$root/scripts/recover.sh" restore --archive "$tmp/backup with spaces.age" --yes >/dev/null 2>&1; then exit 1; fi
log="$(< "$MOCK_LOG")"
[[ "$log" == *'<up><-d><--wait><postgres>'* ]]
[[ "$log" != *'<up><-d><--wait>'$'\n'* ]]
unset MOCK_FAIL

# Backup targets: list and download with a private target file.
printf '%s\n' '{"config":{"kind":"webdav","url":"https://dav.example","vendor":"other","user":"u"},"secret":{"password":"never-print-this-target-password"}}' > "$tmp/target file.json"
: > "$MOCK_LOG"
bash "$root/scripts/recover.sh" fetch --target-file "$tmp/target file.json" --list --image test-image >/dev/null
log="$(< "$MOCK_LOG")"
[[ "$log" == *"<$tmp/target file.json:/target.json:ro><test-image><fetch><--target-file></target.json><--list>"* ]]
[[ "$log" != *'never-print-this-target-password'* ]]
mkdir "$tmp/downloads"
: > "$MOCK_LOG"
bash "$root/scripts/recover.sh" fetch --target-file "$tmp/target file.json" --name talkops-20261015-033000.tar.gz.age --output "$tmp/downloads" --image test-image >/dev/null
log="$(< "$MOCK_LOG")"
[[ "$log" == *"<$tmp/downloads:/output><test-image><fetch><--target-file></target.json><--name><talkops-20261015-033000.tar.gz.age><--output></output>"* ]]
if bash "$root/scripts/recover.sh" fetch --target-file "$tmp/target file.json" --name '../x' --output "$tmp/downloads" >/dev/null 2>&1; then exit 1; fi

# Restore of an archive in the backups volume (downloaded by the setup assistant).
: > "$MOCK_LOG"
bash "$root/scripts/recover.sh" restore --backup talkops-20261015-033000.tar.gz.age --password-file "$tmp/password" --yes >/dev/null
log="$(< "$MOCK_LOG")"
[[ "$log" == *'<talkops><inspect></var/lib/talkops/backups/talkops-20261015-033000.tar.gz.age>'* ]]
[[ "$log" == *'<talkops><restore><--yes></var/lib/talkops/backups/talkops-20261015-033000.tar.gz.age>'* ]]
[[ "$log" != *'/restore.archive'* ]]
[[ "$log" != *'never-print-this-test-password'* ]]
if bash "$root/scripts/recover.sh" restore --backup '../../etc/passwd' --yes >/dev/null 2>&1; then exit 1; fi
echo 'Passed: export, confirmation, Compose options, secret handling, failed-restore service state, target fetch and volume restore'
