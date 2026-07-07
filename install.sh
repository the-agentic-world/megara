#!/bin/sh
set -eu

repo="${MEGARA_REPO:-the-agentic-world/megara}"
version="${MEGARA_VERSION:-latest}"

die() {
  echo "megara install: $*" >&2
  exit 1
}

default_install_dir() {
  if [ -n "${MEGARA_INSTALL_DIR:-}" ]; then
    printf '%s\n' "$MEGARA_INSTALL_DIR"
    return
  fi

  [ -n "${HOME:-}" ] || die "HOME is not set; set MEGARA_INSTALL_DIR to a writable directory"
  if [ -n "${XDG_BIN_HOME:-}" ]; then
    printf '%s\n' "$XDG_BIN_HOME"
    return
  fi
  printf '%s\n' "$HOME/.local/bin"
}

need() {
  command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

cleanup_legacy_binary() {
  [ "${MEGARA_SKIP_LEGACY_CLEANUP:-}" != "1" ] || return 0

  legacy_dir="/usr/local/bin"
  legacy_bin="${legacy_dir}/megara"
  case "$install_dir" in
    "$legacy_dir"|"$legacy_dir"/) return 0 ;;
  esac
  [ -e "$legacy_bin" ] || return 0

  if [ -w "$legacy_dir" ]; then
    if rm -f "$legacy_bin"; then
      echo "Removed legacy Megara binary at ${legacy_bin}"
    else
      echo "Note: could not remove legacy Megara binary at ${legacy_bin}." >&2
    fi
  else
    echo "Note: legacy Megara binary remains at ${legacy_bin}. Remove it or place ${install_dir} earlier in PATH." >&2
  fi
}

is_writable_dir() {
  candidate="$1"
  mkdir -p "$candidate" 2>/dev/null || return 1
  [ -d "$candidate" ] || return 1

  probe="${candidate}/.megara-write-test-$$"
  (: > "$probe") 2>/dev/null || return 1
  rm -f "$probe"
}

select_install_dir() {
  requested_install_dir="$install_dir"

  if is_writable_dir "$install_dir"; then
    return
  fi

  if [ -n "${MEGARA_INSTALL_DIR:-}" ] && [ "${MEGARA_INSTALL_DIR_STRICT:-}" = "1" ]; then
    die "install directory is not writable: $install_dir; set MEGARA_INSTALL_DIR to a writable directory"
  fi

  [ -n "${HOME:-}" ] || die "HOME is not set; set MEGARA_INSTALL_DIR to a writable directory"

  for candidate in "${XDG_BIN_HOME:-}" "$HOME/.local/bin" "$HOME/bin" "$HOME/.megara/bin"; do
    [ -n "$candidate" ] || continue
    [ "$candidate" != "$requested_install_dir" ] || continue
    if is_writable_dir "$candidate"; then
      echo "Note: ${requested_install_dir} is not writable. Installing to ${candidate}." >&2
      install_dir="$candidate"
      return
    fi
  done

  die "no writable install directory found; set MEGARA_INSTALL_DIR to a writable directory"
}

verify_checksum() {
  if command -v shasum >/dev/null 2>&1; then
    (cd "$tmpdir" && shasum -a 256 -c "${archive}.sha256") >/dev/null
  elif command -v sha256sum >/dev/null 2>&1; then
    (cd "$tmpdir" && sha256sum -c "${archive}.sha256") >/dev/null
  else
    die "required command not found: shasum or sha256sum"
  fi
}

need curl
need install
need tar
need uname

install_dir="$(default_install_dir)"
select_install_dir

os="$(uname -s)"
arch="$(uname -m)"

case "$os:$arch" in
  Darwin:arm64|Darwin:aarch64)
    target="aarch64-apple-darwin"
    ;;
  Darwin:x86_64|Darwin:amd64)
    die "macOS Intel is not supported by Megara release artifacts"
    ;;
  Linux:x86_64|Linux:amd64)
    target="x86_64-unknown-linux-gnu"
    ;;
  Linux:arm64|Linux:aarch64)
    die "Linux arm64 is not supported by Megara release artifacts"
    ;;
  *)
    die "unsupported platform: ${os} ${arch}"
    ;;
esac

if [ "$version" = "latest" ]; then
  tag="$(
    curl -fsSL "https://api.github.com/repos/${repo}/releases/latest" \
      | sed -n 's/^[[:space:]]*"tag_name":[[:space:]]*"\([^"]*\)".*$/\1/p' \
      | head -n 1
  )"
  [ -n "$tag" ] || die "could not resolve latest release tag"
else
  case "$version" in
    v*) tag="$version" ;;
    *) tag="v$version" ;;
  esac
fi

archive="megara-${tag}-${target}.tar.gz"
base_url="https://github.com/${repo}/releases/download/${tag}"
tmpdir="$(mktemp -d)"

cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT INT TERM

echo "Downloading ${archive}"
curl -fL "${base_url}/${archive}" -o "${tmpdir}/${archive}"
curl -fL "${base_url}/${archive}.sha256" -o "${tmpdir}/${archive}.sha256"

verify_checksum

mkdir -p "${tmpdir}/extract"
tar -xzf "${tmpdir}/${archive}" -C "${tmpdir}/extract"
[ -x "${tmpdir}/extract/megara" ] || chmod +x "${tmpdir}/extract/megara"

install -m 755 "${tmpdir}/extract/megara" "${install_dir}/megara"
cleanup_legacy_binary

echo "Installed megara to ${install_dir}/megara"
case ":${PATH:-}:" in
  *":${install_dir}:"*) ;;
  *) echo "Note: ${install_dir} is not on PATH. Add it before running megara." >&2 ;;
esac
"${install_dir}/megara" --version
