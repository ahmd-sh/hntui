#!/bin/sh
# hntui installer: downloads the standalone binary for this platform from the
# latest GitHub release. No dependencies — the binary embeds its runtime.
#   curl -fsSL https://raw.githubusercontent.com/ahmd-sh/hntui/main/install.sh | sh
set -e

REPO="ahmd-sh/hntui"
INSTALL_DIR="${HNTUI_INSTALL_DIR:-$HOME/.local/bin}"

os=$(uname -s)
arch=$(uname -m)
case "$os" in
  Darwin) os="darwin" ;;
  Linux) os="linux" ;;
  *) echo "hntui: unsupported OS: $os" >&2; exit 1 ;;
esac
case "$arch" in
  arm64 | aarch64) arch="arm64" ;;
  x86_64 | amd64) arch="x64" ;;
  *) echo "hntui: unsupported architecture: $arch" >&2; exit 1 ;;
esac

if [ "$os-$arch" = "darwin-x64" ]; then
  echo "hntui: no prebuilt binary for Intel macOS (GitHub retired its Intel runners)." >&2
  echo "install with Bun instead:  bun add -g @ahmd-sh/hntui" >&2
  exit 1
fi

asset="hntui-$os-$arch.tar.gz"
url="https://github.com/$REPO/releases/latest/download/$asset"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "downloading $asset ..."
curl -fsSL "$url" -o "$tmp/$asset"
tar -xzf "$tmp/$asset" -C "$tmp"

mkdir -p "$INSTALL_DIR"
mv "$tmp/hntui" "$INSTALL_DIR/hntui"
chmod +x "$INSTALL_DIR/hntui"

echo "installed: $INSTALL_DIR/hntui"
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) echo "note: $INSTALL_DIR is not on your PATH — add it to your shell profile" ;;
esac
