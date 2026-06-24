#!/usr/bin/env bash
# install.sh — one-time installer for the ai-meta `meta` CLI.
#
#   curl -fsSL https://raw.githubusercontent.com/tizz98/ai-meta/main/install.sh | bash
#
# Downloads the `meta` binary from GitHub releases, verifies its checksum, and
# installs it onto your PATH. After installing, run `meta init` in a repo.
#
# Tunables (environment variables):
#   AI_META_VERSION   pin a version (e.g. 0.1.0); default: latest release.
#   AI_META_BIN_DIR   install directory; default: $HOME/.local/bin.
#   AI_META_REPO      owner/repo to fetch from; default: tizz98/ai-meta.
set -euo pipefail

repo="${AI_META_REPO:-tizz98/ai-meta}"
bin_dir="${AI_META_BIN_DIR:-$HOME/.local/bin}"
version="${AI_META_VERSION:-}"

err() { echo "install.sh: $*" >&2; }
die() { err "$*"; exit 1; }

for cmd in curl uname mkdir chmod mv; do
  command -v "$cmd" >/dev/null 2>&1 || die "required command not found: $cmd"
done

# Map the host to a release target (mirrors ./meta shim).
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)            tgt=x86_64-unknown-linux-musl ;;
  Linux-aarch64|Linux-arm64) tgt=aarch64-unknown-linux-musl ;;
  Darwin-arm64)            tgt=aarch64-apple-darwin ;;
  Darwin-x86_64)           tgt=x86_64-apple-darwin ;;
  *) die "unsupported platform $(uname -s)-$(uname -m)" ;;
esac

# Resolve the download base. Without a pinned version, use the `latest` alias
# GitHub serves for the most recent release.
if [ -n "$version" ]; then
  version="${version#v}" # tolerate a leading "v"
  base="https://github.com/$repo/releases/download/v$version"
  label="v$version"
else
  base="https://github.com/$repo/releases/latest/download"
  label="latest"
fi

asset="ai-meta-$tgt"
url="$base/$asset"

tmp="$(mktemp -d "${TMPDIR:-/tmp}/ai-meta-install.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

err "downloading meta ($label) for $tgt..."
curl -fSL --proto '=https' --tlsv1.2 "$url" -o "$tmp/$asset" \
  || die "failed to download $url"

# Verify the checksum when the release publishes one (it always should).
if curl -fsSL --proto '=https' --tlsv1.2 "$url.sha256" -o "$tmp/$asset.sha256" 2>/dev/null; then
  want="$(awk '{print $1}' "$tmp/$asset.sha256")"
  if command -v sha256sum >/dev/null 2>&1; then
    have="$(sha256sum "$tmp/$asset" | awk '{print $1}')"
  elif command -v shasum >/dev/null 2>&1; then
    have="$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')"
  else
    have=""
    err "warning: no sha256 tool found; skipping checksum verification"
  fi
  if [ -n "$have" ] && [ "$want" != "$have" ]; then
    die "checksum mismatch for $asset (expected $want, got $have)"
  fi
else
  err "warning: no checksum published for $asset; skipping verification"
fi

mkdir -p "$bin_dir"
chmod +x "$tmp/$asset"
mv "$tmp/$asset" "$bin_dir/meta"

err "installed meta -> $bin_dir/meta"

# Nudge the user if the install dir isn't on PATH.
case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *)
    err ""
    err "note: $bin_dir is not on your PATH. Add it, e.g.:"
    err "  echo 'export PATH=\"$bin_dir:\$PATH\"' >> ~/.bashrc && exec \$SHELL"
    ;;
esac

err ""
err "done. Run 'meta init' in a repo to get started."
