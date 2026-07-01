#!/usr/bin/env sh
set -u

runtime="unknown"
event="unknown"
matcher=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --runtime|--vendor)
      runtime="${2:-unknown}"
      shift 2
      ;;
    --event)
      event="${2:-unknown}"
      shift 2
      ;;
    --matcher)
      matcher="${2:-}"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

if [ -n "${MEGARA_STATE_DIR:-}" ]; then
  state_dir="$MEGARA_STATE_DIR"
elif [ -d "$PWD/.agents" ]; then
  state_dir="$PWD/.agents/state/hooks"
else
  state_dir="${HOME:-$PWD}/.megara/state/hooks"
fi

mkdir -p "$state_dir" 2>/dev/null || exit 0

payload="$(cat 2>/dev/null || true)"
payload_file="$state_dir/last-$runtime-$event.json"
printf '%s' "$payload" > "$payload_file" 2>/dev/null || true

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

timestamp="$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date)"
payload_bytes="$(printf '%s' "$payload" | wc -c | tr -d ' ')"
log_file="$state_dir/events.jsonl"

printf '{"timestamp":"%s","runtime":"%s","event":"%s","matcher":"%s","payload":"%s","payload_bytes":%s}\n' \
  "$(json_escape "$timestamp")" \
  "$(json_escape "$runtime")" \
  "$(json_escape "$event")" \
  "$(json_escape "$matcher")" \
  "$(json_escape "$payload_file")" \
  "$payload_bytes" >> "$log_file" 2>/dev/null || true

if [ -n "${MEGARA_HOOK_COMMAND:-}" ]; then
  MEGARA_RUNTIME="$runtime" \
  MEGARA_EVENT="$event" \
  MEGARA_MATCHER="$matcher" \
  MEGARA_HOOK_PAYLOAD="$payload_file" \
  sh -c "$MEGARA_HOOK_COMMAND"
  status="$?"
  if [ "$status" -ne 0 ]; then
    printf '{"timestamp":"%s","runtime":"%s","event":"%s","error":"command_failed","status":%s}\n' \
      "$(json_escape "$timestamp")" \
      "$(json_escape "$runtime")" \
      "$(json_escape "$event")" \
      "$status" >> "$log_file" 2>/dev/null || true
    if [ "${MEGARA_HOOK_STRICT:-0}" = "1" ]; then
      exit "$status"
    fi
  fi
fi

exit 0
