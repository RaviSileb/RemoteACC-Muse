#!/usr/bin/env bash
# RemoteACC CLI installer: Linux, macOS, Termux (Android).
# Spuštění: curl -fsSL https://raw.githubusercontent.com/RaviSileb/RemoteACC-Muse/main/scripts/install.sh | bash
# Volitelně: VERSION=v0.1.0 INSTALL_DIR=~/.local/bin bash install.sh
set -eu

REPO='RaviSileb/RemoteACC-Muse'
VERSION="${VERSION:-latest}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

is_termux() { [ -n "${TERMUX_VERSION:-}" ] || [[ "${PREFIX:-}" == *com.termux* ]]; }

resolve_tag() {
  if [ "$VERSION" != 'latest' ]; then
    case "$VERSION" in
      v*) printf '%s' "$VERSION" ;;
      *) printf 'v%s' "$VERSION" ;;
    esac
    return
  fi
  curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
    | grep -o '"tag_name": *"[^"]*"' | head -1 | cut -d'"' -f4
}

download_url() { # $1 = asset, $2 = tag (pouze pro pevnou verzi)
  if [ "$VERSION" = 'latest' ]; then
    printf 'https://github.com/%s/releases/latest/download/%s' "$REPO" "$1"
  else
    printf 'https://github.com/%s/releases/download/%s/%s' "$REPO" "$2" "$1"
  fi
}

install_binary() { # $1 = cesta k racc-cli
  mkdir -p "$INSTALL_DIR"
  cp "$1" "$INSTALL_DIR/racc-cli"
  chmod +x "$INSTALL_DIR/racc-cli"
  if [ "$(uname -s)" = 'Darwin' ]; then
    xattr -d com.apple.quarantine "$INSTALL_DIR/racc-cli" 2>/dev/null || true
  fi
  "$INSTALL_DIR/racc-cli" --help >/dev/null
  echo "racc-cli nainstalovano: $INSTALL_DIR/racc-cli"
  case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) echo "POZNAMKA: pridejte do PATH: export PATH=\"\$PATH:$INSTALL_DIR\"" ;;
  esac
}

install_from_release() { # $1 = asset
  tag="$(resolve_tag)"
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  curl -fsSL -o "$tmp/asset.tgz" "$(download_url "$1" "$tag")"
  tar xzf "$tmp/asset.tgz" -C "$tmp"
  install_binary "$tmp/racc-cli"
}

install_from_source() { # $1 = tag
  if is_termux; then
    pkg update -y
    pkg install -y rust git clang
    dest="$PREFIX/bin"
  else
    command -v cargo >/dev/null || { echo 'CHYBA: chybi cargo (nainstalujte Rust: https://rustup.rs/)' >&2; exit 1; }
    dest="$INSTALL_DIR"
    mkdir -p "$dest"
  fi
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  if [ "$1" = 'latest' ]; then
    git clone --depth 1 "https://github.com/$REPO.git" "$tmp/src"
  else
    git clone --depth 1 --branch "$1" "https://github.com/$REPO.git" "$tmp/src"
  fi
  (cd "$tmp/src" && cargo build --release -p racc-cli)
  if is_termux; then
    cp "$tmp/src/target/release/racc-cli" "$dest/racc-cli"
    chmod +x "$dest/racc-cli"
    "$dest/racc-cli" --help >/dev/null
    echo "racc-cli nainstalovano: $dest/racc-cli"
  else
    install_binary "$tmp/src/target/release/racc-cli"
  fi
}

main() {
  if is_termux; then
    echo 'Termux detekovan: sestavuji ze zdroju (nekolik minut)...'
    if [ "$VERSION" = 'latest' ]; then
      install_from_source 'latest'
    else
      install_from_source "$(resolve_tag)"
    fi
    return
  fi
  os="$(uname -s)"; arch="$(uname -m)"
  case "$os-$arch" in
    Linux-x86_64) install_from_release 'racc-cli-linux-x86_64.tar.gz' ;;
    Darwin-arm64) install_from_release 'racc-cli-macos-arm64.tar.gz' ;;
    *)
      echo "Pro $os/$arch neni hotova binarka: sestavuji ze zdroju..."
      if [ "$VERSION" = 'latest' ]; then tag='latest'; else tag="$(resolve_tag)"; fi
      install_from_source "$tag"
      ;;
  esac
}

main
