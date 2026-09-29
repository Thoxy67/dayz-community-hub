"""aur.py's pure parts."""

import unittest

from dzch.aur import aur as a


class Version(unittest.TestCase):
    def test_pkgver_matches_what_the_pkgbuild_prints(self):
        self.assertEqual(a.pkgver("0.5.0", 123, "abc1234"), "0.5.0.r123.gabc1234")

    def test_a_pre_release_version_has_no_dash(self):
        self.assertEqual(a.pkgver("0.5.0-pre", 1, "a"), "0.5.0_pre.r1.ga")


class Render(unittest.TestCase):
    def test_the_template_gets_the_version(self):
        self.assertEqual(a.render("pkgver=@PKGVER@\n", "1.r2.g3"), "pkgver=1.r2.g3\n")

    def test_a_template_without_the_marker_is_refused(self):
        with self.assertRaises(ValueError):
            a.render("pkgver=1\n", "2")

    def test_the_shipped_template_renders_to_a_pkgbuild(self):
        text = a.render(a.TEMPLATE.read_text(encoding="utf-8"), "0.5.0.r1.gabc")
        self.assertIn("pkgname=dayz-community-hub-git", text)
        self.assertIn("pkgver=0.5.0.r1.gabc", text)
        self.assertIn("apps/gui/src-tauri/tauri.conf.json", text)
        self.assertIn("options=(!lto)", text)
        self.assertNotIn("dayz-community-hub-ui", text)


class Changes(unittest.TestCase):
    def test_only_the_version_moved(self):
        self.assertTrue(a.only_version_moved("a\npkgver=1\nb\n", "a\npkgver=2\nb\n"))
        self.assertFalse(a.only_version_moved("a\npkgver=1\n", "c\npkgver=2\n"))
        self.assertFalse(a.only_version_moved("same", "same"))

    def test_commit_message(self):
        self.assertEqual(a.commit_message("1.r2.g3", "fix x"), "1.r2.g3: fix x")
        self.assertEqual(a.commit_message("1.r2.g3", ""), "1.r2.g3")


if __name__ == "__main__":
    unittest.main()
