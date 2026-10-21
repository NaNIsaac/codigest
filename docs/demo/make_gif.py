"""
Renders docs/demo/demo.gif from recorded terminal sessions.

Each scene is a command line plus a file holding exactly what that command
printed (ANSI colors included). Nothing is typed into a live terminal, so the
GIF is reproducible, but the output must come from a real run:

    cd sample-project
    cdg cap -- pytest -q > cap.txt 2>&1
    cdg pack > pack.txt 2>&1        # rich needs a terminal or force_terminal to keep colors

    uv run --with pillow docs/demo/make_gif.py "pytest -q:cap.txt" "cdg pack:pack.txt"

A scene written as `cmd:file` shows `cdg cap -- cmd` when cmd does not start with cdg.
"""

import re
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

COLS, ROWS = 80, 23
FONT_SIZE = 16
PAD, TITLE_H = 20, 34
FONTS = Path("C:/Windows/Fonts")

BG, FG, DIM_MIX = (13, 17, 23), (201, 209, 217), 0.55
PALETTE = {
    30: (72, 79, 88),
    31: (255, 123, 114),
    32: (63, 185, 80),
    33: (210, 153, 34),
    34: (88, 166, 255),
    35: (188, 140, 255),
    36: (57, 197, 207),
    37: (177, 186, 196),
    90: (110, 118, 129),
    91: (255, 161, 152),
    92: (86, 211, 100),
    93: (227, 179, 65),
    94: (121, 192, 255),
    95: (210, 168, 255),
    96: (86, 212, 221),
    97: (240, 246, 252),
}
PROMPT = [("~/project", (88, 166, 255), False), (" $ ", FG, False)]

_SGR = re.compile(r"\x1b\[([0-9;]*)m")


def load_fonts():
    regular = ImageFont.truetype(str(FONTS / "consola.ttf"), FONT_SIZE)
    bold = ImageFont.truetype(str(FONTS / "consolab.ttf"), FONT_SIZE)
    emoji = ImageFont.truetype(str(FONTS / "seguiemj.ttf"), FONT_SIZE - 2)
    return regular, bold, emoji


def parse(text: str) -> list[list[tuple[str, tuple, bool]]]:
    """ANSI text -> lines of (text, color, bold) runs."""
    lines = []
    for raw in text.replace("\r", "").rstrip("\n").split("\n"):
        runs, fg, bold, dim, pos = [], None, False, False, 0
        for m in [*_SGR.finditer(raw), None]:
            end = m.start() if m else len(raw)
            if end > pos:
                color = fg or FG
                if dim:
                    color = tuple(
                        int(c * DIM_MIX + b * (1 - DIM_MIX)) for c, b in zip(color, BG, strict=True)
                    )
                runs.append((raw[pos:end], color, bold))
            if not m:
                break
            for code in (int(c) for c in (m[1] or "0").split(";") if c):
                match code:
                    case 0:
                        fg, bold, dim = None, False, False
                    case 1:
                        bold = True
                    case 2:
                        dim = True
                    case 22:
                        bold = dim = False
                    case 39:
                        fg = None
                    case c if c in PALETTE:
                        fg = PALETTE[c]
            pos = m.end()
        lines.append(runs)
    return lines


class Screen:
    def __init__(self):
        self.regular, self.bold, self.emoji = load_fonts()
        self.cw = self.regular.getlength("M")
        self.lh = FONT_SIZE + 5
        self.size = (int(PAD * 2 + self.cw * COLS), TITLE_H + PAD * 2 + self.lh * ROWS)
        self.lines: list[list] = []

    def render(self, cursor: bool = False) -> Image.Image:
        img = Image.new("RGB", self.size, BG)
        d = ImageDraw.Draw(img)
        d.rectangle([0, 0, self.size[0], TITLE_H], fill=(22, 27, 34))
        for i, c in enumerate([(255, 95, 86), (255, 189, 46), (39, 201, 63)]):
            d.ellipse([16 + i * 20, 11, 28 + i * 20, 23], fill=c)
        visible = self.lines[-ROWS:]
        for row, runs in enumerate(visible):
            x, y = PAD, TITLE_H + PAD + row * self.lh
            for text, color, bold in runs:
                for ch in text:
                    if ord(ch) > 0x2600:
                        d.text((x, y + 2), ch, font=self.emoji, embedded_color=True)
                        x += self.cw * 2
                    else:
                        d.text((x, y), ch, font=self.bold if bold else self.regular, fill=color)
                        x += self.cw
            if cursor and row == len(visible) - 1:
                d.rectangle([x, y + 2, x + self.cw - 1, y + self.lh - 2], fill=FG)
        return img


def main(scenes: list[str]) -> None:
    screen, frames, durations = Screen(), [], []

    def snap(ms: int, cursor: bool = False) -> None:
        frames.append(screen.render(cursor))
        durations.append(ms)

    for scene in scenes:
        command, _, path = scene.partition(":")
        if not command.startswith("cdg"):
            command = f"cdg cap -- {command}"
        screen.lines.append(list(PROMPT))
        snap(700, cursor=True)
        for i in range(0, len(command), 2):
            screen.lines[-1] = [*PROMPT, (command[: i + 2], FG, False)]
            snap(55, cursor=True)
        snap(350, cursor=True)
        for line in parse(Path(path).read_text(encoding="utf-8")):
            screen.lines.append(line)
            snap(30)
        snap(2600)

    screen.lines.append(list(PROMPT))
    snap(1500, cursor=True)

    out = Path(__file__).with_name("demo.gif")
    frames[0].save(
        out, save_all=True, append_images=frames[1:], duration=durations, loop=0, optimize=True
    )
    print(f"{out} ({len(frames)} frames, {out.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    main(sys.argv[1:])
