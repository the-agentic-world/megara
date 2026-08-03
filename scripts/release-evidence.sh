#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

release_commit="${RELEASE_COMMIT:-${GITHUB_SHA:-$(git rev-parse HEAD)}}"
head_commit="$(git rev-parse HEAD)"
if [[ ! "$release_commit" =~ ^[0-9a-f]{40}$ ]]; then
  echo "RELEASE_COMMIT must be a 40-character lowercase commit SHA" >&2
  exit 2
fi
if [[ "$head_commit" != "$release_commit" ]]; then
  echo "release commit mismatch: HEAD=$head_commit RELEASE_COMMIT=$release_commit" >&2
  exit 2
fi

status_before="$(git status --porcelain --untracked-files=all)"
if [[ -n "$status_before" ]]; then
  echo "release evidence requires a clean worktree before staging: $status_before" >&2
  exit 2
fi

evidence_root="${MEGARA_EVIDENCE_ROOT:-$repo_root/target/megara-evidence/$release_commit}"
if [[ -e "$evidence_root" ]]; then
  echo "refusing to overwrite existing evidence staging directory: $evidence_root" >&2
  exit 2
fi
mkdir -p "$evidence_root/commands" "$evidence_root/traces" "$evidence_root/metadata"

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

run_logged() {
  local name="$1"
  shift
  local stdout="$evidence_root/commands/${name}.stdout"
  local stderr="$evidence_root/commands/${name}.stderr"
  local command_file="$evidence_root/commands/${name}.command"
  local status

  : > "$command_file"
  printf '%q ' "$@" >> "$command_file"
  printf '\n' >> "$command_file"
  set +e
  "$@" >"$stdout" 2>"$stderr"
  status=$?
  set -e
  printf '%s\t%s\n' "$name" "$status" >> "$evidence_root/commands/exit-codes.tsv"
  if [[ "$status" -ne 0 ]]; then
    echo "release evidence command failed: $name (exit $status)" >&2
    return "$status"
  fi
}

run_trace() {
  local name="$1"
  shift
  run_logged "$name" "$@"
  cat "$evidence_root/commands/${name}.stdout" \
    "$evidence_root/commands/${name}.stderr" >"$evidence_root/traces/${name}.trace"
}

run_logged git-head git rev-parse HEAD
run_logged git-status git status --porcelain --untracked-files=all
run_logged rustc-version rustc -Vv
run_logged cargo-version cargo -V
run_logged fmt cargo fmt --check
run_logged check cargo check --all-targets --locked
run_logged clippy cargo clippy --all-targets --locked -- -D warnings
run_logged test-all-targets cargo test --all-targets --locked
cp "$evidence_root/commands/test-all-targets.stdout" "$evidence_root/test-report.txt"
run_logged docs cargo run --quiet --locked -- docs check --root docs
run_logged install-script sh -n install.sh
run_logged diff-check git diff --check

run_trace db cargo test --locked --test unit planning_store_schema -- --nocapture
run_trace fs cargo test --locked --test unit writer -- --nocapture
run_trace protocol cargo test --locked --test unit planning_protocol_golden -- --nocapture
run_trace migration cargo test --locked --test integration planning_migration -- --nocapture
run_trace purge cargo test --locked --test unit planning_store_purge -- --nocapture

cargo_lock_sha256="$(sha256_file Cargo.lock)"
rustc_host="$(rustc -vV | sed -n 's/^host: //p')"
uname_s="$(uname -s)"
uname_m="$(uname -m)"
pi_bin="${PI_BIN:-}"
export EVIDENCE_ROOT="$evidence_root"
export RELEASE_COMMIT="$release_commit"
export HEAD_COMMIT="$head_commit"
export STATUS_BEFORE="$status_before"
export CARGO_LOCK_SHA256="$cargo_lock_sha256"
export RUSTC_HOST="$rustc_host"
export UNAME_S="$uname_s"
export UNAME_M="$uname_m"
export PI_BIN_VALUE="$pi_bin"

python3 - "$evidence_root" <<'PY'
import hashlib
import json
import os
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
files = []
for path in sorted(root.rglob("*")):
    if not path.is_file() or path.name == "manifest.json":
        continue
    relative = path.relative_to(root).as_posix()
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    files.append({"path": relative, "bytes": path.stat().st_size, "sha256": digest})

commands = []
exit_codes = root / "commands" / "exit-codes.tsv"
for line in exit_codes.read_text().splitlines():
    name, status = line.split("\t", 1)
    commands.append({"name": name, "exit_code": int(status)})

manifest = {
    "schema": "megara.release-evidence/v1",
    "release_commit": os.environ["RELEASE_COMMIT"],
    "head_commit": os.environ["HEAD_COMMIT"],
    "working_tree_clean_before_staging": not bool(os.environ["STATUS_BEFORE"]),
    "cargo_lock_sha256": os.environ["CARGO_LOCK_SHA256"],
    "toolchain": {
        "rustc_vv": "commands/rustc-version.stdout",
        "cargo_version": "commands/cargo-version.stdout",
        "rustc_host": os.environ["RUSTC_HOST"],
    },
    "platform": {
        "os": os.environ["UNAME_S"],
        "architecture": os.environ["UNAME_M"],
    },
    "pi_bin": os.environ["PI_BIN_VALUE"] or None,
    "commands": commands,
    "traces": {
        "db": "traces/db.trace",
        "filesystem": "traces/fs.trace",
        "protocol": "traces/protocol.trace",
        "migration": "traces/migration.trace",
        "purge": "traces/purge.trace",
    },
    "hash_scope": "all files below this directory except manifest.json; manifest is self-referential",
    "files": files,
}
(root / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
PY

echo "release evidence staged at $evidence_root"
echo "manifest: $evidence_root/manifest.json"
