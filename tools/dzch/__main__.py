"""The `dzch` command line entry point.

    dzch release [--pre] [--version X] [--dry-run]   forward to dzch.release.release

Every argument after the subcommand goes straight to that module's own
argument parser, `--help` included.
"""

from __future__ import annotations

import sys

USAGE = """usage: dzch <command> [args]

    dzch release [--pre] [--version X] [--dry-run]   build, sign and publish a release"""


def main() -> None:
    if len(sys.argv) < 2 or sys.argv[1] in {"-h", "--help"}:
        print(USAGE)
        sys.exit(0 if len(sys.argv) >= 2 else 2)
    command, rest = sys.argv[1], sys.argv[2:]
    if command == "release":
        from dzch.release import release

        sys.argv = ["dzch release", *rest]
        release.main()
        return
    sys.exit(f"unknown command {command!r}\n\n{USAGE}")


if __name__ == "__main__":
    main()
