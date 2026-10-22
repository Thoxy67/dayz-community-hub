"""keys.py without running the signer."""

import datetime as dt
import tempfile
import unittest
from pathlib import Path

from dzch.keys import keys as k


class RustPubkey(unittest.TestCase):
    SRC = 'x\npub(crate) const PUBKEY: &str = "OLD";\nconst OTHER: &str = "OLD";\n'

    def test_only_the_constant_is_replaced(self):
        out = k.with_rust_pubkey(self.SRC, "NEW")
        self.assertEqual(out, self.SRC.replace('= "OLD";\nconst', '= "NEW";\nconst'))

    def test_the_shipped_updater_has_the_constant(self):
        src = k.UPDATER.read_text(encoding="utf-8")
        self.assertEqual(k.with_rust_pubkey(src, "X").count('PUBKEY: &str = "X";'), 1)

    def test_a_source_without_it_is_refused(self):
        with self.assertRaises(ValueError):
            k.with_rust_pubkey("nothing here", "X")


class Backup(unittest.TestCase):
    def test_an_existing_key_is_moved_aside_never_deleted(self):
        with tempfile.TemporaryDirectory() as d:
            key = Path(d) / "updater.key"
            key.write_text("secret")
            now = dt.datetime(2026, 9, 29, 12, 0, 0, tzinfo=dt.UTC)
            moved = k.backup(key, now)
            self.assertEqual(moved, Path(d) / "updater.key.bak-20260929-120000")
            self.assertFalse(key.exists())
            self.assertEqual(moved.read_text(), "secret")

    def test_nothing_to_back_up(self):
        with tempfile.TemporaryDirectory() as d:
            self.assertIsNone(k.backup(Path(d) / "none", dt.datetime.now(dt.UTC)))

    def test_the_config_dir_can_be_overridden(self):
        import os

        old = os.environ.get("DZCH_CONFIG_DIR")
        os.environ["DZCH_CONFIG_DIR"] = "/tmp/x"
        try:
            self.assertEqual(k.config_dir(), Path("/tmp/x"))
        finally:
            if old is None:
                del os.environ["DZCH_CONFIG_DIR"]
            else:
                os.environ["DZCH_CONFIG_DIR"] = old


if __name__ == "__main__":
    unittest.main()
