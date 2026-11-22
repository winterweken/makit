"""Tests for ifc_extractors: wall and window areas must honour the units an
IFC file declares.

Run from the repository root with a Python that has ifcopenshell:

    python -m unittest discover -s pyrevit-extension/tests
"""
import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "pyrevit-extension", "Makit.extension", "lib"))

try:
    import ifcopenshell
    import ifcopenshell.guid
except ImportError:
    ifcopenshell = None

if ifcopenshell is not None:
    import ifc_extractors


def new_file(length_prefix):
    """An empty IFC4 file whose lengths are in METRE with `length_prefix`
    (None for metres, "MILLI" for millimetres) and areas in square metres."""
    f = ifcopenshell.file(schema="IFC4")
    units = f.createIfcUnitAssignment([
        f.createIfcSIUnit(None, "LENGTHUNIT", length_prefix, "METRE"),
        f.createIfcSIUnit(None, "AREAUNIT", None, "SQUARE_METRE"),
    ])
    f.createIfcProject(ifcopenshell.guid.new(), None, "p", None, None, None, None, None, units)
    return f


def add_quantities(f, element, qset, lengths=(), areas=()):
    quantities = [f.createIfcQuantityLength(name, None, None, value, None) for name, value in lengths]
    quantities += [f.createIfcQuantityArea(name, None, None, value, None) for name, value in areas]
    q = f.createIfcElementQuantity(ifcopenshell.guid.new(), None, qset, None, None, quantities)
    f.createIfcRelDefinesByProperties(ifcopenshell.guid.new(), None, None, None, [element], q)


def new_wall(f, **quantities):
    wall = f.createIfcWall(ifcopenshell.guid.new(), None, "w", None, None, None, None, None, None)
    add_quantities(f, wall, "Qto_WallBaseQuantities", **quantities)
    return wall


def new_window(f, **quantities):
    window = f.createIfcWindow(ifcopenshell.guid.new(), None, "win", None, None, None, None, None, None, None, None, None, None)
    add_quantities(f, window, "Qto_WindowBaseQuantities", **quantities)
    return window


@unittest.skipIf(ifcopenshell is None, "needs ifcopenshell")
class WallAreaTest(unittest.TestCase):

    def test_a_side_area_in_squared_length_units_is_not_taken(self):
        # Revit's exporter writes NetSideArea twice in millimetre files:
        # first in mm² (here the gross 3.5 m × 10 m), then in m².
        f = new_file("MILLI")
        wall = new_wall(
            f,
            lengths=[("Height", 3500.0), ("Length", 10000.0), ("Width", 280.0)],
            areas=[("NetSideArea", 35000000.0), ("NetSideArea", 33.2)],
        )
        self.assertAlmostEqual(ifc_extractors.get_ifc_wall_area(wall, f), 33.2)

    def test_height_times_length_is_converted_to_square_metres(self):
        f = new_file("MILLI")
        wall = new_wall(f, lengths=[("Height", 3000.0), ("Length", 4000.0)])
        self.assertAlmostEqual(ifc_extractors.get_ifc_wall_area(wall, f), 12.0)

    def test_a_metre_file_keeps_its_side_area(self):
        f = new_file(None)
        wall = new_wall(
            f,
            lengths=[("Height", 3.0), ("Length", 2.1)],
            areas=[("NetSideArea", 6.3)],
        )
        self.assertAlmostEqual(ifc_extractors.get_ifc_wall_area(wall, f), 6.3)


@unittest.skipIf(ifcopenshell is None, "needs ifcopenshell")
class WindowAreaTest(unittest.TestCase):

    def test_height_times_width_is_converted_to_square_metres(self):
        f = new_file("MILLI")
        window = new_window(f, lengths=[("Height", 1200.0), ("Width", 1000.0)])
        generic = ifc_extractors.extract_ifc_window_to_generic(window, f)
        self.assertAlmostEqual(generic.area, 1.2)

    def test_an_area_quantity_is_kept(self):
        f = new_file("MILLI")
        window = new_window(f, lengths=[("Height", 2700.0), ("Width", 1500.0)], areas=[("Area", 5.519)])
        generic = ifc_extractors.extract_ifc_window_to_generic(window, f)
        self.assertAlmostEqual(generic.area, 5.5)


@unittest.skipIf(ifcopenshell is None, "needs ifcopenshell")
class SampleFileTest(unittest.TestCase):

    def test_the_sample_house_still_extracts_as_documented(self):
        path = os.path.join(ROOT, "examples", "IFC", "Building-Architecture.ifc")
        model = ifc_extractors.extract_building_model_from_ifc(path)
        self.assertEqual(len(model.walls), 4)
        self.assertAlmostEqual(sum(w.area for w in model.walls), 43.3, places=1)
        self.assertEqual(model.metadata["sourceFile"], "Building-Architecture.ifc")


if __name__ == "__main__":
    unittest.main()
