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

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

path_part() {
  printf '%s' "$1" | sed 's/[^A-Za-z0-9_.-]/_/g'
}

timestamp="$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date)"
file_timestamp="$(date -u +%Y%m%dT%H%M%SZ 2>/dev/null || date +%s)"
payload_bytes="$(printf '%s' "$payload" | wc -c | tr -d ' ')"
log_file="$state_dir/events.jsonl"
safe_runtime="$(path_part "$runtime")"
safe_event="$(path_part "$event")"
payload_dir="$state_dir/payloads/$safe_runtime/$safe_event"
mkdir -p "$payload_dir" 2>/dev/null || true

payload_file="$payload_dir/$file_timestamp-$$.json"
suffix=0
while [ -e "$payload_file" ]; do
  suffix=$((suffix + 1))
  payload_file="$payload_dir/$file_timestamp-$$-$suffix.json"
done

printf '%s' "$payload" > "$payload_file" 2>/dev/null || true

last_payload_file="$state_dir/last-$safe_runtime-$safe_event.json"
printf '%s' "$payload" > "$last_payload_file" 2>/dev/null || true

printf '{"timestamp":"%s","runtime":"%s","event":"%s","matcher":"%s","payload":"%s","last_payload":"%s","payload_bytes":%s}\n' \
  "$(json_escape "$timestamp")" \
  "$(json_escape "$runtime")" \
  "$(json_escape "$event")" \
  "$(json_escape "$matcher")" \
  "$(json_escape "$payload_file")" \
  "$(json_escape "$last_payload_file")" \
  "$payload_bytes" >> "$log_file" 2>/dev/null || true

conversation_role=""
case "$event" in
  UserPromptSubmit)
    conversation_role="user"
    ;;
  Stop)
    conversation_role="assistant"
    ;;
esac

if [ -n "$conversation_role" ]; then
  conversation_events="$state_dir/conversation-events.jsonl"
  printf '{"timestamp":"%s","runtime":"%s","event":"%s","role":"%s","payload":"%s","payload_bytes":%s}\n' \
    "$(json_escape "$timestamp")" \
    "$(json_escape "$runtime")" \
    "$(json_escape "$event")" \
    "$(json_escape "$conversation_role")" \
    "$(json_escape "$payload_file")" \
    "$payload_bytes" >> "$conversation_events" 2>/dev/null || true

  if command -v python3 >/dev/null 2>&1; then
    conversation_log="$state_dir/conversation.jsonl"
    python3 - "$conversation_log" "$timestamp" "$runtime" "$event" "$conversation_role" "$payload_file" <<'PY' 2>/dev/null || true
import json
import sys

log_path, timestamp, runtime, event, role, payload_path = sys.argv[1:]

try:
    with open(payload_path, "r", encoding="utf-8") as source:
        payload = json.load(source)
except Exception:
    sys.exit(0)

field = "prompt" if role == "user" else "last_assistant_message"
content = payload.get(field)
if not isinstance(content, str) or not content.strip():
    sys.exit(0)

entry = {
    "timestamp": timestamp,
    "runtime": runtime,
    "event": event,
    "role": role,
    "content": content,
    "payload": payload_path,
}

for key in ("session_id", "turn_id", "transcript_path", "cwd", "model"):
    value = payload.get(key)
    if value is not None:
        entry[key] = value

with open(log_path, "a", encoding="utf-8") as sink:
    sink.write(json.dumps(entry, ensure_ascii=False, separators=(",", ":")) + "\n")
PY
  fi
fi

if command -v python3 >/dev/null 2>&1; then
  python3 - "$state_dir" "$timestamp" "$runtime" "$event" "$matcher" "$payload_file" <<'PY'
import hashlib
import json
import os
import re
import sys
from pathlib import Path

state_dir, timestamp, runtime, event, matcher, payload_path = sys.argv[1:]


def safe_part(value):
    value = str(value or "unknown").strip() or "unknown"
    return re.sub(r"[^A-Za-z0-9_.-]", "_", value)


def read_payload(path):
    try:
        with open(path, "r", encoding="utf-8") as source:
            return json.load(source)
    except Exception:
        return {}


def append_jsonl(path, entry):
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "a", encoding="utf-8") as sink:
        sink.write(json.dumps(entry, ensure_ascii=False, separators=(",", ":")) + "\n")


def load_json(path, fallback):
    try:
        with open(path, "r", encoding="utf-8") as source:
            value = json.load(source)
        return value if isinstance(value, dict) else fallback
    except Exception:
        return fallback


def write_json_atomic(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + f".{os.getpid()}.tmp")
    with open(tmp, "w", encoding="utf-8") as sink:
        json.dump(value, sink, ensure_ascii=False, indent=2, sort_keys=True)
        sink.write("\n")
    os.replace(tmp, path)


def write_text_atomic(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + f".{os.getpid()}.tmp")
    with open(tmp, "w", encoding="utf-8") as sink:
        sink.write(value)
    os.replace(tmp, path)


def workflow_dir_from_hooks_dir(hooks_dir):
    normalized = Path(hooks_dir)
    if normalized.name == "hooks":
        return normalized.parent / "workflows" / "deep-interview"
    return normalized / "workflows" / "deep-interview"


def parse_block(text, marker):
    if not isinstance(text, str) or marker not in text:
        return None
    lines = text.splitlines()
    start = None
    for index, line in enumerate(lines):
        if line.strip() == marker:
            start = index + 1
            break
    if start is None:
        return None

    fields = {"options": []}
    current_key = None
    saw_field = False
    for raw in lines[start:]:
        if not raw.strip():
            if saw_field:
                break
            continue
        if raw.startswith("  - ") or raw.startswith("    - "):
            if current_key == "options":
                fields.setdefault("options", []).append(raw.split("- ", 1)[1].strip())
            continue
        stripped = raw.strip()
        if not stripped.startswith("- "):
            if saw_field:
                break
            continue
        body = stripped[2:]
        if ":" not in body:
            continue
        key, value = body.split(":", 1)
        key = key.strip().lower().replace("-", "_")
        value = value.strip()
        current_key = key
        saw_field = True
        if key == "options":
            fields["options"] = []
        else:
            fields[key] = value
    return fields if saw_field else None


def parse_bool(value):
    return str(value).strip().lower() in {"1", "true", "yes", "y", "on"}


def normalize_round(value):
    try:
        return int(str(value).strip())
    except Exception:
        return value


def question_from_text(text, payload_path):
    block = parse_block(text, "Megara Question Gate:")
    if not block:
        return None
    question_id = str(block.get("id", "")).strip()
    question = str(block.get("question", "")).strip()
    if not question_id or not question:
        return None
    return {
        "id": question_id,
        "round": normalize_round(block.get("round")),
        "component": str(block.get("component", "")).strip(),
        "dimension": str(block.get("dimension", "")).strip(),
        "question": question,
        "options": [option for option in block.get("options", []) if option],
        "free_text": parse_bool(block.get("free_text", "false")),
        "status": "pending",
        "asked_at": timestamp,
        "payload": payload_path,
    }


def workflow_state_from_text(text):
    block = parse_block(text, "Megara Workflow State:")
    if not block:
        return None
    if str(block.get("skill", "")).strip() != "deep-interview":
        return None
    status = str(block.get("status", "")).strip().lower()
    if not status:
        return None
    return {
        "status": status,
        "ambiguity": str(block.get("ambiguity", "")).strip(),
        "next": str(block.get("next", "")).strip(),
    }


def yaml_string(value):
    return json.dumps(str(value or ""), ensure_ascii=False)


def text_before_block(text, marker):
    if not isinstance(text, str):
        return ""
    lines = text.splitlines()
    for index, line in enumerate(lines):
        if line.strip() == marker:
            return "\n".join(lines[:index]).strip()
    return text.strip()


def unique_spec_path(workflow_dir, session_id):
    specs_dir = workflow_dir / "specs"
    base = specs_dir / f"deep-interview-{safe_part(session_id)}-{safe_part(timestamp)}"
    path = base.with_suffix(".md")
    suffix = 0
    while path.exists():
        suffix += 1
        path = specs_dir / f"{base.name}-{suffix}.md"
    return path


def persist_crystallized_spec(workflow_dir, session_id, terminal, text):
    if terminal.get("status") != "crystallized":
        return None
    if not isinstance(text, str) or not text.strip():
        return None
    if not text_before_block(text, "Megara Workflow State:"):
        return None

    body = text.strip() + "\n"
    content = "\n".join([
        "---",
        'skill: "deep-interview"',
        f"session_id: {yaml_string(session_id)}",
        'status: "crystallized"',
        f"ambiguity: {yaml_string(terminal.get('ambiguity'))}",
        f"next: {yaml_string(terminal.get('next'))}",
        f"persisted_at: {yaml_string(timestamp)}",
        f"payload: {yaml_string(payload_path)}",
        "---",
        "",
        body,
    ])
    if not content.endswith("\n"):
        content += "\n"

    spec_path = unique_spec_path(workflow_dir, session_id)
    write_text_atomic(spec_path, content)
    sha256 = hashlib.sha256(content.encode("utf-8")).hexdigest()
    metadata = {
        "path": str(spec_path),
        "sha256": sha256,
        "persisted_at": timestamp,
        "payload": payload_path,
    }
    append_jsonl(workflow_dir / "specs" / "index.jsonl", {
        "timestamp": timestamp,
        "event": "spec_persisted",
        "session_id": session_id,
        "skill": "deep-interview",
        "status": "crystallized",
        "path": str(spec_path),
        "sha256": sha256,
        "payload": payload_path,
    })
    return metadata


def new_state(session_id, payload):
    return {
        "version": 1,
        "skill": "deep-interview",
        "session_id": session_id,
        "cwd": payload.get("cwd"),
        "active": True,
        "phase": "initialized",
        "pending_question": None,
        "questions": [],
        "updated_at": timestamp,
    }


def session_paths(payload):
    session_id = (
        payload.get("session_id")
        or payload.get("thread_id")
        or payload.get("turn_id")
        or "unknown-session"
    )
    session_id = str(session_id)
    workflow_dir = workflow_dir_from_hooks_dir(state_dir)
    return (
        session_id,
        workflow_dir,
        workflow_dir / f"{safe_part(session_id)}.json",
        workflow_dir / "events.jsonl",
    )


def upsert_question(state, question):
    pending = state.get("pending_question")
    if isinstance(pending, dict) and pending.get("status") == "pending":
        for existing in state.get("questions", []):
            if existing.get("id") == pending.get("id") and existing.get("status") == "pending":
                existing["status"] = "superseded"
                existing["superseded_at"] = timestamp
                break
    questions = [q for q in state.get("questions", []) if q.get("id") != question["id"]]
    questions.append(question)
    state["questions"] = questions
    state["pending_question"] = question
    state["active"] = True
    state["phase"] = "question_pending"
    state["updated_at"] = timestamp


def answer_pending_question(state, prompt):
    pending = state.get("pending_question")
    if not isinstance(pending, dict) or pending.get("status") != "pending":
        return None
    answer = {
        "content": prompt,
        "answered_at": timestamp,
        "payload": payload_path,
    }
    pending_id = pending.get("id")
    for existing in state.get("questions", []):
        if existing.get("id") == pending_id and existing.get("status") == "pending":
            existing["status"] = "answered"
            existing["answer"] = answer
            break
    state["pending_question"] = None
    state["phase"] = "interviewing"
    state["updated_at"] = timestamp
    return {"question_id": pending_id, "answer": answer}


def update_terminal_state(state, terminal, spec=None):
    status = terminal["status"]
    terminal_statuses = {"crystallized", "cancelled", "canceled", "complete", "completed"}
    state["active"] = status not in terminal_statuses
    state["phase"] = status
    state["status"] = status
    if terminal.get("ambiguity"):
        state["ambiguity"] = terminal["ambiguity"]
    if terminal.get("next"):
        state["next"] = terminal["next"]
    if spec:
        state["spec_path"] = spec["path"]
        state["spec_sha256"] = spec["sha256"]
        state["spec_persisted_at"] = spec["persisted_at"]
        state["spec_payload"] = spec["payload"]
    if not state["active"]:
        state["pending_question"] = None
        state["closed_at"] = timestamp
    state["updated_at"] = timestamp


def reject_crystallized_without_spec(state):
    state["active"] = True
    state["phase"] = "crystallization_missing_spec"
    state["status"] = "crystallization_missing_spec"
    state["pending_question"] = None
    state["updated_at"] = timestamp


MUTATION_PATTERNS = [
    r"(^|[;&|]\s*)apply_patch\b",
    r"(^|[;&|]\s*)(rm|mv|cp|mkdir|touch|chmod|chown|ln|install)\b",
    r"(^|[;&|]\s*)git\s+(add|commit|push|tag|checkout|switch|reset|merge|rebase|restore)\b",
    r"(^|[;&|]\s*)(npm|pnpm|yarn|bun)\s+(install|add|remove|update)\b",
    r"(^|[;&|]\s*)cargo\s+fmt\b",
    r"(^|[;&|]\s*)sed\s+-i\b",
    r"(^|[;&|]\s*)perl\s+-pi\b",
    r"(^|[;&|]\s*)tee\b",
    r">>",
    r"(^|[^0-9])>(?!&)",
]


def mutating_command(command):
    if not isinstance(command, str) or not command.strip():
        return False
    return any(re.search(pattern, command) for pattern in MUTATION_PATTERNS)


def current_active_state(session_file):
    state = load_json(session_file, None)
    if not isinstance(state, dict):
        return None
    if state.get("skill") != "deep-interview":
        return None
    if state.get("active") is not True:
        return None
    return state


payload = read_payload(payload_path)
session_id, workflow_dir, session_file, events_file = session_paths(payload)

if event == "Stop":
    text = payload.get("last_assistant_message")
    terminal = workflow_state_from_text(text)
    question = question_from_text(text, payload_path)
    if terminal or question:
        state = load_json(session_file, new_state(session_id, payload))
        if terminal:
            spec = persist_crystallized_spec(workflow_dir, session_id, terminal, text)
            if terminal["status"] == "crystallized" and not spec:
                reject_crystallized_without_spec(state)
                append_jsonl(events_file, {
                    "timestamp": timestamp,
                    "event": "spec_missing",
                    "session_id": session_id,
                    "status": terminal["status"],
                    "payload": payload_path,
                })
            else:
                update_terminal_state(state, terminal, spec)
                event_entry = {
                    "timestamp": timestamp,
                    "event": "workflow_state",
                    "session_id": session_id,
                    "status": terminal["status"],
                    "payload": payload_path,
                }
                if spec:
                    event_entry["spec_path"] = spec["path"]
                    event_entry["spec_sha256"] = spec["sha256"]
                    append_jsonl(events_file, {
                        "timestamp": timestamp,
                        "event": "spec_persisted",
                        "session_id": session_id,
                        "path": spec["path"],
                        "sha256": spec["sha256"],
                        "payload": payload_path,
                    })
                append_jsonl(events_file, event_entry)
        if question:
            upsert_question(state, question)
            append_jsonl(events_file, {
                "timestamp": timestamp,
                "event": "question_pending",
                "session_id": session_id,
                "question_id": question["id"],
                "round": question.get("round"),
                "component": question.get("component"),
                "dimension": question.get("dimension"),
                "payload": payload_path,
            })
        write_json_atomic(session_file, state)

elif event == "UserPromptSubmit":
    prompt = payload.get("prompt")
    if isinstance(prompt, str) and prompt.strip():
        state = load_json(session_file, None)
        if isinstance(state, dict):
            answered = answer_pending_question(state, prompt)
            if answered:
                write_json_atomic(session_file, state)
                append_jsonl(events_file, {
                    "timestamp": timestamp,
                    "event": "question_answered",
                    "session_id": session_id,
                    "question_id": answered["question_id"],
                    "payload": payload_path,
                })

elif event == "PreToolUse" and os.environ.get("MEGARA_MUTATION_GUARD", "block") != "off":
    state = current_active_state(session_file)
    command = ""
    tool_input = payload.get("tool_input")
    if isinstance(tool_input, dict):
        command = tool_input.get("command") or ""
    if state and mutating_command(command):
        append_jsonl(events_file, {
            "timestamp": timestamp,
            "event": "mutation_blocked",
            "session_id": session_id,
            "phase": state.get("phase"),
            "command": command,
            "payload": payload_path,
        })
        message = (
            "MEGARA mutation guard: deep-interview is active. "
            "Answer the pending question or crystallize/cancel the interview before mutating files."
        )
        print(message, file=sys.stderr)
        if os.environ.get("MEGARA_MUTATION_GUARD", "block") != "warn":
            sys.exit(42)
PY
  workflow_status="$?"
  if [ "$workflow_status" -ne 0 ]; then
    exit "$workflow_status"
  fi
fi

if [ -n "${MEGARA_HOOK_COMMAND:-}" ]; then
  MEGARA_RUNTIME="$runtime" \
  MEGARA_EVENT="$event" \
  MEGARA_MATCHER="$matcher" \
  MEGARA_HOOK_PAYLOAD="$payload_file" \
  MEGARA_HOOK_LAST_PAYLOAD="$last_payload_file" \
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
