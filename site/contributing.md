# Contributing

Codigest is MIT licensed and lives at [github.com/NaNIsaac/codigest](https://github.com/NaNIsaac/codigest). Issues and pull requests are welcome.

## Set up

You need [uv](https://docs.astral.sh/uv/). It fetches Python 3.15 for you.

```bash
git clone https://github.com/NaNIsaac/codigest
cd codigest
uv run cdg --help               # run from the checkout
uv run --with pytest pytest -q  # tests
uvx ruff@0.17.1 check           # lint, the same version CI uses
```

CI runs the tests on Ubuntu, a PowerShell check on Windows (`Get-Help` forwarding, including Windows PowerShell 5.1), and ruff on every push to `main` and every pull request.

## Where things are

[Architecture](/architecture) maps the modules. The tests are in `tests/`, one file per area (`test_pack.py`, `test_secrets.py`, `test_termstore.py`, ...). Notes from using Codigest on itself are in `docs/dogfood.md`.

## Guidelines

- Target the Python version in `pyproject.toml` and use its syntax and standard library. Add a dependency only when it is really needed.
- Comments say why, not what. Match the naming, style and language of the code around yours.
- A command's output is part of its interface. If you change it, update the tests that pin it and the pages here that show it.
- Commit messages are short conventional commits: `fix: ...`, `feat: ...`, `docs: ...`.

## This documentation site

The site is VitePress, in `site/`, in English and Korean.

```bash
cd site
npm ci
npm run dev       # local preview with reload
npm run build     # the build CI runs
```

The English pages are in `site/` and the Korean ones in `site/ko/`, with the same file names. When you change a page, change both.

Option tables in the command reference are checked against the CLI:

```bash
uv run python site/scripts/check_reference.py
```

It fails when a documented option does not exist, or an existing option is missing, in either language. Command output shown on the site comes from real runs; when you change a command's output, run it again and paste the result.

Pushing to `main` with changes under `site/` builds the site and deploys it to GitHub Pages.
