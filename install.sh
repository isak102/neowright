#!/usr/bin/env sh
set -eu

repo="isak102/neowright"
version="${NEOWRIGHT_VERSION:-latest}"
install_dir="${NEOWRIGHT_INSTALL_DIR:-$HOME/.local/bin}"

case "$(uname -s)" in
  Darwin) os="apple-darwin" ;;
  Linux) os="unknown-linux-gnu" ;;
  *)
    echo "Unsupported OS: $(uname -s)" >&2
    exit 1
    ;;
esac

case "$(uname -m)" in
  arm64 | aarch64) arch="aarch64" ;;
  x86_64 | amd64) arch="x86_64" ;;
  *)
    echo "Unsupported architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

if [ "$version" = "latest" ]; then
  version="$(curl -fsSL "https://api.github.com/repos/$repo/releases/latest" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
fi

if [ -z "$version" ]; then
  echo "Could not determine latest Neowright release" >&2
  exit 1
fi

target="$arch-$os"
archive="neowright-$version-$target.tar.gz"
url="https://github.com/$repo/releases/download/$version/$archive"
tmp_dir="$(mktemp -d)"

cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

echo "Installing Neowright $version for $target"
curl -fsSL "$url" | tar -xz -C "$tmp_dir"
mkdir -p "$install_dir"

if [ -w "$install_dir" ]; then
  mv "$tmp_dir/neowright" "$install_dir/neowright"
else
  sudo mv "$tmp_dir/neowright" "$install_dir/neowright"
fi

echo "Installed $install_dir/neowright"
