"""Where everything is. The only place the repository root is computed."""

from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
VAR = REPO / "var"
DIST = VAR / "dist"
