"""The parts of release.py that decide things, without the network or jj.

uv run --project tools pytest
"""

import datetime as dt
import os
import tempfile
import time
import unittest

from dzch.release import release as r

CARGO = """[workspace]
members = ["a"]

[workspace.package]
version = "0.4.1"
edition = "2024"

[workspace.dependencies]
serde = { version = "1" }
"""


class Secrets(unittest.TestCase):
    def test_dotenv_skips_comments_and_strips_quotes(self):
        env = r.read_dotenv(
            '# note\n\nTAURI_SIGNING_KEY_FILE="/k/key"\nFORGEJO_TOKEN=abc\nBAD\n'
        )
        self.assertEqual(
            env, {"TAURI_SIGNING_KEY_FILE": "/k/key", "FORGEJO_TOKEN": "abc"}
        )

    def test_the_environment_wins_over_dotenv_and_file(self):
        with tempfile.NamedTemporaryFile("w", delete=False) as fh:
            fh.write("from-file\n")
        try:
            self.assertEqual(
                r.setting(("T",), {"T": "env"}, {"T": "dot"}, fh.name), "env"
            )
            self.assertEqual(
                r.setting(("T",), {"T": " "}, {"T": "dot"}, fh.name), "dot"
            )
            self.assertEqual(r.setting(("T",), {}, {}, fh.name), "from-file")
            self.assertEqual(r.setting(("T",), {}, {}, "/nonexistent"), "")
        finally:
            os.remove(fh.name)

    def test_the_second_name_is_accepted(self):
        self.assertEqual(
            r.setting(("GH_PAT", "GITHUB_TOKEN"), {"GITHUB_TOKEN": "g"}, {}), "g"
        )

    def test_key_path_named_or_config(self):
        self.assertEqual(
            r.key_path({"TAURI_SIGNING_KEY_FILE": "/a/k"}, {}, "/cfg"), "/a/k"
        )
        self.assertEqual(
            r.key_path({}, {"TAURI_SIGNING_KEY_FILE": "/b/k"}, "/cfg"), "/b/k"
        )
        self.assertEqual(r.key_path({}, {}, "/cfg"), "/cfg/updater.key")


class Versions(unittest.TestCase):
    def test_read_and_set_the_workspace_version_only(self):
        self.assertEqual(r.read_version(CARGO), "0.4.1")
        out = r.set_cargo_version(CARGO, "0.5.0")
        self.assertEqual(r.read_version(out), "0.5.0")
        self.assertIn('serde = { version = "1" }', out)

    def test_json_version_keeps_the_layout(self):
        conf = '{\n  "productName": "x",\n  "version": "0.4.1",\n  "deps": {"version": "9"}\n}'
        out = r.set_json_version(conf, "0.4.2")
        self.assertIn('"version": "0.4.2"', out)
        self.assertIn('"deps": {"version": "9"}', out)

    def test_bump_and_parse(self):
        self.assertEqual(r.bump("0.4.9"), "0.4.10")
        with self.assertRaises(ValueError):
            r.parse("0.4")

    def test_next_version(self):
        tags = {"v0.4.0", "v0.4.1"}
        self.assertEqual(r.next_version("0.4.1", tags, None, False, True), "0.4.2")
        # never tagged, or tagged but not out, or a run in progress: this one again
        self.assertEqual(r.next_version("0.4.2", tags, None, False, True), "0.4.2")
        self.assertEqual(r.next_version("0.4.1", tags, None, False, False), "0.4.1")
        self.assertEqual(r.next_version("0.4.1", tags, None, True, True), "0.4.1")
        self.assertEqual(r.next_version("0.4.1", tags, "0.5.0", False, True), "0.5.0")
        with self.assertRaises(ValueError):
            r.next_version("0.4.1", tags, "0.4.1", False, True)

    def test_preview_version(self):
        self.assertEqual(r.preview_version("0.4.1", {"v0.4.1"}, None), "0.4.2-pre")
        self.assertEqual(r.preview_version("0.4.2", {"v0.4.1"}, None), "0.4.2-pre")
        self.assertEqual(
            r.preview_version("0.4.1", set(), "0.5.0-beta.1"), "0.5.0-beta.1"
        )
        with self.assertRaises(ValueError):
            r.preview_version("0.4.1", set(), "0.5.0")

    def test_last_tag_ignores_previews_and_others(self):
        self.assertEqual(
            r.last_tag({"v0.4.1", "v0.4.10", "v0.5.0-pre", "latest"}), "v0.4.10"
        )
        self.assertIsNone(r.last_tag({"latest"}))

    def test_old_previews_never_names_a_stable_release(self):
        tags = ["latest", "v0.4.0", "v0.4.1-pre", "v0.4.2-pre", "dev-abc1234"]
        self.assertEqual(r.old_previews(tags, "v0.4.2-pre"), ["v0.4.1-pre"])
        self.assertEqual(r.old_previews(tags, "v0.4.2"), ["v0.4.1-pre", "v0.4.2-pre"])


class Changelog(unittest.TestCase):
    LOG = "# Changelog\n\nIntro.\n\n## Unreleased\n\n- wip\n\n## 0.4.1 - 2026-06-01\n\n- old\n"

    def test_hand_notes(self):
        log = self.LOG.replace(
            "## Unreleased", "## 0.4.2\n\n- by hand\n\n## Unreleased"
        )
        self.assertEqual(r.hand_notes(log, "0.4.2"), "- by hand")
        self.assertEqual(r.hand_notes(self.LOG, "0.4.1"), "- old")
        self.assertIsNone(r.hand_notes(self.LOG, "0.9.9"))

    def test_notes_go_under_unreleased_once(self):
        out = r.with_release_notes(self.LOG, "0.4.2", "2026-09-29", "- new")
        self.assertLess(out.index("## Unreleased"), out.index("## 0.4.2 - 2026-09-29"))
        self.assertLess(out.index("## 0.4.2"), out.index("## 0.4.1"))
        self.assertEqual(
            r.with_release_notes(out, "0.4.2", "2026-09-30", "- again"), out
        )

    def test_a_hand_written_section_only_gets_its_date(self):
        log = "# Changelog\n\n## 0.4.2\n\n- by hand\n"
        out = r.with_release_notes(log, "0.4.2", "2026-09-29", "- ignored")
        self.assertEqual(out, "# Changelog\n\n## 0.4.2 - 2026-09-29\n\n- by hand\n")

    def test_without_unreleased_the_section_goes_first(self):
        out = r.with_release_notes(
            "# Changelog\n\n## 0.4.1\n\n- a\n", "0.4.2", "d", "- b"
        )
        self.assertLess(out.index("## 0.4.2"), out.index("## 0.4.1"))


class Manifest(unittest.TestCase):
    def test_both_platforms_point_at_the_versioned_release(self):
        now = dt.datetime(2026, 9, 29, 12, 0, tzinfo=dt.UTC)
        m = r.manifest("0.4.2", " notes \n", "WSIG\n", "LSIG", now)
        self.assertEqual(m["version"], "0.4.2")
        self.assertEqual(m["notes"], "notes")
        self.assertEqual(m["pub_date"], "2026-09-29T12:00:00Z")
        win = m["platforms"]["windows-x86_64"]
        self.assertEqual(win["signature"], "WSIG")
        self.assertEqual(
            win["url"],
            "https://git.thoxy.xyz/thoxy/dayz-community-hub/releases/download/v0.4.2/"
            "dayz-community-hub-v0.4.2-x86_64-windows.zip",
        )
        self.assertTrue(
            m["platforms"]["linux-x86_64"]["url"].endswith("-v0.4.2-x86_64.AppImage")
        )

    def test_asset_names_keep_the_historical_scheme(self):
        names = r.asset_names("0.4.2")
        self.assertEqual(names["deb"], "dayz-community-hub-v0.4.2-x86_64.deb")
        self.assertEqual(
            names["windows_sig"], "dayz-community-hub-v0.4.2-x86_64-windows.zip.sig"
        )


class Staging(unittest.TestCase):
    def test_a_file_newer_than_its_signature_is_not_ready(self):
        with tempfile.TemporaryDirectory() as d:
            f, sig = os.path.join(d, "a"), os.path.join(d, "a.sig")
            self.assertFalse(r.signed_pair_ready(f, sig))
            open(f, "w").close()
            open(sig, "w").close()
            self.assertTrue(r.signed_pair_ready(f, sig))
            later = time.time() + 10
            os.utime(f, (later, later))
            self.assertFalse(r.signed_pair_ready(f, sig))

    def test_staged_complete_needs_every_file(self):
        with tempfile.TemporaryDirectory() as d:
            names = r.asset_names("0.4.2")
            for name in names.values():
                open(os.path.join(d, name), "w").close()
            self.assertTrue(r.staged_complete(d, "0.4.2"))
            os.remove(os.path.join(d, names["rpm"]))
            self.assertFalse(r.staged_complete(d, "0.4.2"))


if __name__ == "__main__":
    unittest.main()
