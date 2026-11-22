"""Tests for analysis_engine: walls flagged interior stay out of the
orientation and WWR analysis. Needs no extra packages.

    python -m unittest discover -s pyrevit-extension/tests
"""
import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "pyrevit-extension", "Makit.extension", "lib"))

import analysis_engine
from geometry_models import BuildingModel


def wall(id, x, y, area, external, windows=()):
    data = {
        "id": id, "orientation": {"x": x, "y": y, "z": 0.0}, "area": area,
        "windows": [{"id": w, "area": a} for w, a in windows],
    }
    if external is not None:
        data["isExternal"] = external
    return data


class InteriorWallsTest(unittest.TestCase):

    def setUp(self):
        self.model = {"walls": [
            wall("out", 0.0, 1.0, 10.0, True, [("a", 2.0)]),
            wall("in", 0.0, -1.0, 5.0, False, [("b", 1.0)]),
            wall("unknown", 1.0, 0.0, 4.0, None),
        ], "projectNorth": 0.0, "units": "m"}

    def test_walls_flagged_interior_and_their_windows_are_left_out(self):
        stats = analysis_engine.analyze_wall_orientations(self.model)
        self.assertEqual(stats["totals"]["wallCount"], 2)
        self.assertAlmostEqual(stats["totals"]["wallArea"], 14.0)
        self.assertEqual(stats["totals"]["windowCount"], 1)
        self.assertAlmostEqual(stats["totals"]["windowArea"], 2.0)
        self.assertEqual(stats["byDirection"]["South"]["wallCount"], 0)
        self.assertEqual(stats["byDirection"]["East"]["wallCount"], 1)
        self.assertEqual(stats["totals"]["interiorWallsLeftOut"], 1)

    def test_the_flag_survives_a_round_trip(self):
        model = BuildingModel.from_dict(self.model)
        walls = {w["id"]: w for w in model.to_dict()["walls"]}
        self.assertIs(walls["out"]["isExternal"], True)
        self.assertIs(walls["in"]["isExternal"], False)
        self.assertIsNone(walls["unknown"]["isExternal"])


if __name__ == "__main__":
    unittest.main()
