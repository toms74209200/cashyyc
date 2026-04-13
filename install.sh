#!/bin/bash
set -euo pipefail

REPO="toms74209200/cashyyc"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

OS=$(uname -s)
ARCH=$(uname -m)

case "$OS" in
  Linux)
    case "$ARCH" in
      x86_64)
        UBUNTU_MAJOR=0
        if [ -f /etc/os-release ]; then
          . /etc/os-release
          if [ "${ID:-}" = "ubuntu" ]; then
            UBUNTU_MAJOR=$(echo "${VERSION_ID:-0}" | cut -d. -f1)
          fi
        fi
        if [ "$UBUNTU_MAJOR" -ge 24 ] 2>/dev/null; then
          ARTIFACT="cashyyc-linux-x86_64-ubuntu24"
        else
          ARTIFACT="cashyyc-linux-x86_64-ubuntu22"
        fi
        ;;
      aarch64)
        ARTIFACT="cashyyc-linux-aarch64"
        ;;
      *)
        echo "Unsupported architecture: $ARCH" >&2
        exit 1
        ;;
    esac
    ;;
  Darwin)
    case "$ARCH" in
      arm64)
        ARTIFACT="cashyyc-macos-aarch64"
        ;;
      *)
        echo "Unsupported architecture: $ARCH" >&2
        exit 1
        ;;
    esac
    ;;
  *)
    echo "Unsupported OS: $OS" >&2
    exit 1
    ;;
esac

TAG=$(curl -sf "https://api.github.com/repos/$REPO/releases/latest" \
  | grep -o '"tag_name": "[^"]*"' \
  | cut -d'"' -f4)

if [ -z "$TAG" ]; then
  echo "Failed to fetch latest release tag" >&2
  exit 1
fi

URL="https://github.com/$REPO/releases/download/$TAG/$ARTIFACT"

mkdir -p "$INSTALL_DIR"
echo "Downloading $ARTIFACT ($TAG)..."
curl -fsSL "$URL" -o "$INSTALL_DIR/cyyc"
chmod +x "$INSTALL_DIR/cyyc"
echo "Installed cyyc $TAG to $INSTALL_DIR/cyyc"
