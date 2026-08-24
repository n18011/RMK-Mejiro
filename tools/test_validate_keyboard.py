import importlib.util
import re
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


class ValidateMejiroLayoutTests(unittest.TestCase):
    def test_rejects_a_moved_mejiro_key(self):
        config = (Path(__file__).parents[1] / "keyboard.toml").read_text(encoding="utf-8")
        layout = VALIDATOR.section(config, "layout")
        map_rows = VALIDATOR.matrix_map(layout)
        blocks = dict(
            re.findall(
                r'\[\[keymap\.layer\]\]\s*\nname = "([^"]+)"\s*\nkeys = """(.*?)"""',
                config,
                re.S,
            )
        )
        rows = VALIDATOR.layer_rows(blocks["base"])
        rows[0][0], rows[0][1] = rows[0][1], rows[0][0]
        with self.assertRaises(ValueError):
            VALIDATOR.validate_mejiro_layout(rows, map_rows)

    def test_mejiro_is_the_default_base_layer(self):
        config = (Path(__file__).parents[1] / "keyboard.toml").read_text(encoding="utf-8")
        names = re.findall(r'\[\[keymap\.layer\]\]\s*\nname = "([^"]+)"', config)
        self.assertEqual(names, list(VALIDATOR.EXPECTED_LAYER_NAMES))
        self.assertEqual(names[0], "base")

    def test_base_keeps_qwerty_layer_taps(self):
        config = (Path(__file__).parents[1] / "keyboard.toml").read_text(encoding="utf-8")
        blocks = dict(
            re.findall(
                r'\[\[keymap\.layer\]\]\s*\nname = "([^"]+)"\s*\nkeys = """(.*?)"""',
                config,
                re.S,
            )
        )
        self.assertRegex(blocks["base"], r"\bLT\(1,Space\)")
        self.assertRegex(blocks["base"], r"\bLT\(2,Enter\)")
        self.assertRegex(blocks["base"], r"\bLT\(8,Escape\)")

    def test_rejects_keycodes_outside_rmk_virtual_range(self):
        key_ids = list(range(8, 32))
        key_ids[-1] = 32
        with self.assertRaises(ValueError):
            VALIDATOR.validate_mejiro_key_ids(key_ids)


class ValidateMatrixMapTests(unittest.TestCase):
    def test_rejects_duplicate_matrix_coordinates(self):
        with self.assertRaises(ValueError):
            VALIDATOR.parse_matrix_coordinates(
                [["(0,0,L)", "(0,0,L)"]], logical_rows=1, logical_cols=1
            )

    def test_allows_an_intentionally_unused_matrix_slot(self):
        coordinates = VALIDATOR.parse_matrix_coordinates(
            [["(0,0,L)", "(0,2,R)"]], logical_rows=1, logical_cols=3
        )
        self.assertEqual(coordinates, [(0, 0, "L"), (0, 2, "R")])


class ValidateVialLayoutTests(unittest.TestCase):
    def test_position_objects_do_not_count_as_matrix_keys(self):
        vial = {"layouts": {"keymap": [[{"r": 10}, "0,0", {"y": 0.2}, "0,1"]]}}
        self.assertEqual(VALIDATOR.vial_keymap_coordinates(vial), [(0, 0), (0, 1)])


if __name__ == "__main__":
    unittest.main()
