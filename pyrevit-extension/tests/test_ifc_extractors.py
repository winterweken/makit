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


def window_in(f, wall, area):
    """A window of `area` square metres filling an opening in `wall`."""
    opening = f.createIfcOpeningElement(ifcopenshell.guid.new(), None, "opening", None, None, None, None, None, None)
    f.createIfcRelVoidsElement(ifcopenshell.guid.new(), None, None, None, wall, opening)
    window = new_window(f, areas=[("Area", area)])
    f.createIfcRelFillsElement(ifcopenshell.guid.new(), None, None, None, opening, window)
    return window


@unittest.skipIf(ifcopenshell is None, "needs ifcopenshell")
class NetWallAreaTest(unittest.TestCase):
    """A wall's area is its opaque area: the analyses add the windows back."""

    def test_a_gross_side_area_loses_its_windows(self):
        f = new_file(None)
        wall = new_wall(f, lengths=[("Height", 3.0), ("Length", 10.0)], areas=[("GrossSideArea", 30.0)])
        window_in(f, wall, 6.0)
        self.assertAlmostEqual(ifc_extractors.get_ifc_wall_area(wall, f), 24.0)

    def test_height_times_length_loses_its_windows(self):
        f = new_file(None)
        wall = new_wall(f, lengths=[("Height", 3.0), ("Length", 10.0)])
        window_in(f, wall, 6.0)
        self.assertAlmostEqual(ifc_extractors.get_ifc_wall_area(wall, f), 24.0)

    def test_net_side_area_is_preferred_to_gross(self):
        f = new_file(None)
        wall = new_wall(
            f,
            lengths=[("Height", 3.0), ("Length", 10.0)],
            areas=[("GrossSideArea", 30.0), ("NetSideArea", 24.0)],
        )
        window_in(f, wall, 6.0)
        self.assertAlmostEqual(ifc_extractors.get_ifc_wall_area(wall, f), 24.0)

    def test_a_wall_without_quantities_is_measured_from_its_geometry(self):
        import ifcopenshell.api.context
        import ifcopenshell.api.geometry
        f = new_file(None)
        model = ifcopenshell.api.context.add_context(f, context_type="Model")
        body = ifcopenshell.api.context.add_context(
            f, context_type="Model", context_identifier="Body", target_view="MODEL_VIEW", parent=model)
        wall = f.createIfcWall(ifcopenshell.guid.new(), None, "w", None, None, place(f, (0, 0), (1, 0)), None, None, None)
        rep = ifcopenshell.api.geometry.add_wall_representation(f, context=body, length=4.0, height=3.0, thickness=0.2)
        ifcopenshell.api.geometry.assign_representation(f, product=wall, representation=rep)
        self.assertAlmostEqual(ifc_extractors.get_ifc_wall_area(wall, f), 12.0)


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


def place(f, origin, x_axis):
    """A placement at `origin` (x, y) whose local X axis is `x_axis` (x, y)."""
    return f.createIfcLocalPlacement(None, f.createIfcAxis2Placement3D(
        f.createIfcCartesianPoint((float(origin[0]), float(origin[1]), 0.0)),
        f.createIfcDirection((0.0, 0.0, 1.0)),
        f.createIfcDirection((float(x_axis[0]), float(x_axis[1]), 0.0)),
    ))


def mark_external(f, element):
    prop = f.createIfcPropertySingleValue("IsExternal", None, f.create_entity("IfcBoolean", True), None)
    pset = f.createIfcPropertySet(ifcopenshell.guid.new(), None, "Pset_WallCommon", None, [prop])
    f.createIfcRelDefinesByProperties(ifcopenshell.guid.new(), None, None, None, [element], pset)


def square_house(true_north=None):
    """Four 10 m exterior walls around (5, 5), in millimetres. The north and
    west walls run "backwards", so their local +Y points into the house."""
    f = new_file("MILLI")
    if true_north is not None:
        f.createIfcGeometricRepresentationContext(
            None, "Model", 3, 1e-5,
            f.createIfcAxis2Placement3D(f.createIfcCartesianPoint((0.0, 0.0, 0.0)), None, None),
            f.createIfcDirection(true_north),
        )
    walls = {}
    for name, origin, x_axis in [
        ("south", (0, 0), (1, 0)),         # +Y = north: inward
        ("east", (10000, 0), (0, 1)),      # +Y = west: inward
        ("north", (10000, 10000), (-1, 0)),  # +Y = south: inward
        ("west", (0, 10000), (0, -1)),     # +Y = east: inward
    ]:
        wall = f.createIfcWall(ifcopenshell.guid.new(), None, name, None, None, place(f, origin, x_axis), None, None, None)
        add_quantities(f, wall, "Qto_WallBaseQuantities", lengths=[("Length", 10000.0), ("Height", 3000.0)])
        mark_external(f, wall)
        walls[name] = wall
    # one wall whose local +Y does point outward: an extra east wall run the other way
    wall = f.createIfcWall(ifcopenshell.guid.new(), None, "east2", None, None, place(f, (10000, 10000), (0, -1)), None, None, None)
    add_quantities(f, wall, "Qto_WallBaseQuantities", lengths=[("Length", 10000.0), ("Height", 3000.0)])
    mark_external(f, wall)
    walls["east2"] = wall
    return f, walls


@unittest.skipIf(ifcopenshell is None, "needs ifcopenshell")
class WallOrientationTest(unittest.TestCase):

    def facing(self, model, name):
        wall = next(w for w in model.walls if w.name == name)
        return (round(wall.orientation.x, 6), round(wall.orientation.y, 6))

    def test_each_wall_faces_out_of_the_house(self):
        f, _ = square_house()
        path = os.path.join(self.tmp, "square.ifc")
        f.write(path)
        model = ifc_extractors.extract_building_model_from_ifc(path)
        self.assertEqual(self.facing(model, "south"), (0.0, -1.0))
        self.assertEqual(self.facing(model, "east"), (1.0, 0.0))
        self.assertEqual(self.facing(model, "north"), (0.0, 1.0))
        self.assertEqual(self.facing(model, "west"), (-1.0, 0.0))
        self.assertEqual(self.facing(model, "east2"), (1.0, 0.0))

    def test_the_ifc_exterior_flag_is_kept(self):
        f, _ = square_house()
        inner = f.createIfcWall(ifcopenshell.guid.new(), None, "partition", None, None, place(f, (2000, 5000), (1, 0)), None, None, None)
        prop = f.createIfcPropertySingleValue("IsExternal", None, f.create_entity("IfcBoolean", False), None)
        pset = f.createIfcPropertySet(ifcopenshell.guid.new(), None, "Pset_WallCommon", None, [prop])
        f.createIfcRelDefinesByProperties(ifcopenshell.guid.new(), None, None, None, [inner], pset)
        unflagged = f.createIfcWall(ifcopenshell.guid.new(), None, "unflagged", None, None, place(f, (3000, 5000), (1, 0)), None, None, None)
        for wall in (inner, unflagged):
            add_quantities(f, wall, "Qto_WallBaseQuantities", lengths=[("Length", 2000.0), ("Height", 3000.0)])
        path = os.path.join(self.tmp, "flags.ifc")
        f.write(path)
        walls = {w.name: w for w in ifc_extractors.extract_building_model_from_ifc(path).walls}
        self.assertIs(walls["south"].is_external, True)
        self.assertIs(walls["partition"].is_external, False)
        self.assertIsNone(walls["unflagged"].is_external)

    def test_an_unset_exterior_flag_is_unknown(self):
        f, walls = square_house()
        blank = f.createIfcWall(ifcopenshell.guid.new(), None, "blank", None, None, place(f, (2000, 5000), (1, 0)), None, None, None)
        add_quantities(f, blank, "Qto_WallBaseQuantities", lengths=[("Length", 2000.0), ("Height", 3000.0)])
        prop = f.createIfcPropertySingleValue("IsExternal", None, None, None)
        pset = f.createIfcPropertySet(ifcopenshell.guid.new(), None, "Pset_WallCommon", None, [prop])
        f.createIfcRelDefinesByProperties(ifcopenshell.guid.new(), None, None, None, [blank], pset)
        path = os.path.join(self.tmp, "unset.ifc")
        f.write(path)
        extracted = {w.name: w for w in ifc_extractors.extract_building_model_from_ifc(path).walls}
        self.assertIsNone(extracted["blank"].is_external)

    def test_an_unrotated_model_has_no_project_north_angle(self):
        f, _ = square_house(true_north=(0.0, 1.0))
        path = os.path.join(self.tmp, "unrotated.ifc")
        f.write(path)
        model = ifc_extractors.extract_building_model_from_ifc(path)
        self.assertAlmostEqual(model.project_north, 0.0)

    def test_true_north_rotates_the_compass(self):
        # True north along project +X: the project's east wall faces north.
        import analysis_engine
        f, _ = square_house(true_north=(1.0, 0.0))
        path = os.path.join(self.tmp, "rotated.ifc")
        f.write(path)
        model = ifc_extractors.extract_building_model_from_ifc(path)
        east = next(w for w in model.walls if w.name == "east")
        self.assertEqual(analysis_engine.calculate_wall_orientation(east, model.project_north), "North")
        south = next(w for w in model.walls if w.name == "south")
        self.assertEqual(analysis_engine.calculate_wall_orientation(south, model.project_north), "East")
        # east2 runs the other way along the east side; it must face north too
        east2 = next(w for w in model.walls if w.name == "east2")
        self.assertEqual(analysis_engine.calculate_wall_orientation(east2, model.project_north), "North")

    def setUp(self):
        import tempfile
        self.tmp = tempfile.mkdtemp()

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp)


def curtain_house(external=True, panel_material="Glass"):
    """A 10 m square house in metres with a curtain wall on its south side:
    two 2 x 3 m panels and a 50 mm mullion, all with real box geometry."""
    import ifcopenshell.api.aggregate
    import ifcopenshell.api.context
    import ifcopenshell.api.geometry
    import ifcopenshell.api.material
    f = new_file(None)
    model = ifcopenshell.api.context.add_context(f, context_type="Model")
    body = ifcopenshell.api.context.add_context(
        f, context_type="Model", context_identifier="Body", target_view="MODEL_VIEW", parent=model)
    for name, origin, x_axis in [("south", (0, 0), (1, 0)), ("east", (10, 0), (0, 1)),
                                 ("north", (10, 10), (-1, 0)), ("west", (0, 10), (0, -1))]:
        wall = f.createIfcWall(ifcopenshell.guid.new(), None, name, None, None, place(f, origin, x_axis), None, None, None)
        add_quantities(f, wall, "Qto_WallBaseQuantities", lengths=[("Length", 10.0), ("Height", 3.0)])
        mark_external(f, wall)

    cw = f.createIfcCurtainWall(ifcopenshell.guid.new(), None, "glazing", None, None, place(f, (2, 0), (1, 0)), None, None, None)
    prop = f.createIfcPropertySingleValue("IsExternal", None, f.create_entity("IfcBoolean", external), None)
    pset = f.createIfcPropertySet(ifcopenshell.guid.new(), None, "Pset_CurtainWallCommon", None, [prop])
    f.createIfcRelDefinesByProperties(ifcopenshell.guid.new(), None, None, None, [cw], pset)

    material = ifcopenshell.api.material.add_material(f, name=panel_material)
    parts = []
    for i, x in enumerate((2.0, 4.0)):
        plate = f.createIfcPlate(ifcopenshell.guid.new(), None, "panel %d" % i, None, None, place(f, (x, 0), (1, 0)), None, None, None)
        rep = ifcopenshell.api.geometry.add_wall_representation(f, context=body, length=2.0, height=3.0, thickness=0.02)
        ifcopenshell.api.geometry.assign_representation(f, product=plate, representation=rep)
        ifcopenshell.api.material.assign_material(f, products=[plate], material=material)
        parts.append(plate)
    mullion = f.createIfcMember(ifcopenshell.guid.new(), None, "mullion", None, None, place(f, (6.0, 0), (1, 0)), None, None, None)
    rep = ifcopenshell.api.geometry.add_wall_representation(f, context=body, length=0.05, height=3.0, thickness=0.1)
    ifcopenshell.api.geometry.assign_representation(f, product=mullion, representation=rep)
    parts.append(mullion)
    ifcopenshell.api.aggregate.assign_object(f, products=parts, relating_object=cw)
    return f


@unittest.skipIf(ifcopenshell is None, "needs ifcopenshell")
class CurtainWallTest(unittest.TestCase):

    def setUp(self):
        import tempfile
        self.tmp = tempfile.mkdtemp()

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp)

    def extract(self, f):
        path = os.path.join(self.tmp, "house.ifc")
        f.write(path)
        return ifc_extractors.extract_building_model_from_ifc(path)

    def test_a_curtain_wall_brings_its_glass_and_faces_out(self):
        import analysis_engine
        model = self.extract(curtain_house())
        cw = next(w for w in model.walls if w.name == "glazing")
        self.assertTrue(cw.is_curtain_wall)
        self.assertIs(cw.is_external, True)
        self.assertEqual(sorted(w.area for w in cw.windows), [6.0, 6.0])
        self.assertAlmostEqual(cw.area, 0.15)   # the mullion's face
        # stored inward like Revit's, so the analysis turns it to face south
        self.assertEqual(analysis_engine.calculate_wall_orientation(cw, model.project_north, cw.is_curtain_wall), "South")
        self.assertEqual({w.id for w in model.windows} >= {w.id for w in cw.windows}, True)

    def test_an_opaque_panel_counts_as_wall(self):
        cw = next(w for w in self.extract(curtain_house(panel_material="Aluminium")).walls if w.name == "glazing")
        self.assertEqual(cw.windows, [])
        self.assertAlmostEqual(cw.area, 12.15)

    def test_a_door_is_glass_only_when_it_says_so(self):
        import ifcopenshell.api.material
        f = new_file(None)
        glass = ifcopenshell.api.material.add_material(f, name="Glass")
        frame = ifcopenshell.api.material.add_material(f, name="Aluminium")
        glazed = f.createIfcDoor(ifcopenshell.guid.new(), None, "Curtain Wall Dbl Glass", None, None, None, None, None, None, None, None, None, None)
        entrance = f.createIfcDoor(ifcopenshell.guid.new(), None, "Entrance door", None, None, None, None, None, None, None, None, None, None)
        for d in (glazed, entrance):
            f.createIfcRelAssociatesMaterial(ifcopenshell.guid.new(), None, None, None, [d], f.createIfcMaterialList([frame, glass]))
        self.assertTrue(ifc_extractors.is_glazing(glazed))
        self.assertFalse(ifc_extractors.is_glazing(entrance))

    def test_an_interior_curtain_wall_is_flagged_interior(self):
        cw = next(w for w in self.extract(curtain_house(external=False)).walls if w.name == "glazing")
        self.assertIs(cw.is_external, False)


@unittest.skipIf(ifcopenshell is None, "needs ifcopenshell")
class SampleFileTest(unittest.TestCase):

    def test_the_sample_house_still_extracts_as_documented(self):
        path = os.path.join(ROOT, "examples", "IFC", "Building-Architecture.ifc")
        model = ifc_extractors.extract_building_model_from_ifc(path)
        self.assertEqual(len(model.walls), 4)
        self.assertAlmostEqual(sum(w.area for w in model.walls), 43.3, places=1)
        self.assertEqual(model.metadata["sourceFile"], "Building-Architecture.ifc")

    def test_the_sample_house_walls_face_the_way_its_geometry_does(self):
        # Outward normals measured from the wall solids: two walls on the
        # east side of the house, one on the west.
        import analysis_engine
        path = os.path.join(ROOT, "examples", "IFC", "Building-Architecture.ifc")
        model = ifc_extractors.extract_building_model_from_ifc(path)
        facing = {w.name: analysis_engine.calculate_wall_orientation(w, model.project_north)
                  for w in model.walls}
        self.assertEqual(facing["house - outer wall - house right front"], "East")
        self.assertEqual(facing["house - outer wall - house right back"], "East")
        self.assertEqual(facing["house - outer wall - house left"], "West")


if __name__ == "__main__":
    unittest.main()
