#!/usr/bin/env python3
"""Build, sign, tag and publish a release: `make publish`.

Always run here, on the release machine, never in CI: it needs the updater's
signing key, and building both platforms from one Linux host (the Windows one
through cargo-xwin) is what makes the two halves of `latest.json` agree.

    uv run --project tools dzch release                  # the next version
    uv run --project tools dzch release --version 0.5.0  # a particular one
    uv run --project tools dzch release --pre            # a preview, see below
    uv run --project tools dzch release --dry-run        # say what would happen

What it does, in order, each step skipped when it is already done so that
running it again after a failure carries on from where it stopped:

  1. check    the working copy holds nothing but a release in progress, the
              signing key is there, and so is the Forgejo token
  2. version  the current one if it has never been tagged, else the patch + 1;
              written to Cargo.toml, tauri.conf.json and package.json, and the
              notes since the last tag written into CHANGELOG.md
  3. build    the Linux AppImage (signed by tauri: what the Linux updater
              downloads) and Windows (the .exe through cargo-xwin, zipped and
              signed: what the Windows updater downloads), staged in
              var/dist/vX.Y.Z (skipped when the signed files of this version
              are already there). Arch users get the AUR package (make aur).
  4. tag      commit `release: vX.Y.Z`, tag it, push master and the tag
  5. publish  release vX.Y.Z on thoxy/dayz-community-hub with every file,
              `latest.json` on it and on the rolling `latest` release, which is
              the endpoint installed copies poll; then the same release on the
              GitHub mirror (Thoxy67/dayz-community-hub). Both forges are
              required; `--skip-github` publishes to the Forgejo one only.

The notes are the `## x.y.z` section of CHANGELOG.md when one was written by
hand, else the jj log since the last tag (scripts/changelog.sh); they become
the release body on both forges and the `notes` of latest.json, which the
update banner shows.

A preview (`--pre`, `make prerelease`) builds and publishes the same files as
a Forgejo (and GitHub) pre-release `vX.Y.Z-pre` but touches nothing in the
repository and never `latest.json`: the updater has no preview channel, so a
preview is for downloading by hand. Publishing the same preview again replaces
it, and a new preview or release removes the previews before it.

Secrets, each from the environment, else apps/gui/.env, else ~/.config:

  signing key   TAURI_SIGNING_KEY_FILE    ~/.config/dayz-community-hub/updater.key
  key password  TAURI_SIGNING_KEY_PASS    (empty when unset)
  Forgejo       FORGEJO_TOKEN             ~/.config/dayz-community-hub/forge-token
  GitHub        GH_PAT or GITHUB_TOKEN    ~/.config/dayz-community-hub/github-token
"""

from __future__ import annotations

import argparse
import datetime as dt
import glob
import json
import mimetypes
import os
import re
import shutil
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
import uuid
import zipfile

from dzch import paths

REPO = str(paths.REPO)
GUI = str(paths.GUI)
CARGO = os.path.join(REPO, "Cargo.toml")
TAURI_CONF = os.path.join(REPO, "apps", "gui", "src-tauri", "tauri.conf.json")
PACKAGE_JSON = os.path.join(REPO, "apps", "gui", "package.json")
CHANGELOG = os.path.join(REPO, "CHANGELOG.md")
DOTENV = os.path.join(GUI, ".env")
DIST = str(paths.DIST)

# The Linux build names its target triple (so it can go through zig, see
# scripts/cargo-zigbuild.sh), which moves cargo's output, bundles included,
# under target/<triple>/.
LINUX_TARGET = "x86_64-unknown-linux-gnu"
WINDOWS_TARGET = "x86_64-pc-windows-msvc"
LINUX_BUNDLE = os.path.join(REPO, "target", LINUX_TARGET, "release", "bundle")
WINDOWS_EXE = os.path.join(
    REPO, "target", WINDOWS_TARGET, "release", "dayz-community-hub.exe"
)

FORGE = "https://git.thoxy.xyz"
OWNER = "thoxy"
NAME = "dayz-community-hub"
GITHUB_OWNER = "Thoxy67"
BRANCH = "master"
CONFIG_DIR = os.path.expanduser("~/.config/dayz-community-hub")

# What a release in progress leaves in the working copy: the version, the
# notes, and the lock file the build rewrites for the new version. Anything
# else there is somebody's unfinished work, and a release must not carry it.
RELEASE_FILES = {
    "Cargo.toml",
    "Cargo.lock",
    "CHANGELOG.md",
    "apps/gui/package.json",
    "apps/gui/src-tauri/tauri.conf.json",
}


# ── pure parts, tested in test_release.py ───────────────────────────────────


def read_dotenv(text: str) -> dict[str, str]:
    """`KEY=value` lines, as apps/gui/.env holds them: comments and blank
    lines skipped, one level of matching quotes removed."""
    out = {}
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, _, value = line.partition("=")
        value = value.strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in "'\"":
            value = value[1:-1]
        out[key.strip()] = value
    return out


def setting(
    names: tuple[str, ...],
    env: dict[str, str],
    dotenv: dict[str, str],
    file: str | None = None,
) -> str:
    """The first of `names` set (non-empty) in the environment, then in
    apps/gui/.env, then the contents of `file`; "" when none is."""
    for source in (env, dotenv):
        for name in names:
            value = source.get(name, "").strip()
            if value:
                return value
    if file and os.path.isfile(file):
        return open(file, encoding="utf-8").read().strip()
    return ""


def key_path(
    env: dict[str, str], dotenv: dict[str, str], config_dir: str = CONFIG_DIR
) -> str:
    """Where the updater's signing key is: `TAURI_SIGNING_KEY_FILE` (the name
    apps/gui/.env.example has always used), else the config directory."""
    named = setting(("TAURI_SIGNING_KEY_FILE",), env, dotenv)
    return (
        os.path.expanduser(named) if named else os.path.join(config_dir, "updater.key")
    )


def read_version(cargo_toml: str) -> str:
    """`[workspace.package] version`."""
    m = re.search(
        r'^\[workspace\.package\][^\[]*?^version = "([^"]+)"',
        cargo_toml,
        re.MULTILINE | re.DOTALL,
    )
    if not m:
        raise ValueError("no [workspace.package] version in Cargo.toml")
    return m.group(1)


def parse(version: str) -> tuple[int, int, int]:
    m = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)", version)
    if not m:
        raise ValueError(f"{version!r} is not x.y.z")
    return int(m.group(1)), int(m.group(2)), int(m.group(3))


def bump(version: str) -> str:
    major, minor, patch = parse(version)
    return f"{major}.{minor}.{patch + 1}"


def set_cargo_version(cargo_toml: str, version: str) -> str:
    return re.sub(
        r'(^\[workspace\.package\][^\[]*?^version = ")[^"]+(")',
        rf"\g<1>{version}\g<2>",
        cargo_toml,
        count=1,
        flags=re.MULTILINE | re.DOTALL,
    )


def set_json_version(text: str, version: str) -> str:
    """The first `"version": "…"` of a JSON file (tauri.conf.json,
    package.json), edited as text so the file keeps its layout."""
    return re.sub(r'("version":\s*")[^"]+(")', rf"\g<1>{version}\g<2>", text, count=1)


def next_version(
    current: str, tags: set[str], forced: str | None, in_progress: bool, published: bool
) -> str:
    """The version this run releases.

    A version is bumped only once it is out: the current one if it was never
    tagged (a run resumed after its build), or tagged but not yet on the forge
    (a run resumed after its upload failed); the patch after it otherwise.
    """
    if forced:
        parse(forced)
        if f"v{forced}" in tags:
            raise ValueError(f"v{forced} is already released")
        return forced
    if in_progress or f"v{current}" not in tags or not published:
        return current
    return bump(current)


def preview_version(current: str, tags: set[str], forced: str | None) -> str:
    """A preview's version: the one asked for, which must be a pre-release,
    else `<next version>-pre`, the next being the current one while it is
    unreleased."""
    if forced:
        if not re.fullmatch(r"\d+\.\d+\.\d+-[0-9A-Za-z.-]+", forced):
            raise ValueError(
                f"{forced!r} is not a pre-release version (x.y.z-something)"
            )
        return forced
    base = current if f"v{current}" not in tags else bump(current)
    return f"{base}-pre"


def last_tag(tags: set[str]) -> str | None:
    released = [t for t in tags if re.fullmatch(r"v\d+\.\d+\.\d+", t)]
    return max(released, key=lambda t: parse(t[1:])) if released else None


def old_previews(tags: list[str], published: str) -> list[str]:
    """The previews nobody needs once `published` is out: every `vX.Y.Z-…`
    pre-release but the one just published. Stable releases are never
    removed: they are what the README, the AUR package and old copies link to."""
    return [
        t
        for t in tags
        if t != published and re.fullmatch(r"v\d+\.\d+\.\d+-[0-9A-Za-z.-]+", t)
    ]


def _section(changelog: str, version: str) -> re.Match | None:
    """The `## <version>` heading of a section, dated or not."""
    return re.search(
        rf"^## {re.escape(version)}(?: - [^\n]*)?$", changelog, re.MULTILINE
    )


def _body(changelog: str, m: re.Match) -> str:
    end = changelog.find("\n## ", m.end())
    return changelog[m.end() : end if end != -1 else len(changelog)].strip()


def _unreleased(changelog: str) -> re.Match | None:
    return re.search(r"^## Unreleased[ \t]*$", changelog, re.MULTILINE)


def hand_notes(changelog: str, version: str) -> str | None:
    """The notes written by hand for `version` in CHANGELOG.md: its own
    `## x.y.z` section, else the `## Unreleased` one; None when neither has
    anything, and the jj log is used instead."""
    m = _section(changelog, version)
    if m:
        return _body(changelog, m) or None
    u = _unreleased(changelog)
    return (_body(changelog, u) or None) if u else None


def with_release_notes(changelog: str, version: str, date: str, notes: str) -> str:
    """CHANGELOG.md with this release's section added: under the hand-kept
    `## Unreleased` block, above the releases before it, and not twice. A
    section already written by hand for the version is only given its date."""
    m = _section(changelog, version)
    if m:
        if m.group(0) == f"## {version}":
            return (
                changelog[: m.start()] + f"## {version} - {date}" + changelog[m.end() :]
            )
        return changelog
    u = _unreleased(changelog)
    if u and _body(changelog, u):
        # The notes were written under Unreleased: that section becomes the
        # release's, and a fresh empty Unreleased goes above it.
        return (
            changelog[: u.start()]
            + f"## Unreleased\n\n## {version} - {date}"
            + changelog[u.end() :]
        )
    section = f"## {version} - {date}\n\n{notes.strip()}\n\n"
    unreleased = changelog.find("## Unreleased")
    if unreleased == -1:
        first = changelog.find("\n## ")
        at = first + 1 if first != -1 else len(changelog)
    else:
        after = changelog.find("\n## ", unreleased + 1)
        at = after + 1 if after != -1 else len(changelog)
        if at == len(changelog) and not changelog.endswith("\n\n"):
            section = "\n" + section
    return changelog[:at] + section + changelog[at:]


def asset_names(version: str) -> dict[str, str]:
    """The published file name of each artifact, by role. The names carry the
    tag (as they always have: the AUR package and old links rely on it)."""
    stem = f"{NAME}-v{version}-x86_64"
    return {
        "windows": f"{stem}-windows.zip",
        "windows_sig": f"{stem}-windows.zip.sig",
        "appimage": f"{stem}.AppImage",
        "appimage_sig": f"{stem}.AppImage.sig",
    }


def asset_url(version: str, name: str) -> str:
    return f"{FORGE}/{OWNER}/{NAME}/releases/download/v{version}/{urllib.parse.quote(name)}"


def manifest(
    version: str, notes: str, windows_sig: str, linux_sig: str, now: dt.datetime
) -> dict:
    """`latest.json` as the updater reads it: the Windows zip (what
    src-tauri's updater downloads, verifies and unpacks) and the AppImage."""
    names = asset_names(version)
    return {
        "version": version,
        "notes": notes.strip(),
        "pub_date": now.astimezone(dt.UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": {
            "windows-x86_64": {
                "signature": windows_sig.strip(),
                "url": asset_url(version, names["windows"]),
            },
            "linux-x86_64": {
                "signature": linux_sig.strip(),
                "url": asset_url(version, names["appimage"]),
            },
        },
    }


def signed_pair_ready(path: str, sig: str) -> bool:
    """Whether a file and its signature are there and belong together: a file
    newer than its signature has been rebuilt since, and publishing that pair
    would ship an update every client refuses."""
    if not (os.path.isfile(path) and os.path.isfile(sig)):
        return False
    return os.path.getmtime(sig) >= os.path.getmtime(path)


def staged_complete(stage: str, version: str) -> bool:
    names = asset_names(version)
    if not all(os.path.isfile(os.path.join(stage, n)) for n in names.values()):
        return False
    return signed_pair_ready(
        os.path.join(stage, names["windows"]), os.path.join(stage, names["windows_sig"])
    ) and signed_pair_ready(
        os.path.join(stage, names["appimage"]),
        os.path.join(stage, names["appimage_sig"]),
    )


def plan(version: str, tag: str, notes: str, pre: bool, skip_github: bool) -> str:
    """What a run would do, for `--dry-run`: every file, every upload and the
    manifest, and nothing else touched."""
    names = asset_names(version)
    forges = [f"{FORGE}/{OWNER}/{NAME}"] + (
        [] if skip_github else [f"https://github.com/{GITHUB_OWNER}/{NAME}"]
    )
    lines = [
        f"version      {version}{' (preview: nothing in the repository changes)' if pre else ''}",
        f"stage        {os.path.join(DIST, 'v' + version)}",
        "build        Linux AppImage (zig runner, signed by tauri)",
        "             Windows exe (cargo-xwin), zipped and signed",
        "files        " + "\n             ".join(names.values()),
    ]
    if not pre:
        lines += [
            f"repository   version -> {version} in Cargo.toml, tauri.conf.json, package.json;",
            f"             CHANGELOG.md section; commit `release: {tag}`; tag {tag}; push {BRANCH} + tag",
        ]
    for forge in forges:
        lines.append(
            f"publish      {forge}/releases/tag/{tag}"
            + (" (pre-release)" if pre else "")
        )
    if not pre:
        lines.append(
            f"updater      latest.json on {tag} and on the rolling `latest` release:"
        )
        lines.append(
            json.dumps(
                manifest(
                    version,
                    notes,
                    "<zip signature>",
                    "<AppImage signature>",
                    dt.datetime.now(dt.UTC),
                ),
                indent=2,
            )
        )
    lines += ["notes", notes]
    return "\n".join(lines)


# ── the repository ──────────────────────────────────────────────────────────


def sh(
    *argv: str, cwd: str = REPO, env: dict | None = None, capture: bool = True
) -> str:
    # check=False: a failure is reported with the captured output, which is
    # more useful here than the traceback CalledProcessError would raise.
    r = subprocess.run(
        argv, cwd=cwd, env=env, text=True, capture_output=capture, check=False
    )
    if r.returncode != 0:
        detail = (r.stderr or r.stdout or "").strip() if capture else ""
        sys.exit(f"`{' '.join(argv)}` failed{': ' + detail if detail else ''}")
    return r.stdout if capture else ""


def is_jj() -> bool:
    return shutil.which("jj") is not None and os.path.isdir(os.path.join(REPO, ".jj"))


def changed_files() -> set[str]:
    if is_jj():
        out = sh("jj", "diff", "--name-only", "--no-pager")
    else:
        out = sh("git", "status", "--porcelain", "--untracked-files=all")
        out = "\n".join(line[3:].split(" -> ")[-1] for line in out.splitlines())
    return {line.strip() for line in out.splitlines() if line.strip()}


def tags() -> set[str]:
    return set(sh("git", "tag", "--list", "v*").split())


def source_line() -> str:
    """The change a build is made from, for a preview's notes."""
    if is_jj():
        line = sh(
            "jj",
            "log",
            "-r",
            "@-",
            "--no-graph",
            "-T",
            'change_id.short() ++ " " ++ description.first_line()',
        ).strip()
    else:
        line = sh("git", "log", "-1", "--format=%h %s").strip()
    dirty = " (with uncommitted changes)" if changed_files() else ""
    return f"Built from {line}{dirty}."


def commit_and_tag(tag: str, version: str) -> None:
    print(f"== committing and tagging {tag}")
    if is_jj():
        sh("jj", "commit", "--no-pager", "-m", f"release: {tag}")
        commit = sh("jj", "log", "-r", "@-", "--no-graph", "-T", "commit_id").strip()
        sh("jj", "bookmark", "set", BRANCH, "-r", "@-")
        sh("git", "tag", "-a", tag, "-m", f"DayZ Community Hub {version}", commit)
        sh("jj", "git", "push", "--bookmark", BRANCH)
    else:
        sh("git", "commit", "-am", f"release: {tag}")
        sh("git", "tag", "-a", tag, "-m", f"DayZ Community Hub {version}")
        sh("git", "push", "origin", BRANCH)
    sh("git", "push", "origin", tag)


# ── the forges ──────────────────────────────────────────────────────────────


class Forge:
    """A Gitea-compatible (Forgejo) or GitHub releases API."""

    def __init__(self, label: str, api: str, uploads: str, tok: str, auth: str):
        self.label, self.base, self.uploads, self.tok, self.auth = (
            label,
            api,
            uploads,
            tok,
            auth,
        )

    @classmethod
    def forgejo(cls, tok: str) -> Forge:
        base = f"{FORGE}/api/v1/repos/{OWNER}/{NAME}"
        return cls(f"{OWNER}/{NAME}", base, base, tok, "token")

    @classmethod
    def github(cls, tok: str) -> Forge:
        return cls(
            f"github {GITHUB_OWNER}/{NAME}",
            f"https://api.github.com/repos/{GITHUB_OWNER}/{NAME}",
            f"https://uploads.github.com/repos/{GITHUB_OWNER}/{NAME}",
            tok,
            "Bearer",
        )

    @property
    def is_github(self) -> bool:
        return "github" in self.base

    def call(
        self, method: str, url: str, body=None, raw: bytes | None = None, ctype=None
    ):
        data = (
            raw
            if raw is not None
            else (json.dumps(body).encode() if body is not None else None)
        )
        if not url.startswith("http"):
            url = self.base + url
        req = urllib.request.Request(url, data=data, method=method)
        req.add_header("Authorization", f"{self.auth} {self.tok}")
        req.add_header("Accept", "application/json")
        if data is not None:
            req.add_header("Content-Type", ctype or "application/json")
        try:
            with urllib.request.urlopen(req, timeout=900) as r:
                text = r.read().decode()
                return json.loads(text) if text else None
        except urllib.error.HTTPError as e:
            if e.code == 404:
                return None
            sys.exit(
                f"{self.label}: {method} {url}: {e.code} {e.read().decode(errors='replace')[:300]}"
            )

    def fetch(self, url: str) -> bytes | None:
        req = urllib.request.Request(url)
        req.add_header("Authorization", f"{self.auth} {self.tok}")
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return r.read()
        except urllib.error.URLError:
            return None

    def by_tag(self, tag: str) -> dict | None:
        return self.call("GET", f"/releases/tags/{urllib.parse.quote(tag)}")

    def release(
        self, tag: str, name: str, body: str, pre: bool = False, target: str = BRANCH
    ) -> dict:
        rel = self.by_tag(tag)
        if rel is None:
            rel = self.call(
                "POST",
                "/releases",
                {
                    "tag_name": tag,
                    "name": name,
                    "body": body,
                    "target_commitish": target,
                    "draft": False,
                    "prerelease": pre,
                },
            )
            print(f"  {self.label} release {tag}: created")
        elif rel.get("body") != body:
            rel = self.call("PATCH", f"/releases/{rel['id']}", {"body": body})
            print(f"  {self.label} release {tag}: notes updated")
        return rel

    def assets(self, rel: dict) -> list[dict]:
        if self.is_github:
            return self.call("GET", f"/releases/{rel['id']}/assets?per_page=100") or []
        return rel.get("assets") or []

    def published(self, version: str) -> bool:
        """Whether `version` is fully out: its release carries every file, and
        the rolling `latest.json` names it."""
        rel = self.by_tag(f"v{version}")
        if rel is None:
            return False
        names = {a["name"] for a in self.assets(rel)}
        if not set(asset_names(version).values()) <= names:
            return False
        rolling = self.by_tag("latest")
        for a in self.assets(rolling or {}):
            if a["name"] == "latest.json":
                raw = self.fetch(a["browser_download_url"])
                try:
                    return raw is not None and json.loads(raw).get("version") == version
                except ValueError:
                    return False
        return False

    def delete_asset(self, rel: dict, asset: dict) -> None:
        if self.is_github:
            self.call("DELETE", f"/releases/assets/{asset['id']}")
        else:
            self.call("DELETE", f"/releases/{rel['id']}/assets/{asset['id']}")

    def upload(
        self, rel: dict, path: str, name: str | None = None, replace: bool = False
    ) -> None:
        name = name or os.path.basename(path)
        size = os.path.getsize(path)
        for a in self.assets(rel):
            if a["name"] != name:
                continue
            if a["size"] == size and not replace:
                print(f"  {self.label} {name}: already there")
                return
            self.delete_asset(rel, a)
        with open(path, "rb") as fh:
            payload = fh.read()
        print(f"  {self.label} {name}: uploading {size / 1e6:.1f} MB")
        query = f"?name={urllib.parse.quote(name)}"
        if self.is_github:
            self.call(
                "POST",
                f"{self.uploads}/releases/{rel['id']}/assets{query}",
                raw=payload,
                ctype="application/octet-stream",
            )
            return
        boundary = uuid.uuid4().hex
        ctype = mimetypes.guess_type(name)[0] or "application/octet-stream"
        body = (
            (
                f'--{boundary}\r\nContent-Disposition: form-data; name="attachment"; '
                f'filename="{name}"\r\nContent-Type: {ctype}\r\n\r\n'
            ).encode()
            + payload
            + f"\r\n--{boundary}--\r\n".encode()
        )
        self.call(
            "POST",
            f"/releases/{rel['id']}/assets{query}",
            raw=body,
            ctype=f"multipart/form-data; boundary={boundary}",
        )

    def releases(self) -> list[dict]:
        out, page = [], 1
        size = "per_page" if self.is_github else "limit"
        while True:
            batch = self.call("GET", f"/releases?{size}=50&page={page}") or []
            out += batch
            if len(batch) < 50:
                return out
            page += 1

    def drop_previews(self, published: str) -> None:
        """Delete the previews `old_previews` names, and their tags, once
        `published` is fully up."""
        by_tag = {r["tag_name"]: r for r in self.releases()}
        gone = old_previews(sorted(by_tag), published)
        for tag in gone:
            self.call("DELETE", f"/releases/{by_tag[tag]['id']}")
            path = (
                f"/git/refs/tags/{tag}"
                if self.is_github
                else f"/tags/{urllib.parse.quote(tag)}"
            )
            self.call("DELETE", path)
        if gone:
            print(f"  {self.label}: removed {', '.join(gone)}")


# ── the steps ───────────────────────────────────────────────────────────────


class Secrets:
    def __init__(self):
        env = dict(os.environ)
        dotenv = (
            read_dotenv(open(DOTENV, encoding="utf-8").read())
            if os.path.isfile(DOTENV)
            else {}
        )
        self.key = key_path(env, dotenv)
        self.key_pass = setting(
            ("TAURI_SIGNING_KEY_PASS", "TAURI_SIGNING_PRIVATE_KEY_PASSWORD"),
            env,
            dotenv,
        )
        self.forgejo = setting(
            ("FORGEJO_TOKEN", "GITEA_TOKEN"),
            env,
            dotenv,
            os.path.join(CONFIG_DIR, "forge-token"),
        )
        self.github = setting(
            ("GH_PAT", "GITHUB_TOKEN"),
            env,
            dotenv,
            os.path.join(CONFIG_DIR, "github-token"),
        )

    def signing_env(self) -> dict[str, str]:
        return dict(
            os.environ,
            TAURI_SIGNING_PRIVATE_KEY=open(self.key, encoding="utf-8").read(),
            TAURI_SIGNING_PRIVATE_KEY_PASSWORD=self.key_pass,
        )


def tauri(*args: str, env: dict) -> None:
    sh("bun", "run", "tauri", *args, cwd=GUI, env=env, capture=False)


def build_linux(version: str, secrets: Secrets, config: dict, stage: str) -> None:
    """The AppImage (signed by tauri: createUpdaterArtifacts), linked through
    zig against a glibc floor (scripts/cargo-zigbuild.sh falls back to plain
    cargo when zig is missing, and honours ZIG=0)."""
    print(f"== building Linux {version}")
    env = dict(secrets.signing_env(), NO_STRIP="true")
    tauri(
        "build",
        "--runner",
        os.path.join(REPO, "scripts", "cargo-zigbuild.sh"),
        "--target",
        LINUX_TARGET,
        "--bundles",
        "appimage",
        "--config",
        json.dumps(config),
        env=env,
    )
    names = asset_names(version)

    def one(kind: str, pattern: str) -> str:
        found = sorted(
            glob.glob(os.path.join(LINUX_BUNDLE, kind, pattern)), key=os.path.getmtime
        )
        if not found:
            sys.exit(
                f"the Linux build left no {pattern} in {os.path.join(LINUX_BUNDLE, kind)}"
            )
        return found[-1]

    # Tauri names bundles after the product name and version.
    appimage = one("appimage", f"*_{version}_*.AppImage")
    for src, dst in (
        (appimage, names["appimage"]),
        (appimage + ".sig", names["appimage_sig"]),
    ):
        if not os.path.isfile(src):
            sys.exit(f"the Linux build left no {src} (is the signing key right?)")
        shutil.copy2(src, os.path.join(stage, dst))


def build_windows(version: str, secrets: Secrets, config: dict, stage: str) -> None:
    """The .exe through cargo-xwin, zipped and signed: the updater downloads
    the zip, checks its minisign signature against tauri.conf.json's pubkey
    and replaces the running executable with the one inside."""
    print(f"== building Windows {version}")
    tauri(
        "build",
        "--runner",
        "cargo-xwin",
        "--target",
        WINDOWS_TARGET,
        "--no-bundle",
        "--config",
        json.dumps(config),
        env=dict(
            os.environ,
            TAURI_SIGNING_PRIVATE_KEY="",
            TAURI_SIGNING_PRIVATE_KEY_PASSWORD="",
        ),
    )
    if not os.path.isfile(WINDOWS_EXE):
        sys.exit(f"the Windows build left no {WINDOWS_EXE}")
    names = asset_names(version)
    zip_path = os.path.join(stage, names["windows"])
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        z.write(WINDOWS_EXE, os.path.basename(WINDOWS_EXE))
    sig = zip_path + ".sig"
    if os.path.exists(sig):
        os.remove(sig)
    tauri("signer", "sign", zip_path, env=secrets.signing_env())
    if not signed_pair_ready(zip_path, sig):
        sys.exit(f"signing left no {sig}")


def build(version: str, secrets: Secrets, pre: bool) -> str:
    """Every file of `version`, staged in var/dist/v<version>/. A preview's
    version is given to the build rather than written to the files: it is not
    a version of the source."""
    stage = os.path.join(DIST, f"v{version}")
    if not pre and staged_complete(stage, version):
        print(f"== v{version} is built and signed already ({stage})")
        return stage
    os.makedirs(stage, exist_ok=True)
    config: dict = {"bundle": {"createUpdaterArtifacts": True}}
    if pre:
        config["version"] = version
    build_linux(version, secrets, config, stage)
    build_windows(version, secrets, config, stage)
    if not staged_complete(stage, version):
        sys.exit(f"the build finished without every file in {stage}")
    return stage


def write_manifest(stage: str, version: str, notes: str) -> str:
    names = asset_names(version)
    path = os.path.join(stage, "latest.json")
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(
            manifest(
                version,
                notes,
                open(
                    os.path.join(stage, names["windows_sig"]), encoding="utf-8"
                ).read(),
                open(
                    os.path.join(stage, names["appimage_sig"]), encoding="utf-8"
                ).read(),
                dt.datetime.now(dt.UTC),
            ),
            fh,
            indent=2,
        )
    return path


def publish_files(
    forge: Forge, tag: str, title: str, notes: str, stage: str, version: str, pre: bool
) -> dict:
    rel = forge.release(tag, title, notes, pre=pre)
    for name in asset_names(version).values():
        forge.upload(rel, os.path.join(stage, name), name, replace=pre)
    return rel


def mirrors(secrets: Secrets, skip_github: bool) -> list[Forge]:
    if skip_github:
        print("note: --skip-github: the GitHub mirror is not published")
        return []
    return [Forge.github(secrets.github)]


def publish_preview(cli, secrets: Secrets, forgejo: Forge) -> None:
    current = read_version(open(CARGO, encoding="utf-8").read())
    all_tags = tags()
    version = preview_version(current, all_tags, cli.version)
    tag = f"v{version}"
    since = last_tag(all_tags)
    log = sh("scripts/changelog.sh", since).strip() if since else ""
    notes = "\n\n".join(x for x in (source_line(), log) if x)
    print(f"== preview {tag} (current {current}, last release {since or 'none'})")
    if cli.dry_run:
        print(plan(version, tag, notes, pre=True, skip_github=cli.skip_github))
        return
    stage = build(version, secrets, pre=True)
    title = f"DayZ Community Hub {version} (preview)"
    for forge in [forgejo, *mirrors(secrets, cli.skip_github)]:
        print(f"== publishing the preview to {forge.label}")
        publish_files(forge, tag, title, notes, stage, version, pre=True)
        forge.drop_previews(tag)
    print(f"\ndone: preview {tag} is published (latest.json untouched)")


def main() -> None:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--version", help="release this version rather than the next patch")
    ap.add_argument(
        "--pre",
        action="store_true",
        help="a preview: no commit, no tag, no latest.json; the same version again replaces it",
    )
    ap.add_argument(
        "--dry-run", action="store_true", help="say what would happen, change nothing"
    )
    ap.add_argument(
        "--skip-github",
        action="store_true",
        help="publish to git.thoxy.xyz only, not to the GitHub mirror",
    )
    cli = ap.parse_args()

    secrets = Secrets()
    missing = []
    if not os.path.isfile(secrets.key):
        missing.append(
            f'no signing key at {secrets.key}; `make keys` makes one (docs/build.md, "Publish")'
        )
    if not secrets.forgejo:
        missing.append(
            f"no Forgejo token: set FORGEJO_TOKEN or write one to {CONFIG_DIR}/forge-token"
        )
    if not secrets.github and not cli.skip_github:
        missing.append(
            "no GitHub token: set GH_PAT or write one to "
            f"{CONFIG_DIR}/github-token (or pass --skip-github to publish to git.thoxy.xyz only)"
        )
    if missing and not cli.dry_run:
        sys.exit("\n".join(missing))
    for line in missing:
        print(f"warning: {line}")
    forgejo = Forge.forgejo(secrets.forgejo)
    if cli.pre:
        publish_preview(cli, secrets, forgejo)
        return
    if cli.version and "-" in cli.version:
        sys.exit(f"{cli.version} is a pre-release version: use make prerelease")

    # 1. check
    changed = changed_files()
    stray = changed - RELEASE_FILES
    if stray:
        message = (
            "the working copy has changes a release must not carry; commit or move them first:\n  "
            + "\n  ".join(sorted(stray))
        )
        if not cli.dry_run:
            sys.exit(message)
        print(f"warning: {message}")

    # 2. version
    cargo = open(CARGO, encoding="utf-8").read()
    current = read_version(cargo)
    all_tags = tags()
    version = next_version(
        current,
        all_tags,
        cli.version,
        in_progress=bool(changed),
        published=f"v{current}" not in all_tags
        or (forgejo.published(current) if secrets.forgejo else True),
    )
    tag = f"v{version}"
    since = last_tag(all_tags - {tag})
    notes = hand_notes(open(CHANGELOG, encoding="utf-8").read(), version) or (
        (sh("scripts/changelog.sh", since).strip() if since else "First release.")
        or "No changes worth a line."
    )
    print(f"== releasing {tag} (current {current}, last release {since or 'none'})")
    if cli.dry_run:
        print(plan(version, tag, notes, pre=False, skip_github=cli.skip_github))
        return
    if tag not in all_tags:
        if version != current:
            with open(CARGO, "w", encoding="utf-8") as fh:
                fh.write(set_cargo_version(cargo, version))
            for path in (TAURI_CONF, PACKAGE_JSON):
                text = open(path, encoding="utf-8").read()
                with open(path, "w", encoding="utf-8") as fh:
                    fh.write(set_json_version(text, version))
            print(f"  version {current} -> {version}")
        log = open(CHANGELOG, encoding="utf-8").read()
        with open(CHANGELOG, "w", encoding="utf-8") as fh:
            fh.write(
                with_release_notes(log, version, dt.date.today().isoformat(), notes)
            )

    # 3. build
    stage = build(version, secrets, pre=False)

    # 4. tag
    if tag not in all_tags:
        commit_and_tag(tag, version)

    # 5. publish
    latest = write_manifest(stage, version, notes)
    title = f"DayZ Community Hub {version}"
    for forge in [forgejo, *mirrors(secrets, cli.skip_github)]:
        print(f"== publishing to {forge.label}")
        rel = publish_files(forge, tag, title, notes, stage, version, pre=False)
        forge.upload(rel, latest, "latest.json", replace=True)
        rolling = forge.release(
            "latest",
            "latest",
            "Always points to the latest release. Used by the auto-updater.",
        )
        forge.upload(rolling, latest, "latest.json", replace=True)
        forge.drop_previews(tag)

    print(f"\ndone: {tag} is published")
    print(f"  release : {FORGE}/{OWNER}/{NAME}/releases/tag/{tag}")
    print(f"  updater : {FORGE}/{OWNER}/{NAME}/releases/download/latest/latest.json")


if __name__ == "__main__":
    main()
