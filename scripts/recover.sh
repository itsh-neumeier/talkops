#!/usr/bin/env bash
# Host recovery for Linux Docker Compose. Never execute files from the archive.
set -euo pipefail
usage() {
  cat <<'HELP'
Usage:
  recover.sh fetch --target-file PATH (--list | --name ARCHIVE --output DIRECTORY)
                   [--trust-host-key] [--image IMAGE]
  recover.sh prepare --archive PATH --output NEW_DIRECTORY [--password-file PATH] [--image IMAGE]
  recover.sh restore (--archive PATH | --backup ARCHIVE) [-f COMPOSE_FILE ...] [--env-file PATH ...]
                     [--project-name NAME] [--profile NAME ...] [--password-file PATH] [--yes]
Fetch lists or downloads archives on a backup target. The target file is JSON
with the connection and its secrets; see the backup guide. Keep it private.
--backup restores an archive in the backups volume, for example an archive
that the setup assistant downloaded from a backup target.
Preparation validates the archive and exports configuration without a database.
Review .env, Compose files, ports, IPs, profiles and volumes before restore.
Restore replaces the database and included data volumes. Keep PostgreSQL running.
The password file contains only the backup password, not KEY=value lines.
HELP
}
mode="${1:-}"; [[ -n "$mode" ]] || { usage; exit 2; }; shift
archive=''; output=''; password_file=''; yes=false
backup=''; target_file=''; name=''; list=false; trust=false
image="${TALKOPS_RECOVERY_IMAGE:-ghcr.io/itsh-neumeier/talkops-server:latest}"
compose=(docker compose)
while (($#)); do
  case "$1" in
    --archive) archive="${2:?}"; shift 2;;
    --backup) backup="${2:?}"; shift 2;;
    --target-file) target_file="${2:?}"; shift 2;;
    --name) name="${2:?}"; shift 2;;
    --list) list=true; shift;;
    --trust-host-key) trust=true; shift;;
    --output) output="${2:?}"; shift 2;;
    --password-file) password_file="${2:?}"; shift 2;;
    --image) image="${2:?}"; shift 2;;
    -f|--file|--env-file|--project-name|--profile) compose+=("$1" "${2:?}"); shift 2;;
    --yes) yes=true; shift;;
    -h|--help) usage; exit 0;;
    *) echo "Unknown option: $1" >&2; exit 2;;
  esac
done
[[ "$mode" == prepare || "$mode" == restore || "$mode" == fetch ]] || { usage; exit 2; }
archive_name_ok() { [[ "$1" =~ ^talkops-[0-9]{8}-[0-9]{6}\.tar\.gz(\.age)?$ ]]; }
if [[ "$mode" == fetch ]]; then
  [[ -f "$target_file" ]] || { echo 'Select an existing target file.' >&2; exit 2; }
  target_file="$(realpath -- "$target_file")"
  args=(fetch --target-file /target.json)
  [[ "$trust" == true ]] && args+=(--trust-host-key)
  run=(docker run --rm --read-only --user "$(id -u):$(id -g)" --tmpfs /tmp -v "$target_file:/target.json:ro")
  if [[ "$list" == true || -z "$name" ]]; then
    "${run[@]}" "$image" "${args[@]}" --list
    exit 0
  fi
  archive_name_ok "$name" || { echo 'Select a TalkOps archive name.' >&2; exit 2; }
  [[ -n "$output" && -d "$output" ]] || { echo 'Choose an existing output directory.' >&2; exit 2; }
  [[ ! -e "$output/$name" ]] || { echo 'The archive exists already in the output directory.' >&2; exit 2; }
  output="$(realpath -- "$output")"
  "${run[@]}" -v "$output:/output" "$image" "${args[@]}" --name "$name" --output /output
  echo "Archive downloaded: $output/$name"
  exit 0
fi
in_volume=''
if [[ -n "$backup" ]]; then
  [[ "$mode" == restore ]] || { echo '--backup is only valid for restore.' >&2; exit 2; }
  [[ -z "$archive" ]] || { echo 'Use --archive or --backup, not both.' >&2; exit 2; }
  archive_name_ok "$backup" || { echo 'Select a TalkOps archive name.' >&2; exit 2; }
  in_volume="/var/lib/talkops/backups/$backup"
else
  [[ -f "$archive" ]] || { echo 'Select an existing backup archive.' >&2; exit 2; }
  archive="$(realpath -- "$archive")"
fi
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
if [[ -n "$in_volume" ]]; then
  source_args=()
  restore_path="$in_volume"
  "${compose[@]}" run --rm --no-deps -v "$scratch:/tmp" "${password_args[@]}" talkops inspect "$in_volume"
else
  source_args=(-v "$archive:/restore.archive:ro")
  restore_path=/restore.archive
  "${inspect[@]}" "$image" inspect /restore.archive
fi
if [[ "$yes" != true ]]; then
  read -r -p 'Replace the selected database and included volumes? Type RESTORE: ' confirmation
  [[ "$confirmation" == RESTORE ]] || { echo 'Restore cancelled.'; exit 1; }
fi
"${compose[@]}" stop freeswitch talkops media-worker
"${compose[@]}" up -d --wait postgres
if ! "${compose[@]}" run --rm --no-deps "${source_args[@]}" -v "$scratch:/tmp" "${password_args[@]}" talkops restore --yes "$restore_path"; then
  echo 'Restore failed. Writers stay stopped. Read backups/restore-state.json and correct the failure before retry.' >&2
  exit 1
fi
"${compose[@]}" up -d --wait
echo 'Restore complete. Sign in with a restored account, then check service health and phone registration.'
