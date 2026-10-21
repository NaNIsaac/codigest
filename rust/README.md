# Codigest in Rust

A port of Codigest to a single native binary, built next to the Python package in this
repository. The Python build (`src/codigest`) stays the reference: for the same input the
Rust build must print the same bytes to stdout and exit with the same code.

Both builds install a command called `cdg`, so keep one of them on `PATH`. They are separate
programs that read and write the same data: the terminal capture store, `.codigest/` in a project
(config, baseline, cache), and the shell hooks.

## Install

Windows (PowerShell), to `%LOCALAPPDATA%\Programs\codigest` and your user `PATH`:

```powershell
irm https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.ps1 | iex
```

Linux, macOS and Termux, to `~/.local/bin` (`$PREFIX/bin` in Termux):

```bash
curl -fsSL https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.sh | sh
```

Run the line again to update. `CDG_VERSION=0.12.1` pins a version, and `CDG_INSTALL_DIR=...` picks
the folder, in which case `PATH` is left alone.

Binaries are attached to the GitHub releases tagged `rust-v*`: Windows x64, Linux x64 and ARM64 (static, musl), macOS Intel and Apple Silicon, and Android ARM64 for Termux. Each archive is named `codigest-X.Y.Z-<target>.zip` (Windows) or `.tar.gz` and holds a folder `codigest-X.Y.Z-<target>/` with `cdg` (`cdg.exe` on Windows) and the license.

To install by hand, Windows (PowerShell):

```powershell
$v = "0.12.2"; $t = "x86_64-pc-windows-msvc"
Invoke-WebRequest "https://github.com/NaNIsaac/codigest/releases/download/rust-v$v/codigest-$v-$t.zip" -OutFile codigest.zip
Expand-Archive codigest.zip "$env:LOCALAPPDATA\Programs" -Force
# then add $env:LOCALAPPDATA\Programs\codigest-$v-$t to PATH
```

Linux, macOS and Termux (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `x86_64-apple-darwin`, `aarch64-apple-darwin` or `aarch64-linux-android`):

```bash
v=0.12.2; t=x86_64-unknown-linux-musl
curl -L "https://github.com/NaNIsaac/codigest/releases/download/rust-v$v/codigest-$v-$t.tar.gz" | tar xz
install "codigest-$v-$t/cdg" ~/.local/bin/
```

With the GitHub CLI: `gh release download rust-v0.12.2 -R NaNIsaac/codigest -p '*windows*'`.

## Build

```bash
cd rust
cargo build --release        # target/release/cdg
cargo test                   # unit and integration tests
CODIGEST_PARITY=1 cargo test -- --include-ignored  # also compare with the Python `cdg` on PATH (same version)
```

## Status

Every command of the Python build is ported, with the same options:
`scan`, `digest`, `tree`, `mark`, `diff`, `semdiff`, `cap`, `pack`, `term` (with `clear`,
`path`, `install`, `uninstall` and the hooks' `ingest`), `mcp`, `status`, `init`, `clean` and `shell-help`.

New in this build: `term --raw` prints the stored output of one command without tags.
`scan` and `term` take `--format markdown|json`, and every command takes `--preset NAME`,
the same as in the Python build.

How parity is checked:

- Integration tests run each command in a temporary project with both builds and compare
  stdout byte for byte and the exit code.
- The Python outline used by `digest` reproduces `ast.unparse`; `scripts/outline_parity.py`
  compares the two builds file by file (1,747 files across the 40 repositories checked out next to this one: no differences).
- Import resolution (`-r`, `pack`) was compared on five repositories: no differences.

## Speed

Wall time with [hyperfine](https://github.com/sharkdp/hyperfine) (`-N`, 2 warm-up runs, 15
runs, 5 for the 10,000-file rows), mean ± standard deviation in milliseconds. AMD Ryzen 5
7500F (6 cores, 12 threads), Windows 11, Python 3.15.0; the Python build is the same commit
installed with `uv tool install --editable`. Output went to a pipe, so nothing was copied to
the clipboard. The 1,000- and 10,000-file projects are generated Python packages (6.4 MB for
10,000 files).

| Command | Python | Rust | Ratio |
| --- | ---: | ---: | ---: |
| `--version` | 245 ± 10 | 6.2 ± 1.1 | 39.8x |
| `term -n 5` | 257 ± 7 | 7.5 ± 0.4 | 34.4x |
| `tree`, this repository (198 files) | 277 ± 6 | 11.9 ± 0.7 | 23.2x |
| `scan -y`, this repository | 510 ± 8 | 27.0 ± 1.0 | 18.9x |
| `digest`, xarray (503 files) | 3,142 ± 45 | 255.9 ± 9.0 | 12.3x |
| `tree`, 1,000 files | 453 ± 11 | 52.0 ± 1.0 | 8.7x |
| `scan -y`, 1,000 files | 661 ± 11 | 108.0 ± 3.0 | 6.1x |
| `digest`, 1,000 files | 1,074 ± 17 | 107.8 ± 5.6 | 10.0x |
| `tree`, 10,000 files | 1,818 ± 14 | 319.9 ± 5.4 | 5.7x |
| `scan -y`, 10,000 files | 3,705 ± 38 | 889.4 ± 9.6 | 4.2x |
| `digest`, 10,000 files | 7,811 ± 36 | 838.1 ± 17.3 | 9.3x |

The release binary is 4.6 MB and needs no Python.

## Differences from the Python build

- `digest` also outlines Rust (`.rs`), JavaScript (`.js`, `.jsx`, `.mjs`, `.cjs`), TypeScript
  (`.ts`, `.tsx`) and Go (`.go`) files, with tree-sitter grammars; the Python build only does
  Python and MATLAB. Each definition is one numbered line with its signature as written on one
  line and the first line of its doc comment under it (`///` and `#[doc]`, JSDoc, the `//`
  block above in Go). Members of `impl`, `trait`, `mod`, classes, interfaces and Go types are
  indented below them; in Rust, items under `#[cfg(test)]` are left out. JavaScript and
  TypeScript list exported values (and any function or class value); the body of a top-level
  immediately invoked function is read as top level. A syntax error in a signature can hide
  the items after it; errors inside bodies and between items do not.
- `term --raw` is new. With more than one entry selected it stops with exit code 2 instead of
  joining them, because the boundaries between outputs would be lost.
- A Python file whose syntax is otherwise broken still gets an outline of what parses; the
  Python build shows nothing for it. (Undecodable bytes in code do give no outline in both.)
- When a failing test calls functions from several modules, `pack` orders those modules by
  where the calls appear in the test; the Python build follows `ast.walk`'s breadth-first
  order.
- Status lines printed directly to the terminal (`init`, `clean`, `term clear`) end with
  `\n` on Windows, where Python writes `\r\n`. The command results themselves (stdout of
  `scan`, `pack`, ... and `-o` files) use the platform line ending, as Python does.
- `cap` forwards Ctrl+C and gives the command 5 seconds before killing it.
