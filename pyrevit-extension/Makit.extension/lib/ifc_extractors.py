# -*- coding: utf-8 -*-
"""IFC-specific extraction functions that convert to generic format"""

try:
    import ifcopenshell
    import ifcopenshell.util.element
    import ifcopenshell.util.placement
    import ifcopenshell.util.unit
    import ifcopenshell.geom
    IFC_AVAILABLE = True
except ImportError:
    IFC_AVAILABLE = False
    print("Warning: ifcopenshell not available. Install with: pip install ifcopenshell")

import math
import os
from geometry_models import (
    GeometricVector, GenericWall, GenericWindow, GenericStorey, GenericSlab, BuildingModel
)

# Constants
SQM_TO_SQF = 10.7639  # Square meters to square feet conversion
# A floor plate belongs to the storey whose elevation is within this many
# metres of its top face (finish floors sit a few centimetres above a level)
PLATE_STOREY_TOLERANCE = 0.3


def get_ifc_property_value(element, pset_name, prop_name):
    """
    Get a property value from an IFC element

    Args:
        element: IFC element
        pset_name: Property set name
        prop_name: Property name

    Returns:
        Property value or None
    """
    if not IFC_AVAILABLE:
        return None

    try:
        psets = ifcopenshell.util.element.get_psets(element)
        if pset_name in psets and prop_name in psets[pset_name]:
            return psets[pset_name][prop_name]
    except:
        pass
    return None


def get_wall_axis(wall):
    """
    A wall's start point and unit direction in plan, in file units, from its
    placement: IFC runs a wall along its local X axis.

    Returns:
        ((x, y), (dx, dy)) or None
    """
    try:
        matrix = ifcopenshell.util.placement.get_local_placement(wall.ObjectPlacement)
    except Exception:
        return None
    dx, dy = matrix[0][0], matrix[1][0]
    length = math.hypot(dx, dy)
    if length == 0:
        return None
    return (matrix[0][3], matrix[1][3]), (dx / length, dy / length)

def get_wall_midpoint(wall):
    """Middle of a wall's axis in plan (its start point without a Length)."""
    axis = get_wall_axis(wall)
    if axis is None:
        return None
    (x, y), (dx, dy) = axis
    length = 0
    for quantity in get_element_quantities(wall):
        if quantity.is_a('IfcQuantityLength') and quantity.Name == 'Length':
            length = quantity.LengthValue
    return x + dx * length / 2, y + dy * length / 2

def get_is_external(element):
    """
    The element's IsExternal flag from its Pset_*Common property set:
    True, False, or None when the file does not say (no property, or one
    left without a value).
    """
    for pset, props in ifcopenshell.util.element.get_psets(element).items():
        if pset.endswith('Common') and props.get('IsExternal') is not None:
            return bool(props['IsExternal'])
    return None

def get_building_centre(walls):
    """
    Plan centre of the walls flagged IsExternal (of all walls if none are),
    used to tell a wall's outer face from its inner one.
    """
    chosen = [w for w in walls if get_is_external(w)] or list(walls)
    points = [p for p in (get_wall_midpoint(w) for w in chosen) if p is not None]
    if not points:
        return None
    return (sum(p[0] for p in points) / len(points), sum(p[1] for p in points) / len(points))

def get_ifc_wall_orientation(wall, centre=None):
    """
    Outward-facing normal of an IFC wall, in project coordinates

    The wall runs along its placement's X axis, so its faces look along the
    perpendicular. Which of the two is outside is not fixed by the local Y
    axis (Revit exports both ways), so the normal is turned to point away
    from `centre`, the middle of the building. Without a centre it is the
    placement's +Y side.

    Args:
        wall: IFC wall element
        centre: (x, y) plan centre of the building, in file units

    Returns:
        GeometricVector or None
    """
    if not IFC_AVAILABLE:
        return None

    axis = get_wall_axis(wall)
    if axis is None:
        print("Warning: Could not extract wall orientation: no placement")
        return None
    _, (dx, dy) = axis
    nx, ny = -dy, dx
    if centre is not None:
        mid = get_wall_midpoint(wall)
        if (mid[0] - centre[0]) * nx + (mid[1] - centre[1]) * ny < 0:
            nx, ny = -nx, -ny
    return GeometricVector(nx, ny, 0.0)

def get_axis_points(wall):
    """
    The first and last point of the wall's Axis representation, in the
    wall's local coordinates (x, y, z): an IfcPolyline or IfcIndexedPolyCurve
    of two or more points (2D points get z 0). None when the wall has no
    such axis.
    """
    representation = getattr(wall, 'Representation', None)
    if not representation:
        return None
    for rep in representation.Representations:
        if rep.RepresentationIdentifier != 'Axis' or not rep.Items:
            continue
        item = rep.Items[0]
        if item.is_a('IfcPolyline'):
            points = [p.Coordinates for p in item.Points]
        elif item.is_a('IfcIndexedPolyCurve'):
            points = list(item.Points.CoordList)
        else:
            return None
        if len(points) < 2:
            return None
        ends = []
        for p in (points[0], points[-1]):
            ends.append((p[0], p[1], p[2] if len(p) > 2 else 0.0))
        return ends
    return None

def to_project(matrix, point, length_scale):
    """A local (x, y, z) through a 4x4 placement matrix, scaled to metres."""
    x, y, z = point
    return tuple(
        float(matrix[i][0] * x + matrix[i][1] * y + matrix[i][2] * z + matrix[i][3]) * length_scale
        for i in range(3))

def axis_from_faces(faces, direction, z=None):
    """
    A wall's axis measured from its triangles (metres, project coordinates):
    its ends are the least and greatest extent of the vertices along the
    plan `direction` (dx, dy), on the line through their mean across it, at
    `z` (the lowest vertex when None). None without vertices or direction.
    """
    vertices = [v for _, _, triangle in faces for v in triangle]
    if not vertices:
        return None
    dx, dy = direction
    length = math.hypot(dx, dy)
    if length == 0:
        return None
    dx, dy = dx / length, dy / length
    along = [v[0] * dx + v[1] * dy for v in vertices]
    across = sum(-v[0] * dy + v[1] * dx for v in vertices) / len(vertices)
    if z is None:
        z = min(v[2] for v in vertices)

    def point(a):
        return (a * dx - across * dy, a * dy + across * dx, z)

    return point(min(along)), point(max(along))

def get_ifc_wall_geometry(wall, ifc_file):
    """
    A wall's axis as (start, end) GeometricVectors in metres, project
    coordinates: the ends of its Axis representation, else its placement
    origin and X direction run out to its Length quantity, else measured
    from its geometry along the placement's X axis. (None, None) when the
    wall offers none of these.
    """
    if not IFC_AVAILABLE:
        return None, None
    try:
        length_scale, _ = get_unit_scales(ifc_file)
        if not getattr(wall, 'ObjectPlacement', None):
            return None, None
        matrix = ifcopenshell.util.placement.get_local_placement(wall.ObjectPlacement)

        points = get_axis_points(wall)
        if points:
            start, end = [to_project(matrix, p, length_scale) for p in points]
            return GeometricVector(*start), GeometricVector(*end)

        length = 0.0
        for quantity in get_element_quantities(wall):
            if quantity.is_a('IfcQuantityLength') and quantity.Name == 'Length':
                length = quantity.LengthValue
        if length > 0:
            start = to_project(matrix, (0.0, 0.0, 0.0), length_scale)
            end = to_project(matrix, (length, 0.0, 0.0), length_scale)
            return GeometricVector(*start), GeometricVector(*end)

        try:
            faces = get_faces(wall)
        except Exception:
            faces = []
        axis = axis_from_faces(faces, (matrix[0][0], matrix[1][0]))
        if axis:
            return GeometricVector(*axis[0]), GeometricVector(*axis[1])
    except Exception as e:
        print("Warning: no axis for wall {}: {}".format(wall.GlobalId, str(e)))
    return None, None

def get_unit_scales(ifc_file):
    """
    Metres per length unit and square metres per area unit of an IFC file
    (0.001 and 1.0 for a file in millimetres with areas in square metres)
    """
    try:
        return (
            ifcopenshell.util.unit.calculate_unit_scale(ifc_file, 'LENGTHUNIT'),
            ifcopenshell.util.unit.calculate_unit_scale(ifc_file, 'AREAUNIT'),
        )
    except Exception:
        return 1.0, 1.0

def get_element_quantities(element):
    """Every quantity in an element's quantity sets"""
    for rel in element.IsDefinedBy:
        if rel.is_a('IfcRelDefinesByProperties'):
            prop_def = rel.RelatingPropertyDefinition
            if prop_def.is_a('IfcElementQuantity'):
                for quantity in prop_def.Quantities:
                    yield quantity

def get_opening_area(wall, ifc_file):
    """
    Square metres of the windows and doors that fill openings in a wall,
    from their Area quantity or else their overall height and width
    """
    length_scale, area_scale = get_unit_scales(ifc_file)
    total = 0.0
    for voids in getattr(wall, 'HasOpenings', None) or []:
        for fills in voids.RelatedOpeningElement.HasFillings or []:
            element = fills.RelatedBuildingElement
            area = 0.0
            for quantity in get_element_quantities(element):
                if quantity.is_a('IfcQuantityArea') and 'Area' in quantity.Name:
                    area = quantity.AreaValue * area_scale
                    break
            if not area:
                height = getattr(element, 'OverallHeight', None) or 0
                width = getattr(element, 'OverallWidth', None) or 0
                area = height * width * length_scale ** 2
            total += area
    return total

def get_side_area_from_geometry(element):
    """
    One side of an element measured from its geometry, in square metres:
    half the vertical surface facing the most common direction. Openings
    are already cut out of the geometry. None without geometry.
    """
    try:
        triangles = get_triangles(element)
    except Exception:
        return None
    weights = {}
    for n, area, _ in triangles:
        if abs(n[2]) > 0.1:
            continue
        key = (round(n[0], 2), round(n[1], 2))
        if key[0] < 0 or (key[0] == 0 and key[1] < 0):
            key = (-key[0], -key[1])
        weights[key] = weights.get(key, 0.0) + area
    if not weights:
        return None
    return max(weights.values()) / 2.0

def get_ifc_wall_area(wall, ifc_file):
    """
    A wall's opaque side area in square metres: net of the windows and
    doors in it, which the analyses add back as window area
    """
    if not IFC_AVAILABLE:
        return 0.0

    try:
        length_scale, area_scale = get_unit_scales(ifc_file)
        height = 0
        length = 0
        net = []
        gross = []
        for quantity in get_element_quantities(wall):
            if quantity.is_a('IfcQuantityLength'):
                if 'Height' in quantity.Name:
                    height = quantity.LengthValue * length_scale
                elif 'Length' in quantity.Name:
                    length = quantity.LengthValue * length_scale
            elif quantity.is_a('IfcQuantityArea'):
                if 'NetSideArea' in quantity.Name:
                    net.append(quantity.AreaValue * area_scale)
                elif 'GrossSideArea' in quantity.Name:
                    gross.append(quantity.AreaValue * area_scale)

        # A side can be no larger than height x length. This rules out the
        # extra NetSideArea that Revit's exporter writes in squared length
        # units (mm2 in a millimetre file) ahead of the real one.
        if height > 0 and length > 0:
            net = [a for a in net if a <= height * length * 1.01]
            gross = [a for a in gross if a <= height * length * 1.01]

        if net:
            return net[0]
        # A gross side, or height x length, still includes the openings
        if gross:
            return max(gross[0] - get_opening_area(wall, ifc_file), 0.0)
        if height > 0 and length > 0:
            return max(height * length - get_opening_area(wall, ifc_file), 0.0)
        # No quantities: measure the geometry, openings already cut out
        measured = get_side_area_from_geometry(wall)
        if measured is not None:
            return measured
        print("Warning: no area for wall {}: no quantities or geometry".format(wall.GlobalId))

    except Exception as e:
        print("Warning: Could not calculate wall area: {}".format(str(e)))

    return 0.0

def get_element_position(element):
    """Get absolute position of an element"""
    if not IFC_AVAILABLE:
        return None
        
    try:
        matrix = ifcopenshell.util.placement.get_local_placement(element.ObjectPlacement)
        return GeometricVector(matrix[0][3], matrix[1][3], matrix[2][3])
    except:
        return None

def extract_ifc_wall_to_generic(wall, ifc_file, area_unit='sqm', centre=None):
    """
    Extract an IFC wall element and convert to GenericWall
    """
    generic_wall = GenericWall(
        id=wall.GlobalId,
        name=wall.Name or ""
    )

    # Get orientation
    orientation = get_ifc_wall_orientation(wall, centre)
    if orientation:
        generic_wall.orientation = orientation
        
    # Get Geometry Points
    start, end = get_ifc_wall_geometry(wall, ifc_file)
    if start:
        generic_wall.start_point = start
    if end:
        generic_wall.end_point = end

    # Get area
    area_sqm = get_ifc_wall_area(wall, ifc_file)
    if area_unit == 'sqm':
        generic_wall.area = round(area_sqm, 1)
    else:
        generic_wall.area = round(area_sqm * SQM_TO_SQF, 1)

    # Get wall type
    if hasattr(wall, 'ObjectType') and wall.ObjectType:
        generic_wall.wall_type = wall.ObjectType
    elif hasattr(wall, 'IsTypedBy') and wall.IsTypedBy:
        for rel in wall.IsTypedBy:
            if rel.RelatingType:
                generic_wall.wall_type = rel.RelatingType.Name or ""
                break

    # Get level/storey
    try:
        for rel in wall.ContainedInStructure:
            if rel.is_a('IfcRelContainedInSpatialStructure'):
                container = rel.RelatingStructure
                if container.is_a('IfcBuildingStorey'):
                    generic_wall.level = container.Name or ""
                    break
    except:
        pass

    # Get dimensions from quantities
    try:
        for rel in wall.IsDefinedBy:
            if rel.is_a('IfcRelDefinesByProperties'):
                prop_def = rel.RelatingPropertyDefinition
                if prop_def.is_a('IfcElementQuantity'):
                    for quantity in prop_def.Quantities:
                        if quantity.is_a('IfcQuantityLength'):
                            if 'Height' in quantity.Name:
                                generic_wall.height = quantity.LengthValue
                            elif 'Length' in quantity.Name or 'Width' in quantity.Name:
                                generic_wall.length = quantity.LengthValue
                        elif quantity.is_a('IfcQuantityLength') and 'Width' in quantity.Name:
                            generic_wall.width = quantity.LengthValue
    except:
        pass

    # Check if curtain wall
    generic_wall.is_curtain_wall = 'CURTAIN' in (wall.ObjectType or '').upper()

    # Exterior or interior, as the file flags it (None if it does not)
    generic_wall.is_external = get_is_external(wall)

    # Where the wall stands, for the split above and below grade
    generic_wall.bottom, generic_wall.top = get_wall_elevations(wall, ifc_file)

    # Store IFC-specific properties
    generic_wall.properties['ifcType'] = wall.is_a()
    if hasattr(wall, 'Description') and wall.Description:
        generic_wall.properties['description'] = wall.Description

    return generic_wall


def get_storey_name(element):
    """Name of the building storey that contains an element, or None"""
    try:
        for rel in element.ContainedInStructure:
            if rel.is_a('IfcRelContainedInSpatialStructure') and rel.RelatingStructure.is_a('IfcBuildingStorey'):
                return rel.RelatingStructure.Name
    except Exception:
        pass
    return None

# Faces by entity id, kept only while extract_building_model_from_ifc runs:
# a wall's area, elevations and axis all need them, and tessellation is the
# slow part. Entity ids repeat across files, so the cache never outlives one
# extraction.
_FACES_CACHE = None

def get_faces(element):
    """
    An element's surface as triangles in world coordinates, in metres:
    a list of (unit normal, area, (a, b, c) vertices)
    """
    if _FACES_CACHE is not None and element.id() in _FACES_CACHE:
        return _FACES_CACHE[element.id()]
    faces = tessellate(element)
    if _FACES_CACHE is not None:
        _FACES_CACHE[element.id()] = faces
    return faces

def tessellate(element):
    """get_faces without the cache"""
    settings = ifcopenshell.geom.settings()
    settings.set('use-world-coords', True)
    shape = ifcopenshell.geom.create_shape(settings, element)
    verts = shape.geometry.verts
    faces = shape.geometry.faces
    triangles = []
    for i in range(0, len(faces), 3):
        a, b, c = [verts[3 * faces[i + k]:3 * faces[i + k] + 3] for k in range(3)]
        u = [b[j] - a[j] for j in range(3)]
        v = [c[j] - a[j] for j in range(3)]
        n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
        length = math.sqrt(sum(x * x for x in n))
        if length == 0:
            continue
        triangles.append(([x / length for x in n], length / 2.0, (a, b, c)))
    return triangles

def get_triangles(element):
    """
    An element's surface as triangles in world coordinates, in metres:
    a list of (unit normal, area, centroid)
    """
    return [
        (n, area, [(a[j] + b[j] + c[j]) / 3.0 for j in range(3)])
        for n, area, (a, b, c) in get_faces(element)
    ]

def get_element_faces(element):
    """
    Faces of an element's own geometry, or of its decomposed parts when it
    has none itself (an IfcRoof made of slabs, say). Never both.
    """
    if getattr(element, 'Representation', None):
        return get_faces(element)
    faces = []
    for part in ifcopenshell.util.element.get_decomposition(element):
        if getattr(part, 'Representation', None):
            faces.extend(get_faces(part))
    return faces

def get_footprint(faces):
    """
    The plan footprint of the up-facing faces (normal z > 0.5) as a shapely
    geometry, or None without shapely or without up-facing faces
    """
    try:
        from shapely.geometry import Polygon
        from shapely.ops import unary_union
    except ImportError:
        return None
    polygons = []
    for n, _, (a, b, c) in faces:
        if n[2] <= 0.5:
            continue
        polygon = Polygon([(a[0], a[1]), (b[0], b[1]), (c[0], c[1])])
        if polygon.is_valid and polygon.area > 0:
            polygons.append(polygon)
    if not polygons:
        return None
    return unary_union(polygons)

def get_up_facing_area(faces):
    """Surface area of the faces that look up (normal z > 0.5): a roof's slopes"""
    return sum(area for n, area, _ in faces if n[2] > 0.5)

def get_z_range(faces):
    """(bottom, top) of the faces' vertices in metres, or (None, None)"""
    zs = [point[2] for _, _, triangle in faces for point in triangle]
    if not zs:
        return None, None
    return min(zs), max(zs)

def get_wall_elevations(wall, ifc_file):
    """
    (bottom, top) of a wall in metres, project z: from its geometry (its
    parts' for a curtain wall), else from its placement and Height quantity,
    else (None, None)
    """
    try:
        faces = get_element_faces(wall)
    except Exception:
        faces = []
    if faces:
        return get_z_range(faces)
    length_scale, _ = get_unit_scales(ifc_file)
    try:
        matrix = ifcopenshell.util.placement.get_local_placement(wall.ObjectPlacement)
    except Exception:
        return None, None
    height = None
    for quantity in get_element_quantities(wall):
        if quantity.is_a('IfcQuantityLength') and 'Height' in quantity.Name:
            height = quantity.LengthValue
    if height is None:
        return None, None
    bottom = matrix[2][3] * length_scale
    return bottom, bottom + height * length_scale

def get_storey_elevation(storey, ifc_file):
    """
    A storey's elevation in metres, project z: its placement's z, the frame
    the plates' geometry is matched in, else its Elevation attribute
    """
    length_scale, _ = get_unit_scales(ifc_file)
    placement = getattr(storey, 'ObjectPlacement', None)
    if placement is not None:
        try:
            matrix = ifcopenshell.util.placement.get_local_placement(placement)
            return matrix[2][3] * length_scale
        except Exception:
            pass
    elevation = getattr(storey, 'Elevation', None)
    return elevation * length_scale if elevation is not None else 0.0

def has_ancestor(element, *ifc_classes):
    """
    Whether the element is part, at any depth, of an aggregate of one of the
    classes (an IfcRoof -> IfcElementAssembly -> IfcSlab chain, say)
    """
    parent = ifcopenshell.util.element.get_aggregate(element)
    while parent is not None:
        if any(parent.is_a(ifc_class) for ifc_class in ifc_classes):
            return True
        parent = ifcopenshell.util.element.get_aggregate(parent)
    return False

def slab_kind(slab):
    """
    'roof' for a ROOF slab or one anywhere under an IfcRoof, 'landing' for a
    LANDING or a part of a stair or ramp, 'other' for a part of anything else,
    else 'floor'
    """
    predefined = getattr(slab, 'PredefinedType', None)
    if predefined == 'ROOF' or has_ancestor(slab, 'IfcRoof'):
        return 'roof'
    if predefined == 'LANDING' or has_ancestor(slab, 'IfcStair', 'IfcRamp'):
        return 'landing'
    if ifcopenshell.util.element.get_aggregate(slab) is not None:
        return 'other'
    return 'floor'

def has_shapely():
    """Whether shapely, which comes with IfcOpenShell, can be imported"""
    try:
        import shapely  # noqa: F401
        return True
    except ImportError:
        return False

def nearest_storey(top, storeys):
    """The storey whose elevation is nearest `top`, within PLATE_STOREY_TOLERANCE, or None"""
    best = None
    for storey in storeys:
        distance = abs(storey.elevation - top)
        if distance <= PLATE_STOREY_TOLERANCE and (best is None or distance < best[0]):
            best = (distance, storey)
    return best[1] if best else None

def slab_from_faces(slab, faces, storeys, to_unit=1.0):
    """
    A GenericSlab from an IfcSlab's faces: a floor plate's area is its plan
    footprint and it joins the storey nearest its top face (which records the
    plate in its slabIds); a roof slab's area is its up-facing surface. Areas
    are multiplied by `to_unit` (square feet per square metre for 'sqf').
    """
    generic = GenericSlab(id=slab.GlobalId, name=slab.Name or "")
    generic.kind = slab_kind(slab)
    generic.bottom, generic.top = get_z_range(faces)
    generic.slab_type = getattr(slab, 'ObjectType', None) or ""
    generic.is_external = get_is_external(slab)
    generic.properties['ifcType'] = slab.is_a()
    footprint = get_footprint(faces) if generic.kind != 'roof' else None
    if footprint is not None:
        generic.area = round(footprint.area * to_unit, 2)
    else:
        generic.area = round(get_up_facing_area(faces) * to_unit, 2)
    if generic.kind == 'floor' and generic.top is not None:
        storey = nearest_storey(generic.top, storeys)
        if storey is not None:
            generic.storey = storey.name
            storey.slab_ids.append(generic.id)
    elif generic.kind != 'floor':
        generic.storey = get_storey_name(slab)
    return generic

def extract_ifc_slab_to_generic(slab, ifc_file, storeys, area_unit='sqm'):
    """
    An IfcSlab as a GenericSlab (see slab_from_faces), or None without geometry
    """
    try:
        faces = get_faces(slab)
    except Exception as e:
        print("Warning: no geometry for slab {}: {}".format(slab.GlobalId, str(e)))
        return None
    if not faces:
        return None
    return slab_from_faces(slab, faces, storeys, 1.0 if area_unit == 'sqm' else SQM_TO_SQF)

def extract_ifc_roof_to_generic(roof, ifc_file, area_unit='sqm'):
    """
    An IfcRoof as a GenericSlab of kind 'roof' whose area is the up-facing
    surface of its own geometry, or of its parts when it has none. None when
    nothing has geometry.
    """
    try:
        faces = get_element_faces(roof)
    except Exception as e:
        print("Warning: no geometry for roof {}: {}".format(roof.GlobalId, str(e)))
        return None
    if not faces:
        return None
    generic = GenericSlab(id=roof.GlobalId, name=roof.Name or "")
    generic.kind = 'roof'
    to_unit = 1.0 if area_unit == 'sqm' else SQM_TO_SQF
    generic.area = round(get_up_facing_area(faces) * to_unit, 2)
    generic.bottom, generic.top = get_z_range(faces)
    generic.storey = get_storey_name(roof)
    generic.slab_type = getattr(roof, 'ObjectType', None) or ""
    generic.is_external = get_is_external(roof)
    generic.properties['ifcType'] = roof.is_a()
    return generic

def extract_storeys_and_slabs(ifc_file, building_model, storey_filter=None, area_unit='sqm'):
    """
    Fill a building model's storeys and slabs.

    Storeys carry their elevation in metres. Floor plates (IfcSlab, not a roof,
    landing or part of something) join the storey nearest their top face; a
    storey's floorArea is the footprint union of its plates, so finish floors
    over structural floors count once (their sum, with a warning, when shapely
    is missing). Roofs are IfcRoof elements (own geometry, else their parts)
    and ROOF slabs outside any IfcRoof, each counted once. With a storey
    filter, only that storey and its slabs are kept. Areas follow
    `area_unit` ('sqm' or 'sqf'), like the walls' and windows'.
    """
    to_unit = 1.0 if area_unit == 'sqm' else SQM_TO_SQF
    if not has_shapely():
        print("Warning: shapely not available: floor plates measure their surface, "
              "not their footprint, and storey floor areas are sums, not unions")

    storeys = []
    for ifc_storey in ifc_file.by_type('IfcBuildingStorey'):
        if storey_filter and ifc_storey.Name != storey_filter:
            continue
        storey = GenericStorey(id=ifc_storey.GlobalId, name=ifc_storey.Name or "")
        storey.elevation = get_storey_elevation(ifc_storey, ifc_file)
        storeys.append(storey)
    storeys.sort(key=lambda s: s.elevation)

    slabs = []
    footprints = {}
    for slab in ifc_file.by_type('IfcSlab'):
        if has_ancestor(slab, 'IfcRoof'):
            continue  # measured with its roof, however deep it sits
        try:
            faces = get_faces(slab)
        except Exception as e:
            print("Warning: no geometry for slab {}: {}".format(slab.GlobalId, str(e)))
            continue
        if not faces:
            continue
        generic = slab_from_faces(slab, faces, storeys, to_unit)
        slabs.append(generic)
        if generic.kind == 'floor':
            footprints[generic.id] = get_footprint(faces)

    for roof in ifc_file.by_type('IfcRoof'):
        if has_ancestor(roof, 'IfcRoof'):
            continue  # part of a roof already measured
        generic = extract_ifc_roof_to_generic(roof, ifc_file, area_unit)
        if generic is not None:
            slabs.append(generic)

    if storey_filter:
        slabs = [s for s in slabs if s.storey == storey_filter]

    try:
        from shapely.ops import unary_union
    except ImportError:
        unary_union = None
    for storey in storeys:
        shapes = [footprints.get(i) for i in storey.slab_ids]
        if unary_union is not None and shapes and all(shape is not None for shape in shapes):
            storey.floor_area = round(unary_union(shapes).area * to_unit, 2)
        else:
            ids = set(storey.slab_ids)
            storey.floor_area = round(sum(s.area for s in slabs if s.id in ids), 2)

    building_model.storeys = storeys
    building_model.slabs = slabs

def is_glazing(part):
    """
    Whether a curtain-wall part is glass: an IfcWindow; a door whose name
    or type says glass or glazed (a framed entrance door with a glass
    light is not); or a panel whose name, type or material says so
    """
    if part.is_a('IfcWindow'):
        return True
    names = [part.Name or '', getattr(part, 'ObjectType', None) or '']
    if not part.is_a('IfcDoor'):
        try:
            names += [m.Name or '' for m in ifcopenshell.util.element.get_materials(part)]
        except Exception:
            pass
    text = ' '.join(names).lower()
    return 'glass' in text or 'glaz' in text

def extract_ifc_curtain_wall_to_generic(curtain_wall, ifc_file, area_unit='sqm', centre=None):
    """
    An IFC curtain wall as a GenericWall

    Its glazed panels become its windows and its frames and opaque panels
    its wall area, each measured as face area from the geometry, since
    exporters rarely give curtain walls quantities. Like Revit's, its
    orientation points inward: isCurtainWall tells the analyses to
    reverse it. Returns None when no part has geometry.
    """
    length_scale, _ = get_unit_scales(ifc_file)
    meshes = []
    for part in ifcopenshell.util.element.get_decomposition(curtain_wall):
        if not getattr(part, 'Representation', None) or part.is_a('IfcCurtainWall'):
            continue
        try:
            meshes.append((part, get_faces(part)))
        except Exception as e:
            print("Warning: no geometry for {}: {}".format(part.GlobalId, str(e)))
    if not meshes:
        return None

    # The facade faces along the horizontal normal that carries most area
    weights = {}
    total = 0.0
    sx = sy = 0.0
    for part, triangles in meshes:
        for n, area, (a, b, c) in triangles:
            total += area
            sx += (a[0] + b[0] + c[0]) / 3.0 * area
            sy += (a[1] + b[1] + c[1]) / 3.0 * area
            if abs(n[2]) > 0.1:
                continue
            key = (round(n[0], 2), round(n[1], 2))
            if key[0] < 0 or (key[0] == 0 and key[1] < 0):
                key = (-key[0], -key[1])
            weights[key] = weights.get(key, 0.0) + area
    if not weights:
        return None
    nx, ny = max(weights, key=weights.get)
    length = math.hypot(nx, ny)
    nx, ny = nx / length, ny / length
    if centre is not None and total > 0:
        mx = sx / total - centre[0] * length_scale
        my = sy / total - centre[1] * length_scale
        if mx * nx + my * ny < 0:
            nx, ny = -nx, -ny

    to_unit = 1.0 if area_unit == 'sqm' else SQM_TO_SQF
    level = get_storey_name(curtain_wall) or ""
    generic_wall = GenericWall(id=curtain_wall.GlobalId, name=curtain_wall.Name or "")
    generic_wall.orientation = GeometricVector(-nx, -ny, 0.0)
    generic_wall.is_curtain_wall = True
    generic_wall.is_external = get_is_external(curtain_wall)
    generic_wall.level = level
    generic_wall.wall_type = curtain_wall.ObjectType or ""
    generic_wall.properties['ifcType'] = curtain_wall.is_a()
    faces = [face for _, triangles in meshes for face in triangles]
    generic_wall.bottom, generic_wall.top = get_z_range(faces)
    axis = axis_from_faces(faces, (-ny, nx))
    if axis:
        generic_wall.start_point = GeometricVector(*axis[0])
        generic_wall.end_point = GeometricVector(*axis[1])

    opaque = 0.0
    for part, triangles in meshes:
        # each panel or frame shows two faces along the facade normal
        face = sum(area for n, area, _ in triangles if abs(n[0] * nx + n[1] * ny) > 0.99) / 2.0
        if face <= 0:
            continue
        if is_glazing(part):
            window = GenericWindow(id=part.GlobalId, name=part.Name or "")
            window.area = round(face * to_unit, 2)
            window.host_id = curtain_wall.GlobalId
            window.level = level
            generic_wall.windows.append(window)
        else:
            opaque += face
    generic_wall.area = round(opaque * to_unit, 2)
    return generic_wall


def extract_ifc_window_to_generic(window, ifc_file, area_unit='sqm'):
    """
    Extract an IFC window element and convert to GenericWindow
    """
    generic_window = GenericWindow(
        id=window.GlobalId,
        name=window.Name or ""
    )
    length_scale, area_scale = get_unit_scales(ifc_file)
    
    # Get Position
    pos = get_element_position(window)
    if pos:
        generic_window.position = pos

    # Try to find host wall
    try:
        for rel in window.FillsVoids:
            opening = rel.RelatingOpeningElement
            for fill_rel in opening.VoidsElements:
                host = fill_rel.RelatingBuildingElement
                if host.is_a('IfcWall'):
                    generic_window.host_id = host.GlobalId
                    break
    except:
        pass

    # Get dimensions from quantities
    try:
        for rel in window.IsDefinedBy:
            if rel.is_a('IfcRelDefinesByProperties'):
                prop_def = rel.RelatingPropertyDefinition
                if prop_def.is_a('IfcElementQuantity'):
                    for quantity in prop_def.Quantities:
                        if quantity.is_a('IfcQuantityLength'):
                            if 'Height' in quantity.Name:
                                generic_window.height = quantity.LengthValue
                            elif 'Width' in quantity.Name:
                                generic_window.width = quantity.LengthValue
                        elif quantity.is_a('IfcQuantityArea'):
                            area_sqm = quantity.AreaValue * area_scale
                            if area_unit == 'sqm':
                                generic_window.area = round(area_sqm, 1)
                            else:
                                generic_window.area = round(area_sqm * SQM_TO_SQF, 1)
    except:
        pass

    # The window's overall size when the quantities gave none
    if generic_window.height == 0 and getattr(window, 'OverallHeight', None):
        generic_window.height = window.OverallHeight
    if generic_window.width == 0 and getattr(window, 'OverallWidth', None):
        generic_window.width = window.OverallWidth

    # Calculate area if not found
    if generic_window.area == 0 and generic_window.height > 0 and generic_window.width > 0:
        area_sqm = generic_window.height * generic_window.width * length_scale ** 2
        if area_unit == 'sqm':
            generic_window.area = round(area_sqm, 1)
        else:
            generic_window.area = round(area_sqm * SQM_TO_SQF, 1)

    # Last resort: measure one side of the window's geometry
    if generic_window.area == 0:
        measured = get_side_area_from_geometry(window)
        if measured:
            if area_unit == 'sqm':
                generic_window.area = round(measured, 1)
            else:
                generic_window.area = round(measured * SQM_TO_SQF, 1)

    # Get level
    try:
        for rel in window.ContainedInStructure:
            if rel.is_a('IfcRelContainedInSpatialStructure'):
                container = rel.RelatingStructure
                if container.is_a('IfcBuildingStorey'):
                    generic_window.level = container.Name or ""
                    break
    except:
        pass

    return generic_window


def extract_building_model_from_ifc(ifc_path, options=None):
    """
    Extract a complete building model from IFC in generic format (see
    build_building_model). Faces are tessellated once per element while
    this runs.
    """
    global _FACES_CACHE
    _FACES_CACHE = {}
    try:
        return build_building_model(ifc_path, options)
    finally:
        _FACES_CACHE = None


def build_building_model(ifc_path, options=None):
    """
    Extract a complete building model from IFC in generic format

    Args:
        ifc_path: Path to IFC file
        options: Dictionary of extraction options
            - 'wallType': Filter by wall type
            - 'areaUnit': 'sqm' or 'sqf' (default: 'sqm')
            - 'includeWindows': Include windows (default: True)
            - 'storey': Filter by building storey name

    Returns:
        BuildingModel object
    """
    if not IFC_AVAILABLE:
        raise ImportError("ifcopenshell is required for IFC extraction. Install with: pip install ifcopenshell")

    options = options or {}
    area_unit = options.get('areaUnit', 'sqm')
    include_windows = options.get('includeWindows', True)
    wall_type_filter = options.get('wallType')
    storey_filter = options.get('storey')

    # Open IFC file
    try:
        ifc_file = ifcopenshell.open(ifc_path)
    except Exception as e:
        raise Exception("Failed to open IFC file: {}".format(str(e)))

    # Create building model
    building_model = BuildingModel()
    building_model.units = 'm' if area_unit == 'sqm' else 'ft'
    # Lets makit tell whose extraction a shared building-model.json is
    building_model.metadata['sourceFile'] = os.path.basename(ifc_path)

    # Get project info
    project = ifc_file.by_type('IfcProject')[0] if ifc_file.by_type('IfcProject') else None
    if project:
        building_model.metadata['projectName'] = project.Name or ""
        building_model.metadata['description'] = project.Description or ""

    # Get true north (IFC stores this in IfcGeometricRepresentationContext)
    try:
        contexts = ifc_file.by_type('IfcGeometricRepresentationContext')
        for context in contexts:
            if hasattr(context, 'TrueNorth') and context.TrueNorth:
                # TrueNorth is true north in project coordinates. The generic
                # format wants the clockwise angle that turns project
                # coordinates into true ones (analysis_engine.rotate_vector_2d):
                # 0 when true north is the project's +Y.
                north = context.TrueNorth.DirectionRatios
                building_model.project_north = math.atan2(-north[0], north[1])
                break
    except:
        building_model.project_north = 0.0

    # Extract walls
    wall_id_to_generic = {}
    walls = ifc_file.by_type('IfcWall')
    centre = get_building_centre(walls)

    for wall in walls:
        try:
            # Apply storey filter
            if storey_filter:
                wall_storey = None
                for rel in wall.ContainedInStructure:
                    if rel.is_a('IfcRelContainedInSpatialStructure'):
                        container = rel.RelatingStructure
                        if container.is_a('IfcBuildingStorey'):
                            wall_storey = container.Name
                            break

                if wall_storey != storey_filter:
                    continue

            # Apply wall type filter
            if wall_type_filter:
                wall_type = wall.ObjectType or ""
                if wall_type_filter.lower() not in wall_type.lower():
                    continue

            # Extract wall
            generic_wall = extract_ifc_wall_to_generic(wall, ifc_file, area_unit, centre)
            building_model.walls.append(generic_wall)
            wall_id_to_generic[wall.GlobalId] = generic_wall

        except Exception as e:
            print("Error extracting wall {}: {}".format(wall.GlobalId, str(e)))

    # Extract curtain walls; their glass becomes windows
    for curtain_wall in ifc_file.by_type('IfcCurtainWall'):
        try:
            parent = ifcopenshell.util.element.get_aggregate(curtain_wall)
            if parent is not None and parent.is_a('IfcCurtainWall'):
                continue  # part of a curtain wall already extracted
            if storey_filter and get_storey_name(curtain_wall) != storey_filter:
                continue
            if wall_type_filter and wall_type_filter.lower() not in (curtain_wall.ObjectType or '').lower():
                continue
            generic_wall = extract_ifc_curtain_wall_to_generic(curtain_wall, ifc_file, area_unit, centre)
            if generic_wall is None:
                continue
            if not include_windows:
                generic_wall.windows = []
            building_model.walls.append(generic_wall)
            building_model.windows.extend(generic_wall.windows)
        except Exception as e:
            print("Error extracting curtain wall {}: {}".format(curtain_wall.GlobalId, str(e)))

    # Extract windows if requested
    if include_windows:
        windows = ifc_file.by_type('IfcWindow')

        for window in windows:
            try:
                # Extract window
                generic_window = extract_ifc_window_to_generic(window, ifc_file, area_unit)
                building_model.windows.append(generic_window)

                # Add to host wall's window list
                if generic_window.host_id and generic_window.host_id in wall_id_to_generic:
                    host_wall = wall_id_to_generic[generic_window.host_id]
                    host_wall.windows.append(generic_window)

            except Exception as e:
                print("Error extracting window {}: {}".format(window.GlobalId, str(e)))

    # Storeys, floor plates and roofs
    extract_storeys_and_slabs(ifc_file, building_model, storey_filter, area_unit)

    return building_model


def analyze_ifc_walls(ifc_path, options=None):
    """
    Extract from IFC and immediately analyze

    Args:
        ifc_path: Path to IFC file
        options: Extraction and analysis options combined

    Returns:
        Analysis results dictionary
    """
    from analysis_engine import analyze_wall_orientations, generate_orientation_report

    # Extract building model
    building_model = extract_building_model_from_ifc(ifc_path, options)

    # Analyze
    stats = analyze_wall_orientations(building_model)

    # Generate report
    report = generate_orientation_report(stats)

    # Return combined results
    return {
        'stats': stats,
        'report': report,
        'buildingModel': building_model.to_dict()
    }
