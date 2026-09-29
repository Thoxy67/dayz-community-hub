"""Generate and publish the AUR package: `make aur` and `make aur-build`.

The package, dayz-community-hub-git, builds the latest `master` from source.
Its PKGBUILD is kept in this repository (packaging/aur/PKGBUILD.in); this
fills in the version, writes .SRCINFO with makepkg, and pushes the result to
the package's own repository, git.thoxy.xyz/AUR/dayz-community-hub-git,
through a clone kept in var/aur/.

    uv run --project tools dzch aur              # generate, commit, push
    uv run --project tools dzch aur --dry-run    # generate, show the diff, change nothing remote
    uv run --project tools dzch aur --force      # commit and push even when nothing changed
    uv run --project tools dzch aur --build      # generate, then build the package locally (no push)

The version is `<app version>.r<commits on master>.g<short hash>`, what the
PKGBUILD's own pkgver() computes on a user's machine, read here from
`origin/master` (else `master`): what the package would build now.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys

from dzch import paths

PKG = "dayz-community-hub-git"
TEMPLATE = paths.REPO / "packaging" / "aur" / "PKGBUILD.in"
README = paths.REPO / "packaging" / "aur" / "README.md"
GITIGNORE = paths.REPO / "packaging" / "aur" / "gitignore"
LICENSE = paths.REPO / "LICENSE"
TAURI_CONF = paths.TAURI / "tauri.conf.json"
AUR_DIR = paths.VAR / "aur"
CLONE = AUR_DIR / PKG
BUILD = AUR_DIR / "build"
REMOTE = os.environ.get(
    "DZCH_AUR_REMOTE", "ssh://git@git.thoxy.xyz:222/AUR/dayz-community-hub-git.git"
)

# ── pure parts, tested in test_aur.py ───────────────────────────────────────


def pkgver(version: str, count: int, short: str) -> str:
    """What the PKGBUILD's pkgver() prints: pacman versions may not hold `-`."""
    return f"{version.replace('-', '_')}.r{count}.g{short}"


def render(template: str, version: str) -> str:
    if "@PKGVER@" not in template:
        raise ValueError("the PKGBUILD template has no @PKGVER@")
    return template.replace("@PKGVER@", version)


def only_version_moved(old: str, new: str) -> bool:
    """Whether two PKGBUILDs (or .SRCINFOs) differ in nothing but pkgver."""

    def strip(text: str) -> list[str]:
        return [
            line for line in text.splitlines() if not line.strip().startswith("pkgver")
        ]

    return old != new and strip(old) == strip(new)


def commit_message(version: str, subject: str) -> str:
    return f"{version}: {subject}" if subject else version


# ── the machinery ───────────────────────────────────────────────────────────


def sh(*argv: str, cwd=None, capture: bool = True) -> str:
    r = subprocess.run(argv, cwd=cwd, text=True, capture_output=capture, check=False)
    if r.returncode != 0:
        detail = (r.stderr or r.stdout or "").strip() if capture else ""
        sys.exit(f"`{' '.join(argv)}` failed{': ' + detail if detail else ''}")
    return r.stdout if capture else ""


def source_ref() -> str:
    for ref in ("origin/master", "master"):
        r = subprocess.run(
            ["git", "rev-parse", "--verify", "--quiet", ref],
            cwd=paths.REPO,
            capture_output=True,
            text=True,
            check=False,
        )
        if r.returncode == 0:
            return ref
    sys.exit("neither origin/master nor master exists in this repository")


def current_pkgver() -> tuple[str, str]:
    ref = source_ref()
    version = json.loads(TAURI_CONF.read_text(encoding="utf-8"))["version"]
    count = int(sh("git", "rev-list", "--count", ref, cwd=paths.REPO).strip())
    short = sh("git", "rev-parse", "--short=7", ref, cwd=paths.REPO).strip()
    subject = sh("git", "log", "-1", "--format=%s", ref, cwd=paths.REPO).strip()
    return pkgver(version, count, short), subject


def checkout(dry_run: bool) -> None:
    """The package's repository in var/aur/, cloned or brought up to date."""
    AUR_DIR.mkdir(parents=True, exist_ok=True)
    if (CLONE / ".git").is_dir():
        if not dry_run:
            sh("git", "pull", "--ff-only", "--quiet", cwd=CLONE)
    else:
        print(f"== cloning {REMOTE}")
        sh("git", "clone", "--quiet", REMOTE, str(CLONE))


def generate(into, version: str) -> None:
    into.mkdir(parents=True, exist_ok=True)
    (into / "PKGBUILD").write_text(
        render(TEMPLATE.read_text(encoding="utf-8"), version), encoding="utf-8"
    )
    shutil.copyfile(README, into / "README.md")
    shutil.copyfile(GITIGNORE, into / ".gitignore")
    shutil.copyfile(LICENSE, into / "LICENSE")
    srcinfo = sh("makepkg", "--printsrcinfo", cwd=into)
    (into / ".SRCINFO").write_text(srcinfo, encoding="utf-8")


def build_locally(version: str) -> None:
    """Build the generated package in var/aur/build, away from the clone."""
    if BUILD.exists():
        for name in ("PKGBUILD", ".SRCINFO"):
            (BUILD / name).unlink(missing_ok=True)
    generate(BUILD, version)
    print(f"== building {PKG} {version} in {BUILD} (makepkg -f --cleanbuild)")
    sh("makepkg", "-f", "--cleanbuild", cwd=BUILD, capture=False)
    built = sorted(BUILD.glob(f"{PKG}-*.pkg.tar.zst"), key=os.path.getmtime)
    if built:
        print(f"\nbuilt: {built[-1]}\ninstall it with: sudo pacman -U {built[-1]}")


def main() -> None:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument(
        "--dry-run", action="store_true", help="generate and show, push nothing"
    )
    ap.add_argument(
        "--force", action="store_true", help="commit and push even when nothing changed"
    )
    ap.add_argument(
        "--build",
        action="store_true",
        help="build the package locally instead of publishing",
    )
    cli = ap.parse_args()

    if shutil.which("makepkg") is None:
        sys.exit("makepkg is needed (an Arch-based system)")
    version, subject = current_pkgver()
    print(f"== {PKG} {version} (from: {subject})")

    if cli.build:
        build_locally(version)
        return

    checkout(cli.dry_run)
    old = (
        (CLONE / "PKGBUILD").read_text(encoding="utf-8")
        if (CLONE / "PKGBUILD").exists()
        else ""
    )
    generate(CLONE, version)
    new = (CLONE / "PKGBUILD").read_text(encoding="utf-8")
    status = sh("git", "status", "--porcelain", cwd=CLONE).strip()
    print(sh("git", "diff", "--stat", cwd=CLONE) or "(no changes)")
    if cli.dry_run:
        print(sh("git", "diff", cwd=CLONE))
        sh("git", "checkout", "--quiet", "--", ".", cwd=CLONE)
        sh("git", "clean", "-fdq", cwd=CLONE)
        print("dry run: nothing committed or pushed")
        return
    if not status and not cli.force:
        print(
            "nothing changed: the package already builds this version (--force to push anyway)"
        )
        return
    if only_version_moved(old, new):
        print("only the version moved: pushed so AUR helpers see the new commit")
    sh("git", "add", "-A", cwd=CLONE)
    sh(
        "git",
        "commit",
        "--quiet",
        "--allow-empty",
        "-m",
        commit_message(version, subject),
        cwd=CLONE,
    )
    sh("git", "push", "--quiet", cwd=CLONE)
    print(f"\ndone: {PKG} {version} is published to {REMOTE}")


if __name__ == "__main__":
    main()
