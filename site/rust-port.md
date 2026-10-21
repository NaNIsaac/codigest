# Rust Port

::: info Status: released as a preview
Every command of the Python build has been ported to Rust, and prebuilt binaries are attached to GitHub releases marked as pre-release (see [Install](#install)). The Python package on PyPI is still the main release, and the other pages on this site document it.
:::

The port is a single native binary, built next to the Python package in the repository (`rust/`). The Python build (`src/codigest`) stays the reference: for the same input the Rust build must print the same bytes to stdout and exit with the same code.

For now the binary is called `cdg-rs`, so it can sit next to the Python `cdg` and be compared with it. They are separate programs. Both read and write the same data: the terminal capture store, `.codigest/` in a project (config, baseline, cache), and the shell hooks.

## Install

Prebuilt binaries are attached to the GitHub releases tagged `rust-v*`: Windows x64, Linux x64 and ARM64 (static, musl), macOS Intel and Apple Silicon, and Android ARM64 for Termux. Each archive holds a folder named after the target with `cdg-rs` (`cdg-rs.exe` on Windows) and the license.

Windows (PowerShell):

```powershell
$v = "0.12.0"; $t = "x86_64-pc-windows-msvc"
Invoke-WebRequest "https://github.com/NaNIsaac/codigest/releases/download/rust-v$v/cdg-rs-$v-$t.zip" -OutFile cdg-rs.zip
Expand-Archive cdg-rs.zip "$env:LOCALAPPDATA\Programs" -Force
# then add $env:LOCALAPPDATA\Programs\cdg-rs-$v-$t to PATH
```

Linux, macOS and Termux (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `x86_64-apple-darwin`, `aarch64-apple-darwin` or `aarch64-linux-android`):

```bash
v=0.12.0; t=x86_64-unknown-linux-musl
curl -L "https://github.com/NaNIsaac/codigest/releases/download/rust-v$v/cdg-rs-$v-$t.tar.gz" | tar xz
install "cdg-rs-$v-$t/cdg-rs" ~/.local/bin/
```

With the GitHub CLI: `gh release download rust-v0.12.0 -R NaNIsaac/codigest -p '*windows*'`.
## Build

You need a Rust toolchain. From a checkout of the repository:

```bash
cd rust
cargo build --release        # target/release/cdg-rs
cargo test                   # unit and integration tests
CODIGEST_PARITY=1 cargo test # also compare with the Python `cdg` on PATH
```

The release binary is 4.6 MB and needs no Python.

## What is ported

Every command, with the same options: `scan`, `digest`, `tree`, `mark`, `diff`, `semdiff`, `cap`, `pack`, `term` (with `clear`, `path`, `install`, `uninstall` and the hooks' `ingest`), `mcp`, `status`, `init`, `clean` and `shell-help`.

New in this build: `term --raw` prints the stored output of one command without tags. See the [note in the command reference](/commands#term-raw). The section below describes the one other thing only this build has. `--format markdown|json` of `scan` and `term` and `--preset NAME` work the same in both builds; they are in the [command reference](/commands#formats).

## Only in this build

### More languages in `digest`

Besides Python and MATLAB, `digest` outlines Rust, JavaScript (`.js`, `.jsx`, `.mjs`, `.cjs`), TypeScript (`.ts`, `.tsx`) and Go: one numbered line per definition, members indented under their type or `impl`, and the first line of the doc comment underneath. Rust items under `#[cfg(test)]` are left out. Part of `cdg-rs digest rust/src/presets.rs` in this repository:

```text
 1: //! `--preset NAME`: option sets kept in TOML, expanded into the command line before it is
25: pub fn user_config() -> PathBuf
        `%APPDATA%\codigest\config.toml` on Windows, else `$XDG_CONFIG_HOME/codigest/config.toml`
40: fn project_config(start: &Path) -> Option<PathBuf>
        The nearest `.codigest/config.toml` from `start` upwards, stopping at a Git root.
93: pub fn expand(argv: Vec<OsString>) -> Result<Vec<OsString>>
```

## How parity is checked

- Integration tests run each command in a temporary project with both builds and compare stdout byte for byte and the exit code. `CODIGEST_PARITY=1 cargo test` turns these on; they need the Python `cdg` on `PATH`.
- The Python outline used by `digest` reproduces `ast.unparse`. `scripts/outline_parity.py` compares the two builds file by file: 1,747 files across the 40 repositories checked out next to the project, with no differences.
- Import resolution (`-r`, `pack`) was compared on five repositories, with no differences.

## Speed

Wall time with [hyperfine](https://github.com/sharkdp/hyperfine) (`-N`, 2 warm-up runs, 15 runs, 5 for the 10,000-file rows), mean ± standard deviation in milliseconds. AMD Ryzen 5 7500F (6 cores, 12 threads), Windows 11, Python 3.15.0; the Python build is the same commit installed with `uv tool install --editable`. Output went to a pipe, so nothing was copied to the clipboard. The 1,000- and 10,000-file projects are generated Python packages (6.4 MB for 10,000 files).

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

## Differences from the Python build

- `term --raw` is new. With more than one entry selected it stops with exit code 2 instead of joining them, because the boundaries between outputs would be lost.
- A Python file whose syntax is otherwise broken still gets an outline of what parses; the Python build shows nothing for it. (Undecodable bytes in code do give no outline in both.)
- When a failing test calls functions from several modules, `pack` orders those modules by where the calls appear in the test; the Python build follows `ast.walk`'s breadth-first order.
- Status lines printed directly to the terminal (`init`, `clean`, `term clear`) end with `\n` on Windows, where Python writes `\r\n`. The command results themselves (stdout of `scan`, `pack`, ... and `-o` files) use the platform line ending, as Python does.
- `cap` forwards Ctrl+C and gives the command 5 seconds before killing it.

The rest of this site describes the Python `cdg`.
