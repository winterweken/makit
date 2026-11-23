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

import geometry_models

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


def geometry_file():
    """A metre file with a Model/Body context, for fixtures with real geometry."""
    import ifcopenshell.api.context
    f = new_file(None)
    model = ifcopenshell.api.context.add_context(f, context_type="Model")
    body = ifcopenshell.api.context.add_context(
        f, context_type="Model", context_identifier="Body", target_view="MODEL_VIEW", parent=model)
    return f, body


def place3(f, origin, z, x_axis=(1, 0), axis=(0.0, 0.0, 1.0)):
    """A placement at (`origin`, `z`) with local X along `x_axis` and local Z along `axis`."""
    return f.createIfcLocalPlacement(None, f.createIfcAxis2Placement3D(
        f.createIfcCartesianPoint((float(origin[0]), float(origin[1]), float(z))),
        f.createIfcDirection(tuple(float(a) for a in axis)),
        f.createIfcDirection((float(x_axis[0]), float(x_axis[1]), 0.0)),
    ))


def new_storey(f, name, elevation):
    return f.createIfcBuildingStorey(
        ifcopenshell.guid.new(), None, name, None, None, place3(f, (0, 0), elevation),
        None, None, "ELEMENT", float(elevation))


def slab_box(f, body, name, origin, size, top, thickness=0.2, predefined="FLOOR", storey=None, tilt=0.0,
             object_type=None):
    """An IfcSlab of `size` = (dx, dy) metres whose top face is at `top` (flat), tilted `tilt`
    radians about its X axis when asked. The slab API extrudes upward from its placement."""
    import math
    import ifcopenshell.api.geometry
    import ifcopenshell.api.spatial
    axis = (0.0, -math.sin(tilt), math.cos(tilt))
    slab = f.createIfcSlab(ifcopenshell.guid.new(), None, name, None, object_type,
                           place3(f, origin, top - thickness, (1, 0), axis), None, None, predefined)
    dx, dy = size
    rep = ifcopenshell.api.geometry.add_slab_representation(
        f, context=body, depth=thickness, polyline=[(0.0, 0.0), (dx, 0.0), (dx, dy), (0.0, dy)])
    ifcopenshell.api.geometry.assign_representation(f, product=slab, representation=rep)
    if storey is not None:
        ifcopenshell.api.spatial.assign_container(f, products=[slab], relating_structure=storey)
    return slab


class TempDirTest(unittest.TestCase):
    """Writes fixtures to a temp folder and extracts them."""

    def setUp(self):
        import tempfile
        self.tmp = tempfile.mkdtemp()

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp)

    def extract(self, f, options=None):
        path = os.path.join(self.tmp, "fixture.ifc")
        f.write(path)
        return ifc_extractors.extract_building_model_from_ifc(path, options)


@unittest.skipIf(ifcopenshell is None, "needs ifcopenshell")
class FloorPlateTest(TempDirTest):

    def test_overlapping_floors_on_one_storey_count_once(self):
        f, body = geometry_file()
        level = new_storey(f, "Level 1", 0.0)
        slab_box(f, body, "structural", (0, 0), (10, 10), top=0.0, storey=level)
        slab_box(f, body, "finish", (2, 2), (4, 4), top=0.02, thickness=0.02, storey=level)
        model = self.extract(f)
        storey = model.storeys[0]
        self.assertEqual(storey.name, "Level 1")
        self.assertAlmostEqual(storey.floor_area, 100.0, places=1)
        self.assertEqual(len(storey.slab_ids), 2)
        self.assertEqual(sorted(round(s.area, 1) for s in model.slabs), [16.0, 100.0])
        self.assertTrue(all(s.kind == "floor" and s.storey == "Level 1" for s in model.slabs))

    def test_a_slab_far_below_its_level_has_no_storey(self):
        f, body = geometry_file()
        level = new_storey(f, "Level 1", 0.0)
        slab_box(f, body, "pad", (0, 0), (10, 10), top=-0.5, storey=level)
        model = self.extract(f)
        self.assertIsNone(model.slabs[0].storey)
        self.assertAlmostEqual(model.storeys[0].floor_area, 0.0)
        self.assertEqual(model.storeys[0].slab_ids, [])

    def test_without_storeys_every_plate_is_loose(self):
        f, body = geometry_file()
        slab_box(f, body, "floor", (0, 0), (10, 10), top=0.0)
        model = self.extract(f)
        self.assertEqual(model.storeys, [])
        self.assertEqual(len(model.slabs), 1)
        self.assertIsNone(model.slabs[0].storey)

    def test_a_stair_landing_is_not_a_plate(self):
        import ifcopenshell.api.aggregate
        f, body = geometry_file()
        level = new_storey(f, "Level 1", 0.0)
        slab_box(f, body, "floor", (0, 0), (10, 10), top=0.0, storey=level)
        stair = f.createIfcStair(ifcopenshell.guid.new(), None, "stair", None, None,
                                 place3(f, (20, 0), 0.0), None, None, "STRAIGHT_RUN_STAIR")
        landing = slab_box(f, body, "landing", (20, 0), (2, 2), top=0.0, predefined="LANDING", storey=level)
        ifcopenshell.api.aggregate.assign_object(f, products=[landing], relating_object=stair)
        model = self.extract(f)
        kinds = {s.name: s.kind for s in model.slabs}
        self.assertEqual(kinds, {"floor": "floor", "landing": "landing"})
        self.assertNotIn(landing.GlobalId, model.storeys[0].slab_ids)
        self.assertAlmostEqual(model.storeys[0].floor_area, 100.0, places=1)

    def test_plates_record_their_elevations_and_type(self):
        f, body = geometry_file()
        level = new_storey(f, "Level 2", 3.0)
        slab_box(f, body, "upper", (0, 0), (6, 5), top=3.0, thickness=0.3, storey=level,
                 object_type="Floor:Generic 300")
        model = self.extract(f)
        slab = model.slabs[0]
        self.assertAlmostEqual(slab.top, 3.0, places=3)
        self.assertAlmostEqual(slab.bottom, 2.7, places=3)
        self.assertEqual(slab.slab_type, "Floor:Generic 300")
        self.assertIsNone(slab.is_external)
        self.assertEqual(slab.properties["ifcType"], "IfcSlab")
        self.assertAlmostEqual(model.storeys[0].elevation, 3.0)

    def test_a_storey_filter_keeps_only_that_storey(self):
        f, body = geometry_file()
        l1 = new_storey(f, "L1", 0.0)
        l2 = new_storey(f, "L2", 3.0)
        slab_box(f, body, "ground", (0, 0), (10, 10), top=0.0, storey=l1)
        slab_box(f, body, "upper", (0, 0), (8, 8), top=3.0, storey=l2)
        model = self.extract(f, {"storey": "L2"})
        self.assertEqual([s.name for s in model.storeys], ["L2"])
        self.assertEqual([s.name for s in model.slabs], ["upper"])
        self.assertAlmostEqual(model.storeys[0].floor_area, 64.0, places=1)


@unittest.skipIf(ifcopenshell is None, "needs ifcopenshell")
class RoofTest(TempDirTest):

    def test_a_roof_aggregate_counts_its_slopes_once(self):
        # Two 10 x 5 m slopes at 20 degrees: 50 m2 of surface each, 47 m2 of footprint.
        # (At 30 degrees the slabs' 60-degree edge faces sit exactly on the
        # up-facing boundary, normal z = 0.5, and floating point picks a side.)
        import math
        import ifcopenshell.api.aggregate
        f, body = geometry_file()
        roof = f.createIfcRoof(ifcopenshell.guid.new(), None, "gable", None, None,
                               place3(f, (0, 0), 6.0), None, None, "GABLE_ROOF")
        left = slab_box(f, body, "left", (0, 5), (10, 5), top=6.0, predefined="ROOF", tilt=math.radians(20))
        right = slab_box(f, body, "right", (0, 5), (10, 5), top=6.0, predefined="ROOF", tilt=-math.radians(20))
        ifcopenshell.api.aggregate.assign_object(f, products=[left, right], relating_object=roof)
        model = self.extract(f)
        roofs = [s for s in model.slabs if s.kind == "roof"]
        self.assertEqual([r.id for r in roofs], [roof.GlobalId])
        self.assertAlmostEqual(roofs[0].area, 100.0, places=0)
        self.assertEqual(roofs[0].name, "gable")

    def test_a_roof_slab_outside_any_roof_counts(self):
        f, body = geometry_file()
        slab_box(f, body, "flat roof", (0, 0), (8, 8), top=6.0, predefined="ROOF")
        model = self.extract(f)
        roofs = [s for s in model.slabs if s.kind == "roof"]
        self.assertEqual(len(roofs), 1)
        self.assertAlmostEqual(roofs[0].area, 64.0, places=1)

    def test_a_roof_slab_is_not_a_floor_plate(self):
        f, body = geometry_file()
        top = new_storey(f, "Roof", 6.0)
        slab_box(f, body, "flat roof", (0, 0), (8, 8), top=6.0, predefined="ROOF", storey=top)
        model = self.extract(f)
        self.assertAlmostEqual(model.storeys[0].floor_area, 0.0)
        self.assertEqual(model.storeys[0].slab_ids, [])

    def test_a_roof_with_its_own_geometry_is_measured_once(self):
        import ifcopenshell.api.aggregate
        import ifcopenshell.api.geometry
        f, body = geometry_file()
        roof = f.createIfcRoof(ifcopenshell.guid.new(), None, "roof", None, None,
                               place3(f, (0, 0), 6.0), None, None, "FLAT_ROOF")
        rep = ifcopenshell.api.geometry.add_slab_representation(
            f, context=body, depth=0.2, polyline=[(0.0, 0.0), (8.0, 0.0), (8.0, 8.0), (0.0, 8.0)])
        ifcopenshell.api.geometry.assign_representation(f, product=roof, representation=rep)
        part = slab_box(f, body, "part", (0, 0), (10, 10), top=6.2, predefined="ROOF")
        ifcopenshell.api.aggregate.assign_object(f, products=[part], relating_object=roof)
        model = self.extract(f)
        roofs = [s for s in model.slabs if s.kind == "roof"]
        self.assertEqual(len(roofs), 1)
        self.assertAlmostEqual(roofs[0].area, 64.0, places=1)


class GenericModelTest(unittest.TestCase):
    """The generic format carries storeys and slabs and stamps its version."""

    def test_storeys_and_slabs_round_trip(self):
        model = geometry_models.BuildingModel()
        storey = geometry_models.GenericStorey("s1", "L1")
        storey.elevation = 3.0
        storey.floor_area = 12.5
        storey.slab_ids = ["a"]
        slab = geometry_models.GenericSlab("a", "floor a")
        slab.area = 12.5
        slab.top = 3.0
        slab.bottom = 2.8
        slab.storey = "L1"
        slab.slab_type = "Floor:Generic"
        slab.is_external = False
        model.storeys.append(storey)
        model.slabs.append(slab)
        data = model.to_dict()
        self.assertEqual(data["storeys"][0],
                         {"id": "s1", "name": "L1", "elevation": 3.0, "floorArea": 12.5, "slabIds": ["a"]})
        self.assertEqual(data["slabs"][0]["kind"], "floor")
        self.assertEqual(data["slabs"][0]["type"], "Floor:Generic")
        self.assertIs(data["slabs"][0]["isExternal"], False)
        self.assertEqual(data["slabs"][0]["top"], 3.0)
        back = geometry_models.BuildingModel.from_dict(data)
        self.assertEqual(back.storeys[0].slab_ids, ["a"])
        self.assertEqual(back.slabs[0].storey, "L1")
        self.assertEqual(back.slabs[0].bottom, 2.8)

    def test_to_dict_stamps_format_version_2(self):
        model = geometry_models.BuildingModel()
        model.metadata["projectName"] = "p"
        self.assertEqual(model.to_dict()["metadata"]["formatVersion"], 2)
        self.assertEqual(geometry_models.FORMAT_VERSION, 2)
        self.assertNotIn("formatVersion", model.metadata)

    def test_old_dicts_without_storeys_still_load(self):
        model = geometry_models.BuildingModel.from_dict({"walls": [], "metadata": {}})
        self.assertEqual((model.storeys, model.slabs), ([], []))


if __name__ == "__main__":
    unittest.main()
