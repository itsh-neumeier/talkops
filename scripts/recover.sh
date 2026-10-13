#!/usr/bin/env bash
# Host recovery for Linux Docker Compose. Never execute files from the archive.
set -euo pipefail
usage() {
  cat <<'HELP'
Usage:
  recover.sh prepare --archive PATH --output NEW_DIRECTORY [--password-file PATH] [--image IMAGE]
  recover.sh restore --archive PATH [-f COMPOSE_FILE ...] [--env-file PATH ...]
                     [--project-name NAME] [--profile NAME ...] [--password-file PATH] [--yes]
Preparation validates the archive and exports configuration without a database.
Review .env, Compose files, ports, IPs, profiles and volumes before restore.
Restore replaces the database and included data volumes. Keep PostgreSQL running.
The password file contains only the backup password, not KEY=value lines.
HELP
}
mode="${1:-}"; [[ -n "$mode" ]] || { usage; exit 2; }; shift
archive=''; output=''; password_file=''; yes=false
image="${TALKOPS_RECOVERY_IMAGE:-ghcr.io/itsh-neumeier/talkops-server:latest}"
compose=(docker compose)
while (($#)); do
  case "$1" in
    --archive) archive="${2:?}"; shift 2;;
    --output) output="${2:?}"; shift 2;;
    --password-file) password_file="${2:?}"; shift 2;;
    --image) image="${2:?}"; shift 2;;
    -f|--file|--env-file|--project-name|--profile) compose+=("$1" "${2:?}"); shift 2;;
    --yes) yes=true; shift;;
    -h|--help) usage; exit 0;;
    *) echo "Unknown option: $1" >&2; exit 2;;
  esac
done
[[ "$mode" == prepare || "$mode" == restore ]] || { usage; exit 2; }
[[ -f "$archive" ]] || { echo 'Select an existing backup archive.' >&2; exit 2; }
archive="$(realpath -- "$archive")"
password_args=()
if [[ -n "$password_file" ]]; then
  [[ -f "$password_file" ]] || { echo 'Password file not found.' >&2; exit 2; }
  password_file="$(realpath -- "$password_file")"
  export TALKOPS_BACKUP_PASSWORD="$(< "$password_file")"
  password_args=(-e TALKOPS_BACKUP_PASSWORD)
fi
scratch="$(mktemp -d "${TMPDIR:-/tmp}/talkops-recovery.XXXXXXXX")"
chmod 1777 "$scratch"
cleanup() { rm -rf -- "$scratch"; }
trap cleanup EXIT
inspect=(docker run --rm --read-only --user "$(id -u):$(id -g)" -v "$scratch:/tmp" -v "$archive:/restore.archive:ro" "${password_args[@]}")
if [[ "$mode" == prepare ]]; then
  [[ -n "$output" && ! -e "$output" ]] || { echo 'Choose a new output directory.' >&2; exit 2; }
  parent="$(realpath -- "$(dirname -- "$output")")"; name="$(basename -- "$output")"
  [[ "$name" != . && "$name" != .. ]] || exit 2
  "${inspect[@]}" -v "$parent:/output" "$image" inspect /restore.archive --output "/output/$name"
  echo 'Configuration exported. Review it before starting PostgreSQL or restoring.'
  echo 'Use -f, --env-file, --project-name and --profile to select the reviewed stack.'
  exit 0
fi
[[ -z "$output" ]] || { echo '--output is only valid for prepare.' >&2; exit 2; }
"${compose[@]}" config --quiet
"${inspect[@]}" "$image" inspect /restore.archive
if [[ "$yes" != true ]]; then
  read -r -p 'Replace the selected database and included volumes? Type RESTORE: ' confirmation
  [[ "$confirmation" == RESTORE ]] || { echo 'Restore cancelled.'; exit 1; }
fi
"${compose[@]}" stop freeswitch talkops media-worker
"${compose[@]}" up -d --wait postgres
if ! "${compose[@]}" run --rm --no-deps -v "$archive:/restore.archive:ro" -v "$scratch:/tmp" "${password_args[@]}" talkops restore --yes /restore.archive; then
  echo 'Restore failed. Writers stay stopped. Read backups/restore-state.json and correct the failure before retry.' >&2
  exit 1
fi
"${compose[@]}" up -d --wait
echo 'Restore complete. Sign in with a restored account, then check service health and phone registration.'
