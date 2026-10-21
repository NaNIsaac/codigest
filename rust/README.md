# Codigest in Rust

A port of Codigest to a single native binary, built next to the Python package in this
repository. The Python build (`src/codigest`) stays the reference: for the same input the
Rust build must print the same bytes to stdout and exit with the same code.

While the port is in progress the binary is called `cdg-rs`, so it can be installed next to
the Python `cdg` and compared with it. Both read and write the same data: the terminal
capture store, `.codigest/` in a project, and the same configuration files.

## Build

```bash
cd rust
cargo build --release        # target/release/cdg-rs
cargo test
```

## Status

| Command | State |
| --- | --- |
| `term` (with `-n`, `-i`, `-f`, `--here`, `--history-only`, `--context`, `clear`, `path`) | ported; output identical to Python |
| `term --raw` | new: the stored output of one command, without tags |
| `mcp` | ported; responses identical to Python |
| Python and MATLAB outlines (used by `digest`) | ported; identical to `ast.unparse` on 1,747 files |
| Import resolution (`-r`, `pack`), semantic diff, failing-test callees | ported, not yet wired to commands |
| `scan`, `tree`, `init`, `clean`, `digest` | in progress |
| `cap`, `term ingest`, `term install`, `mark`, `diff`, `semdiff`, `status`, `pack` | in progress |

## Checking parity

Unit and integration tests run with `cargo test`. Tests that compare against the installed
Python `cdg` are skipped unless `CODIGEST_PARITY=1` is set.

The outline engine has its own comparison over any number of folders:

```bash
cargo build --example outline
python scripts/outline_parity.py ../src target/debug/examples/outline ../src ../tests
```

## Differences from the Python build

- `term --raw` is new. With more than one entry selected it stops with exit code 2 instead of
  joining them, because the boundaries between outputs would be lost.
- A Python file with a syntax error still gets an outline of what parses; the Python build
  shows nothing for it.
- When a failing test calls functions from several modules, `pack` orders those modules by
  where the calls appear in the test; the Python build follows `ast.walk`'s breadth-first
  order.
