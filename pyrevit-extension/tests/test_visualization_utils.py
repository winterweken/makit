"""The standalone visualisation builds wall faces from the extraction's metres."""
import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "pyrevit-extension", "Makit.extension", "lib"))

import geometry_models
import visualization_utils


def wall(start, end, bottom=None, top=None, height=0.0):
    w = geometry_models.GenericWall("w", "wall")
    w.start_point = geometry_models.GeometricVector(*start)
    w.end_point = geometry_models.GeometricVector(*end)
    w.bottom, w.top, w.height = bottom, top, height
    return w


class ProjectedViewTest(unittest.TestCase):

    def test_a_face_stands_from_bottom_to_top(self):
        view = visualization_utils.create_projected_view([wall((0, 0, 5), (4, 0, 5), bottom=0.0, top=3.0)], "South", [])
        points = view["faces"][0]["points"]
        self.assertEqual(points[0], visualization_utils.isometric_project(0, 0, 0))
        self.assertEqual(points[2], visualization_utils.isometric_project(4, 0, 3))

    def test_without_elevations_the_axis_z_and_height_are_used(self):
        view = visualization_utils.create_projected_view([wall((0, 0, 5), (4, 0, 5), height=2.0)], "South", [])
        points = view["faces"][0]["points"]
        self.assertEqual(points[0], visualization_utils.isometric_project(0, 0, 5))
        self.assertEqual(points[3], visualization_utils.isometric_project(0, 0, 7))


if __name__ == "__main__":
    unittest.main()
