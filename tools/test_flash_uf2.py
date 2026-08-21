import importlib.util
import errno
from pathlib import Path
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).with_name("flash_uf2.py")
SPEC = importlib.util.spec_from_file_location("flash_uf2", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
FLASH = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FLASH)


class FlashUf2Tests(unittest.TestCase):
    def test_parses_unmounted_uf2_volume(self):
        rows = FLASH.parse_lsblk_pairs(
            'PATH="/dev/sda" FSTYPE="vfat" LABEL="XIAO-SENSE" '
            'MOUNTPOINTS=""\n'
        )

        self.assertEqual(rows, [{
            "PATH": "/dev/sda",
            "FSTYPE": "vfat",
            "LABEL": "XIAO-SENSE",
            "MOUNTPOINTS": "",
        }])

    def test_copies_image_and_detects_marker(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "firmware.uf2"
            mount = root / "XIAO-SENSE"
            mount.mkdir()
            (mount / "INFO_UF2.TXT").write_text("UF2 bootloader", encoding="utf-8")
            source.write_bytes(b"UF2 test image")

            self.assertTrue(FLASH.is_uf2_mount(mount))
            destination = FLASH.copy_uf2(source, mount)

            self.assertEqual(destination, mount / source.name)
            self.assertEqual(destination.read_bytes(), source.read_bytes())

    def test_treats_bootloader_disconnect_after_complete_write_as_success(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "firmware.uf2"
            mount = root / "XIAO-SENSE"
            mount.mkdir()
            source.write_bytes(b"UF2 test image")

            with mock.patch.object(FLASH.os, "fsync", side_effect=OSError(errno.EIO, "gone")):
                destination = FLASH.copy_uf2(source, mount)

            self.assertEqual(destination.read_bytes(), source.read_bytes())


if __name__ == "__main__":
    unittest.main()
