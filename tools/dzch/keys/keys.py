"""Make a new updater signing key: `make keys`.

    uv run --project tools dzch keys              # new key pair, public half into the updater
    uv run --project tools dzch keys --dry-run    # say what would happen

The private key goes where `make publish` reads it: TAURI_SIGNING_KEY_FILE
(environment, then ./.env), else ~/.config/dayz-community-hub/updater.key
(or $DZCH_CONFIG_DIR/updater.key); made by `cargo xtask keygen` without a
password, in the format `tauri signer generate` wrote (xtask/src/sign.rs). A
key already there is kept beside it as updater.key.bak-<timestamp> (and its
.pub likewise): nothing is deleted. The public key is written into
crates/app/hub/src/features/updater/release.rs (`PUBKEY`), which the updater
checks every download against.

A new key means every copy already installed, which carries the old public
key, refuses the updates signed with the new one: those copies have to be
updated by hand once, then the new key carries on.
"""

from __future__ import annotations

import argparse
import datetime as dt
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

from dzch import paths

UPDATER = (
    paths.REPO
    / "crates"
    / "app"
    / "hub"
    / "src"
    / "features"
    / "updater"
    / "release.rs"
)


def config_dir() -> Path:
    return Path(
        os.environ.get("DZCH_CONFIG_DIR")
        or os.path.expanduser("~/.config/dayz-community-hub")
    )


# ── pure parts, tested in test_keys.py ──────────────────────────────────────


def with_rust_pubkey(source: str, pubkey: str) -> str:
    """release.rs with its `PUBKEY` constant replaced and nothing else."""
    new, n = re.subn(
        r'(pub\(crate\) const PUBKEY: &str = ")[^"]*(";)',
        rf"\g<1>{pubkey}\g<2>",
        source,
        count=1,
    )
    if n != 1:
        raise ValueError("release.rs has no `PUBKEY` constant")
    return new


def backup_name(path: Path, now: dt.datetime) -> Path:
    return path.with_name(f"{path.name}.bak-{now.strftime('%Y%m%d-%H%M%S')}")


def backup(path: Path, now: dt.datetime) -> Path | None:
    """Move an existing file aside; return where it went."""
    if not path.exists():
        return None
    dest = backup_name(path, now)
    shutil.move(str(path), dest)
    return dest


# ── the command ─────────────────────────────────────────────────────────────


def generate(key: Path) -> str:
    """Run `cargo xtask keygen`; return the public key (base64, as the updater
    holds it)."""
    r = subprocess.run(
        ["cargo", "xtask", "keygen", "--write", str(key)],
        cwd=paths.REPO,
        text=True,
        capture_output=True,
        check=False,
    )
    if r.returncode != 0:
        sys.exit(f"cargo xtask keygen failed: {(r.stderr or r.stdout).strip()}")
    pub = key.with_name(key.name + ".pub")
    if not pub.is_file():
        sys.exit(f"cargo xtask keygen left no {pub}")
    os.chmod(key, 0o600)
    return pub.read_text(encoding="utf-8").strip()


def main() -> None:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument(
        "--dry-run", action="store_true", help="say what would happen, change nothing"
    )
    cli = ap.parse_args()

    from dzch.release import release

    env = dict(os.environ)
    dotenv = (
        release.read_dotenv(open(release.DOTENV, encoding="utf-8").read())
        if os.path.isfile(release.DOTENV)
        else {}
    )
    key = Path(release.key_path(env, dotenv, str(config_dir())))
    pub = key.with_name(key.name + ".pub")
    now = dt.datetime.now(dt.UTC)
    print(f"new key       {key}")
    for existing in (key, pub):
        if existing.exists():
            print(f"kept aside    {existing} -> {backup_name(existing, now)}")
    print(f"public key -> {UPDATER} (PUBKEY)")
    if cli.dry_run:
        print("dry run: nothing changed")
        return
    for existing in (key, pub):
        backup(existing, now)
    pubkey = generate(key)
    UPDATER.write_text(
        with_rust_pubkey(UPDATER.read_text(encoding="utf-8"), pubkey), encoding="utf-8"
    )
    print(
        f"\ndone. Commit {UPDATER.relative_to(paths.REPO)} before the next `make publish`.\n"
        "Copies installed before this key cannot verify updates signed with it:\n"
        "they need one manual update to a release built with the new key."
    )


if __name__ == "__main__":
    main()
