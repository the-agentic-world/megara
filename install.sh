#!/bin/sh
set -eu

repo="${MEGARA_REPO:-the-agentic-world/megara}"
install_dir="${MEGARA_INSTALL_DIR:-/usr/local/bin}"
version="${MEGARA_VERSION:-latest}"

die() {
  echo "megara install: $*" >&2
  exit 1
}

need() {
  command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

need curl
need tar
need uname
need shasum

os="$(uname -s)"
arch="$(uname -m)"

[ "$os" = "Darwin" ] || die "only macOS is supported by this installer"

case "$arch" in
  arm64|aarch64)
    target="aarch64-apple-darwin"
    ;;
  x86_64|amd64)
    target="x86_64-apple-darwin"
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

if mkdir -p "$install_dir" 2>/dev/null && [ -w "$install_dir" ]; then
  install -m 755 "${tmpdir}/extract/megara" "${install_dir}/megara"
else
  need sudo
  sudo mkdir -p "$install_dir"
  sudo install -m 755 "${tmpdir}/extract/megara" "${install_dir}/megara"
fi

echo "Installed megara to ${install_dir}/megara"
"${install_dir}/megara" --version
