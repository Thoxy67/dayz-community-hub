#!/usr/bin/env python3
"""Render the README's screenshots into .forgejo/assets/screenshots/.

Every picture is the real window on the mock hub, rendered headless by the
`ui_shot` example (no display needed): 1280 x 800 points at 1.25 pixels per
point, in English, with the mock's update offer turned off. Two pictures are
grids put together here: four themes on the server list, and the two seasonal
themes side by side.

    uv run --with pillow scripts/readme_shots.py          # every picture
    uv run --with pillow scripts/readme_shots.py servers  # some of them

Pillow is the only dependency (for the grids and the PNG optimisation);
oxipng, when it is on the PATH, makes the files smaller still.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / ".forgejo/assets/screenshots"
HEADER = [{"size": [1280, 800]}, {"ppp": 1.25}, {"locale": "en"}]


def key(k: str) -> dict:
    return {"key": k}


def shot(name: str) -> dict:
    # Filled in with the temporary folder by `render`.
    return {"shot": name}


# One script per run of ui_shot; each line is ui_shot's (see its header).
SCRIPTS: dict[str, list[dict]] = {
    "main": [
        {"theme": "chernarus"},
        {"view": "servers"},
        {"steps": 30},
        # The fourth row of the mock is a modded server.
        *[key("ArrowDown")] * 4,
        {"command": "servers.detail.tab", "args": {"tab": "mods"}},
        {"steps": 20},
        shot("servers"),
        {"command": "servers.detail.tab", "args": {"tab": "stats"}},
        {"wait": 1500},
        shot("server-stats"),
        *[
            line
            for view in ("favorites", "mods", "options", "offline", "friends", "stats")
            for line in ({"view": view}, {"steps": 20}, shot(view if view != "stats" else "statistics"))
        ],
        {"view": "news"},
        {"wait": 1500},
        shot("news"),
    ],
    "controller": [
        {"theme": "chernarus"},
        {"view": "servers"},
        {"steps": 30},
        *[{"pad": "dpad_down"}] * 4,
        {"steps": 20},
        shot("controller"),
    ],
    "themes": [
        {"view": "servers"},
        {"steps": 30},
        *[
            line
            for theme in ("topo", "nord", "catppuccin_latte", "blood_moon")
            for line in ({"theme": theme}, {"steps": 20}, shot(f"theme-{theme}"))
        ],
    ],
    # No theme line: one picked during the season would be worn instead of the season's.
    "halloween": [{"date": "2026-10-15"}, {"view": "servers"}, {"steps": 30}, {"wait": 2500}, shot("halloween")],
    "christmas": [{"date": "2026-12-15"}, {"view": "favorites"}, {"steps": 30}, {"wait": 4000}, shot("christmas")],
}

# The pictures the README shows, and where each comes from.
SINGLE = ["servers", "server-stats", "favorites", "mods", "options", "offline", "friends", "statistics", "news"]
SINGLE += ["controller"]
GRIDS = {
    "themes": (["theme-topo", "theme-nord", "theme-catppuccin_latte", "theme-blood_moon"], 2),
    "seasonal": (["halloween", "christmas"], 2),
}
NEEDS = {
    "main": {n for n in SINGLE if n != "controller"},
    "controller": {"controller"},
    "themes": {"themes"},
    "halloween": {"seasonal"},
    "christmas": {"seasonal"},
}


def render(scripts: list[str], tmp: Path) -> None:
    subprocess.run(
        ["cargo", "build", "-q", "-p", "dayz-community-hub", "--example", "ui_shot"],
        cwd=ROOT,
        check=True,
    )
    env = {**os.environ, "DZCH_MOCK": "update-none,news-images"}
    for name in scripts:
        path = tmp / f"{name}.jsonl"
        lines = [
            {"shot": str(tmp / f"{line['shot']}.png")} if "shot" in line else line for line in HEADER + SCRIPTS[name]
        ]
        path.write_text("".join(json.dumps(line) + "\n" for line in lines))
        run = ["cargo", "run", "-q", "-p", "dayz-community-hub", "--example", "ui_shot", "--", str(path)]
        subprocess.run(run, cwd=ROOT, env=env, check=True, stdout=subprocess.DEVNULL)


def grid(tmp: Path, names: list[str], cols: int, out: Path, w: int = 800, h: int = 500, gap: int = 8) -> None:
    rows = (len(names) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * w + (cols - 1) * gap, rows * h + (rows - 1) * gap), (20, 20, 20))
    for i, name in enumerate(names):
        picture = Image.open(tmp / f"{name}.png").convert("RGB").resize((w, h), Image.LANCZOS)
        sheet.paste(picture, ((i % cols) * (w + gap), (i // cols) * (h + gap)))
    sheet.save(out, optimize=True)


def main(wanted: list[str]) -> int:
    every = set(SINGLE) | set(GRIDS)
    unknown = set(wanted) - every
    if unknown:
        print(f"unknown pictures: {', '.join(sorted(unknown))}; known: {', '.join(sorted(every))}")
        return 2
    wanted_set = set(wanted) or every
    scripts = [s for s, gives in NEEDS.items() if gives & wanted_set]
    OUT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as t:
        tmp = Path(t)
        render(scripts, tmp)
        written = []
        for name in SINGLE:
            if name in wanted_set:
                Image.open(tmp / f"{name}.png").convert("RGB").save(OUT / f"{name}.png", optimize=True)
                written.append(OUT / f"{name}.png")
        for name, (parts, cols) in GRIDS.items():
            if name in wanted_set:
                grid(tmp, parts, cols, OUT / f"{name}.png")
                written.append(OUT / f"{name}.png")
    if shutil.which("oxipng"):
        subprocess.run(["oxipng", "-q", "-o", "3", "--strip", "safe", *map(str, written)], check=True)
    for path in written:
        print(path.relative_to(ROOT))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
