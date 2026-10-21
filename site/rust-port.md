# Rust Port

::: warning Status: complete in the repository, not released
Every command of the Python build has been ported to Rust. There is no release yet: no published binary and no installer. The Python package on PyPI is the only released implementation, and the other pages on this site document it.
:::

The port is a single native binary, built next to the Python package in the repository (`rust/`). The Python build (`src/codigest`) stays the reference: for the same input the Rust build must print the same bytes to stdout and exit with the same code.

For now the binary is called `cdg-rs`, so it can sit next to the Python `cdg` and be compared with it. They are separate programs. Both read and write the same data: the terminal capture store, `.codigest/` in a project (config, baseline, cache), and the shell hooks.

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

New in this build: `term --raw` prints the stored output of one command without tags. See the [note in the command reference](/commands#term-raw). The sections below describe the other options only this build has.

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

### `--format markdown|json` for `scan` and `term`

XML stays the default. Markdown is for chats that render it: `scan` gives the instruction, the structure, and a heading and a fenced block per file with the language of its extension; `term` gives a heading per command, its details as a list, and the command with its output in one `console` block. The fence grows past any backticks inside. JSON is for scripts.

```console
$ cdg-rs scan src/format.rs:17-31 --format markdown -m "Review these"
$ cdg-rs term -n 3 --failed --format json
```

`scan --format json` has `project`, `instruction`, `structure` and `files` (`path`, `lines` for a range, `content`). `term --format json` is an array with `index` counting back from the latest, `command`, `exit_code`, `cwd`, `started_at`, `duration_ms`, `output`, `output_available` and `truncated`.

### `--preset NAME`

A preset is a set of options saved under a name. `--preset NAME` puts them right after the command name, so anything typed on the command line wins.

```toml
# %APPDATA%\codigest\config.toml on Windows, ~/.config/codigest/config.toml elsewhere,
# or .codigest/config.toml in the project (replaces a user preset of the same name)
[presets.term.debug]
format = "markdown"
count = 3
failed = true
```

```console
$ cdg-rs term --preset debug          # last 3 failed commands as Markdown
$ cdg-rs term --preset debug -n 1     # the same, only the last one
```

Keys are option names with `_` for `-` (`no_copy = true` is `--no-copy`). `true` adds a flag, `false` leaves it out, and lists are joined with commas. An unknown preset stops with exit code 2 and names the file to define it in.

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
