#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 4 ]; then
  echo "usage: $0 <version> <github-repo> <dist-dir> <output-file>" >&2
  exit 2
fi

version="$1"
repo="$2"
dist_dir="$3"
output_file="$4"
tag="v${version}"
bin_name="sisyphus"

sha_for() {
  local target="$1"
  local archive="${bin_name}-${tag}-${target}.tar.gz"
  local checksum_file
  checksum_file="$(find "${dist_dir}" -type f -name "${archive}.sha256" -print -quit)"
  if [ -z "${checksum_file}" ]; then
    echo "missing checksum for ${archive}" >&2
    exit 1
  fi
  awk '{print $1}' "${checksum_file}"
}

url_for() {
  local target="$1"
  local archive="${bin_name}-${tag}-${target}.tar.gz"
  printf 'https://github.com/%s/releases/download/%s/%s' "${repo}" "${tag}" "${archive}"
}

macos_arm_sha="$(sha_for "aarch64-apple-darwin")"
linux_x64_sha="$(sha_for "x86_64-unknown-linux-gnu")"
macos_arm_url="$(url_for "aarch64-apple-darwin")"
linux_x64_url="$(url_for "x86_64-unknown-linux-gnu")"

cat > "${output_file}" <<FORMULA
class Sisyphus < Formula
  desc "Local issue-to-agent broker and lifecycle controller"
  homepage "https://github.com/${repo}"
  version "${version}"

  on_macos do
    depends_on arch: :arm64
    url "${macos_arm_url}"
    sha256 "${macos_arm_sha}"
  end

  on_linux do
    if Hardware::CPU.intel?
      url "${linux_x64_url}"
      sha256 "${linux_x64_sha}"
    end
  end

  def install
    bin.install "sisyphus"
  end

  test do
    assert_match "sisyphus", shell_output("#{bin}/sisyphus --help")
  end
end
FORMULA
