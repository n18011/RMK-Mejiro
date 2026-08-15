import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("validate_keyboard.py")
SPEC = importlib.util.spec_from_file_location("validate_keyboard", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
VALIDATOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VALIDATOR)


class ValidateMejiroKeyIdsTests(unittest.TestCase):
    def test_accepts_any_unique_set_of_24_virtual_keys(self):
        VALIDATOR.validate_mejiro_key_ids(list(range(8, 32)))

    def test_rejects_duplicate_virtual_keys(self):
        key_ids = list(range(8, 32))
        key_ids[-1] = 30
        with self.assertRaises(ValueError):
            VALIDATOR.validate_mejiro_key_ids(key_ids)

    def test_rejects_keycodes_outside_rmk_virtual_range(self):
        key_ids = list(range(8, 32))
        key_ids[-1] = 32
        with self.assertRaises(ValueError):
            VALIDATOR.validate_mejiro_key_ids(key_ids)


if __name__ == "__main__":
    unittest.main()
