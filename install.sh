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
  printf '%s\n' "$HOME/.local/bin"
}

need() {
  command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

cleanup_legacy_binary() {
  [ "${MEGARA_SKIP_LEGACY_CLEANUP:-}" != "1" ] || return

  legacy_dir="/usr/local/bin"
  legacy_bin="${legacy_dir}/megara"
  case "$install_dir" in
    "$legacy_dir"|"$legacy_dir"/) return ;;
  esac
  [ -e "$legacy_bin" ] || return

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

need curl
need install
need tar
need uname
need shasum

install_dir="$(default_install_dir)"

os="$(uname -s)"
arch="$(uname -m)"

[ "$os" = "Darwin" ] || die "only macOS is supported by this installer"

case "$arch" in
  arm64|aarch64)
    target="aarch64-apple-darwin"
    ;;
  x86_64|amd64)
    die "macOS Intel is not supported by Megara release artifacts"
    ;;
  *)
    die "unsupported macOS architecture: $arch"
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

(cd "$tmpdir" && shasum -a 256 -c "${archive}.sha256") >/dev/null

mkdir -p "${tmpdir}/extract"
tar -xzf "${tmpdir}/${archive}" -C "${tmpdir}/extract"
[ -x "${tmpdir}/extract/megara" ] || chmod +x "${tmpdir}/extract/megara"

mkdir -p "$install_dir" || die "failed to create install directory: $install_dir"
[ -w "$install_dir" ] || die "install directory is not writable: $install_dir; set MEGARA_INSTALL_DIR to a writable directory"
install -m 755 "${tmpdir}/extract/megara" "${install_dir}/megara"
cleanup_legacy_binary

echo "Installed megara to ${install_dir}/megara"
case ":${PATH:-}:" in
  *":${install_dir}:"*) ;;
  *) echo "Note: ${install_dir} is not on PATH. Add it before running megara." >&2 ;;
esac
"${install_dir}/megara" --version
