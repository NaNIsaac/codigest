#!/bin/sh
# Installs cdg, the Codigest binary, from a GitHub release.
#
#   curl -fsSL https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.sh | sh
#
# CDG_VERSION=0.12.1 picks a release instead of the latest one. CDG_INSTALL_DIR picks the
# folder (default ~/.local/bin, or $PREFIX/bin in Termux). Running it again replaces the
# installed binary.
set -eu

repo=NaNIsaac/codigest

if [ "$(uname -o 2>/dev/null || true)" = Android ]; then
    target=aarch64-linux-android
    default_dir="${PREFIX:-$HOME/.local}/bin"
else
    case "$(uname -s)-$(uname -m)" in
        Linux-x86_64) target=x86_64-unknown-linux-musl ;;
        Linux-aarch64 | Linux-arm64) target=aarch64-unknown-linux-musl ;;
        Darwin-x86_64) target=x86_64-apple-darwin ;;
        Darwin-arm64) target=aarch64-apple-darwin ;;
        MINGW* | MSYS* | CYGWIN*)
            echo "On Windows, run in PowerShell: irm https://raw.githubusercontent.com/$repo/main/install.ps1 | iex" >&2
            exit 1
            ;;
        *)
            echo "No prebuilt cdg for $(uname -s) $(uname -m). Install the Python package instead: pipx install codigest" >&2
            exit 1
            ;;
    esac
    default_dir="$HOME/.local/bin"
fi
dir="${CDG_INSTALL_DIR:-$default_dir}"

if [ -n "${CDG_VERSION:-}" ]; then
    version="${CDG_VERSION#rust-}"
    version="${version#v}"
else
    # The newest rust-v* release that is not a draft or pre-release. Not releases/latest:
    # a GitHub release for a Python v* tag would take that spot.
    tag=$(curl -fsSL "https://api.github.com/repos/$repo/releases?per_page=30" | awk '
        /^    "tag_name":/ { split($0, f, "\""); tag = f[4]; draft = 0 }
        /^    "draft": false/ { draft = 1 }
        /^    "prerelease": false/ && draft && tag ~ /^rust-v/ { print tag; exit }')
    version="${tag#rust-v}"
fi
[ -n "$version" ] || { echo "Could not find the latest release." >&2; exit 1; }

name="codigest-$version-$target"
# A pip, pipx or uv install of the Python package puts a script called cdg in the same folder.
if [ -f "$dir/cdg" ] && [ "$(head -c 2 "$dir/cdg")" = "#!" ] && [ -z "${CDG_FORCE:-}" ]; then
    echo "$dir/cdg is the Python codigest ($("$dir/cdg" --version 2>/dev/null || echo "unknown version"))." >&2
    echo "Remove it first (uv tool uninstall codigest, or pipx uninstall codigest), pick another" >&2
    echo "CDG_INSTALL_DIR, or set CDG_FORCE=1 to replace it." >&2
    exit 1
fi

# GitHub records each asset's SHA-256 as "sha256:<hex>"
want=$(curl -fsSL "https://api.github.com/repos/$repo/releases/tags/rust-v$version" | awk -v asset="$name.tar.gz" '
    index($0, "\"name\": \"" asset "\"") { found = 1 }
    found && /"digest": "sha256:/ { split($0, f, "\""); sub("sha256:", "", f[4]); print f[4]; exit }')
[ -n "$want" ] || { echo "Release rust-v$version has no SHA-256 digest for $name.tar.gz." >&2; exit 1; }

url="https://github.com/$repo/releases/download/rust-v$version/$name.tar.gz"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

echo "Downloading $name.tar.gz"
curl -fsSL "$url" -o "$work/$name.tar.gz"
if command -v sha256sum >/dev/null; then
    got=$(sha256sum "$work/$name.tar.gz" | cut -d ' ' -f 1)
else
    got=$(shasum -a 256 "$work/$name.tar.gz" | cut -d ' ' -f 1)
fi
[ "$got" = "$want" ] || { echo "Checksum mismatch for $name.tar.gz: expected $want, got $got." >&2; exit 1; }
tar xzf "$work/$name.tar.gz" -C "$work"
mkdir -p "$dir"
cp "$work/$name/cdg" "$dir/cdg.new"
chmod 755 "$dir/cdg.new"
# Renaming over the old file works even while it is running.
mv -f "$dir/cdg.new" "$dir/cdg"

echo "Installed $("$dir/cdg" --version) to $dir"
case ":$PATH:" in
    *":$dir:"*)
        first=$(command -v cdg || true)
        if [ -n "$first" ] && [ "$first" != "$dir/cdg" ]; then
            echo "Warning: cdg in new shells is $first, not this install. Remove one or put $dir earlier on PATH." >&2
        fi
        ;;
    *) echo "Add $dir to PATH to run cdg from anywhere." ;;
esac
echo "Next: cdg term install   (captures command output in new shells)"
