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
    # The latest release is the newest that is not a draft or pre-release.
    tag=$(curl -fsSL "https://api.github.com/repos/$repo/releases/latest" |
        sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)
    version="${tag#rust-v}"
fi
[ -n "$version" ] || { echo "Could not find the latest release." >&2; exit 1; }

name="codigest-$version-$target"
url="https://github.com/$repo/releases/download/rust-v$version/$name.tar.gz"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

echo "Downloading $name.tar.gz"
curl -fsSL "$url" | tar xz -C "$work"
mkdir -p "$dir"
cp "$work/$name/cdg" "$dir/cdg.new"
chmod 755 "$dir/cdg.new"
# Renaming over the old file works even while it is running.
mv -f "$dir/cdg.new" "$dir/cdg"

echo "Installed $("$dir/cdg" --version) to $dir"
case ":$PATH:" in
    *":$dir:"*) ;;
    *) echo "Add $dir to PATH to run cdg from anywhere." ;;
esac
echo "Next: cdg term install   (captures command output in new shells)"
