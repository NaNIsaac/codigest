"""
`--preset NAME`: option sets kept in TOML, expanded into the command line before it is
parsed. Options given on the command line come after the preset's, and the later one
wins, so a preset only fills in what was not said.

    [presets.term.debug]
    format = "markdown"
    count = 3
    failed = true

Presets are read from the user configuration file and then from the project's
`.codigest/config.toml`; a project preset replaces a user preset of the same name.
"""

import math
import os
from decimal import Decimal
from pathlib import Path
lazy import tomllib


class PresetError(Exception):
    pass


def user_config() -> Path:
    """%APPDATA%\\codigest\\config.toml on Windows, else $XDG_CONFIG_HOME/codigest/config.toml
    or ~/.config/codigest/config.toml."""
    if os.name == "nt":
        base = os.environ.get("APPDATA") or Path.home() / "AppData" / "Roaming"
    else:
        base = os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config"
    return Path(base) / "codigest" / "config.toml"


def _project_config(start: Path) -> Path | None:
    """The nearest `.codigest/config.toml` from `start` upwards, stopping at a Git root."""
    for directory in (start, *start.parents):
        candidate = directory / ".codigest" / "config.toml"
        if candidate.is_file():
            return candidate
        if (directory / ".git").exists():
            return None
    return None


def _preset_from(file: Path, command: str, name: str) -> dict | None:
    try:
        text = file.read_text(encoding="utf-8")
    except OSError, UnicodeDecodeError:
        return None
    try:
        doc = tomllib.loads(text)
    except tomllib.TOMLDecodeError as err:
        raise PresetError(f"reading {file}: {err}") from err
    found = doc
    for key in ("presets", command, name):
        if not isinstance(found, dict):
            return None
        found = found.get(key)
    return found if isinstance(found, dict) else None


def _number(value: float) -> str:
    """A float the way Rust prints it: no exponent, no trailing `.0`."""
    if math.isnan(value):
        return "NaN"
    if value in (float("inf"), float("-inf")):
        return "inf" if value > 0 else "-inf"
    text = format(Decimal(repr(value)), "f")
    return text.rstrip("0").rstrip(".") if "." in text else text


def _item(value: object) -> str:
    """An array element as Rust joins it: strings bare, anything else as TOML writes it."""
    match value:
        case str():
            return value
        case bool():
            return "true" if value else "false"
        case list():
            return (
                "[" + ", ".join(f'"{v}"' if isinstance(v, str) else _item(v) for v in value) + "]"
            )
        case _:
            return str(value)


def _to_args(table: dict, origin: str) -> list[str]:
    args = []
    for key in sorted(table):
        value = table[key]
        flag = "--" + key.replace("_", "-")
        match value:
            case True:
                args.append(flag)
            case False:
                pass
            case str():
                args += [flag, value]
            case int():
                args += [flag, str(value)]
            case float():
                args += [flag, _number(value)]
            case list():
                args += [flag, ",".join(_item(v) for v in value)]
            case dict():
                raise PresetError(f"{origin}: option {key} cannot take table")
            case _:
                raise PresetError(f"{origin}: option {key} cannot take datetime")
    return args


def _debug(text: str) -> str:
    """Rust's `{:?}` of a string, for the common characters."""
    escaped = text.replace("\\", "\\\\").replace('"', '\\"')
    return '"' + escaped.replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t") + '"'


def expand(argv: list[str]) -> list[str]:
    """`argv` (without the program name) with `--preset NAME` replaced by the preset's
    options, placed right after the command name so that anything typed after it
    overrides them."""
    at = next((i for i, a in enumerate(argv) if a == "--preset" or a.startswith("--preset=")), None)
    if at is None:
        return argv
    if argv[at].startswith("--preset="):
        name, width = argv[at].removeprefix("--preset="), 1
    elif at + 1 < len(argv):
        name, width = argv[at + 1], 2
    else:
        raise PresetError("--preset needs a name")
    # The command is the first argument that is not an option.
    command_at = next((i for i, a in enumerate(argv) if not a.startswith("-")), None)
    if command_at is None:
        raise PresetError("--preset needs a command")
    command = argv[command_at]

    user = user_config()
    project = _project_config(Path.cwd())
    found = None
    for file in (user, project):
        if file is not None and (table := _preset_from(file, command, name)) is not None:
            found = table, file
    if found is None:
        raise PresetError(
            f"no preset {_debug(name)} for {command}; "
            f"define [presets.{command}.{name}] in {user} or .codigest/config.toml"
        )
    table, file = found
    inserted = _to_args(table, f"preset {name} in {file}")

    out = []
    for i, arg in enumerate(argv):
        if at <= i < at + width:
            continue
        out.append(arg)
        if i == command_at:
            out += inserted
    return out
