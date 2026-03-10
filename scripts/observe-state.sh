#!/usr/bin/env bash
set -euo pipefail

lines=20
interval=2
watch_mode=0
app_root=""

usage() {
  cat <<'EOF'
Usage: observe-state.sh [--watch] [--lines N] [--interval SECONDS] [--root PATH]

Shows recent desktop log lines and recent session_history rows from the local
Windows app data directory. In --watch mode it refreshes continuously.
EOF
}

discover_root() {
  local candidate
  for candidate in /mnt/c/Users/*/AppData/Local/RdpLaunch; do
    if [[ -d "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

print_snapshot() {
  local log_path db_path now
  log_path="$app_root/logs/app.log"
  db_path="$app_root/rdp-launch.db"
  now="$(date '+%Y-%m-%d %H:%M:%S %Z')"

  printf '== RdpLaunch Smoke Observe ==\n'
  printf 'Time: %s\n' "$now"
  printf 'Root: %s\n\n' "$app_root"

  printf '%s\n' '-- Recent Log Lines --'
  if [[ -f "$log_path" ]]; then
    tail -n "$lines" "$log_path"
  else
    printf 'missing log file: %s\n' "$log_path"
  fi
  printf '\n'

  printf '%s\n' '-- Recent Session History --'
  if [[ -f "$db_path" ]]; then
    sqlite3 -header -column "$db_path" \
      "select launch_id, profile_name, target, process_id, state, started_at, ended_at
       from session_history
       order by started_at desc
       limit ${lines};"
  else
    printf 'missing database file: %s\n' "$db_path"
  fi
  printf '\n'
}

while (($# > 0)); do
  case "$1" in
    --watch)
      watch_mode=1
      shift
      ;;
    --lines)
      lines="${2:?missing value for --lines}"
      shift 2
      ;;
    --interval)
      interval="${2:?missing value for --interval}"
      shift 2
      ;;
    --root)
      app_root="${2:?missing value for --root}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'unknown argument: %s\n\n' "$1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$app_root" ]]; then
  app_root="$(discover_root)" || {
    printf 'could not find a Windows app root under /mnt/c/Users/*/AppData/Local/RdpLaunch\n' >&2
    exit 1
  }
fi

if [[ "$watch_mode" -eq 1 ]]; then
  while true; do
    clear
    print_snapshot
    sleep "$interval"
  done
else
  print_snapshot
fi
