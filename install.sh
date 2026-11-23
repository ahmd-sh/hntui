#!/bin/sh
# hntui installer: downloads the standalone binary for this platform from the
# latest GitHub release (or HNTUI_VERSION's). No dependencies.
#   curl -fsSL https://raw.githubusercontent.com/ahmd-sh/hntui/main/install.sh | sh
#   curl -fsSL .../install.sh | HNTUI_VERSION=0.6.0 sh
set -e

REPO="ahmd-sh/hntui"
INSTALL_DIR="${HNTUI_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${HNTUI_VERSION#v}"

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

asset="hntui-$os-$arch.tar.gz"
if [ -n "$VERSION" ]; then
  url="https://github.com/$REPO/releases/download/v$VERSION/$asset"
else
  url="https://github.com/$REPO/releases/latest/download/$asset"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "downloading $asset${VERSION:+ (v$VERSION)} ..."
if ! curl -fsSL "$url" -o "$tmp/$asset"; then
  echo "hntui: download failed: $url" >&2
  [ -n "$VERSION" ] && echo "is v$VERSION a release with a $os-$arch binary? see https://github.com/$REPO/releases" >&2
  exit 1
fi
tar -xzf "$tmp/$asset" -C "$tmp"

mkdir -p "$INSTALL_DIR"
mv "$tmp/hntui" "$INSTALL_DIR/hntui"
chmod +x "$INSTALL_DIR/hntui"

echo "installed: $INSTALL_DIR/hntui"
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) echo "note: $INSTALL_DIR is not on your PATH — add it to your shell profile" ;;
esac
