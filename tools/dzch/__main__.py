"""The `dzch` command line entry point.

    dzch release [--pre] [--version X] [--dry-run] [--skip-github]
    dzch aur [--dry-run] [--force] [--build]
    dzch keys [--dry-run]

Every argument after the subcommand goes straight to that module's own
argument parser, `--help` included.
"""

from __future__ import annotations

import importlib
import sys

COMMANDS = {
    "release": (
        "dzch.release.release",
        "build, sign and publish a release (AppImage + Windows)",
    ),
    "aur": (
        "dzch.aur.aur",
        "generate and publish the AUR package, or build it locally",
    ),
    "keys": ("dzch.keys.keys", "make a new updater signing key"),
}

USAGE = "usage: dzch <command> [args]\n\n" + "\n".join(
    f"    dzch {name:<8} {desc}" for name, (_, desc) in COMMANDS.items()
)


def main() -> None:
    if len(sys.argv) < 2 or sys.argv[1] in {"-h", "--help"}:
        print(USAGE)
        sys.exit(0 if len(sys.argv) >= 2 else 2)
    command, rest = sys.argv[1], sys.argv[2:]
    if command not in COMMANDS:
        sys.exit(f"unknown command {command!r}\n\n{USAGE}")
    module = importlib.import_module(COMMANDS[command][0])
    sys.argv = [f"dzch {command}", *rest]
    module.main()


if __name__ == "__main__":
    main()
