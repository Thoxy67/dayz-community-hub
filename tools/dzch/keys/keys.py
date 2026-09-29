"""Make a new updater signing key: `make keys`.

    uv run --project tools dzch keys              # new key pair, public half into tauri.conf.json
    uv run --project tools dzch keys --dry-run    # say what would happen

The private key goes where `make publish` reads it: TAURI_SIGNING_KEY_FILE
(environment, then apps/gui/.env), else ~/.config/dayz-community-hub/updater.key
(or $DZCH_CONFIG_DIR/updater.key); made by `tauri signer generate` without a
password. A key already there is kept beside it as
updater.key.bak-<timestamp> (and its .pub likewise): nothing is deleted.
The public key is written into apps/gui/src-tauri/tauri.conf.json
(plugins.updater.pubkey), which both updaters check every download against.

A new key means every copy already installed, which carries the old public
key, refuses the updates signed with the new one: those copies have to be
updated by hand once, then the new key carries on.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

from dzch import paths

TAURI_CONF = paths.TAURI / "tauri.conf.json"


def config_dir() -> Path:
    return Path(
        os.environ.get("DZCH_CONFIG_DIR")
        or os.path.expanduser("~/.config/dayz-community-hub")
    )


# ── pure parts, tested in test_keys.py ──────────────────────────────────────


def with_pubkey(conf_text: str, pubkey: str) -> str:
    """tauri.conf.json with plugins.updater.pubkey replaced, edited as text so
    the file keeps its layout."""
    data = json.loads(conf_text)
    if "pubkey" not in data.get("plugins", {}).get("updater", {}):
        raise ValueError("tauri.conf.json has no plugins.updater.pubkey")
    new, n = re.subn(
        r'("pubkey"\s*:\s*")[^"]*(")', rf"\g<1>{pubkey}\g<2>", conf_text, count=1
    )
    if n != 1:
        raise ValueError("could not find the pubkey value to replace")
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
    """Run the tauri signer; return the public key (base64, as tauri.conf wants it)."""
    key.parent.mkdir(parents=True, exist_ok=True)
    r = subprocess.run(
        ["bunx", "tauri", "signer", "generate", "--ci", "-p", "", "-w", str(key)],
        cwd=paths.GUI,
        text=True,
        capture_output=True,
        check=False,
    )
    if r.returncode != 0:
        sys.exit(f"tauri signer generate failed: {(r.stderr or r.stdout).strip()}")
    pub = key.with_name(key.name + ".pub")
    if not pub.is_file():
        sys.exit(f"tauri signer generate left no {pub}")
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
    print(f"public key -> {TAURI_CONF} (plugins.updater.pubkey)")
    if cli.dry_run:
        print("dry run: nothing changed")
        return
    for existing in (key, pub):
        backup(existing, now)
    pubkey = generate(key)
    TAURI_CONF.write_text(
        with_pubkey(TAURI_CONF.read_text(encoding="utf-8"), pubkey), encoding="utf-8"
    )
    print(
        "\ndone. Commit apps/gui/src-tauri/tauri.conf.json before the next `make publish`.\n"
        "Copies installed before this key cannot verify updates signed with it:\n"
        "they need one manual update to a release built with the new key."
    )


if __name__ == "__main__":
    main()
