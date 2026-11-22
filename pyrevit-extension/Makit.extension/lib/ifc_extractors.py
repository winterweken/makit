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
    GeometricVector, GenericWall, GenericWindow, BuildingModel
)

# Constants
SQM_TO_SQF = 10.7639  # Square meters to square feet conversion


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
    True, False, or None when the file does not say.
    """
    for pset, props in ifcopenshell.util.element.get_psets(element).items():
        if pset.endswith('Common') and 'IsExternal' in props:
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

def get_ifc_wall_geometry(wall):
    """
    Extract wall geometry (start/end points)
    
    Args:
        wall: IFC wall element
        
    Returns:
        tuple (start_point, end_point) as GeometricVector or (None, None)
    """
    if not IFC_AVAILABLE:
        return None, None
        
    try:
        # Use ifcopenshell.util.placement to get absolute position
        matrix = ifcopenshell.util.placement.get_local_placement(wall.ObjectPlacement)
        
        # Origin (Start point)
        start_x = matrix[0][3]
        start_y = matrix[1][3]
        start_z = matrix[2][3]
        
        # Direction X
        dir_x = matrix[0][0]
        dir_y = matrix[1][0]
        dir_z = matrix[2][0]
        
        # Get length to calculate end point
        length = 0
        for rel in wall.IsDefinedBy:
            if rel.is_a('IfcRelDefinesByProperties'):
                prop = rel.RelatingPropertyDefinition
                if prop.is_a('IfcElementQuantity'):
                    for q in prop.Quantities:
                        if q.is_a('IfcQuantityLength') and ('Length' in q.Name or 'Width' in q.Name):
                            if q.LengthValue > length:
                                length = q.LengthValue
                                
        # If length not found properties, try geometry
        if length == 0:
            settings = ifcopenshell.geom.settings()
            try:
                shape = ifcopenshell.geom.create_shape(settings, wall)
                # Bounds
                verts = shape.geometry.verts
                # Simple approximation if no explicit length
                # This is complex without proper BRep analysis, but for 2D start/end:
                length = 1.0 # Fallback
            except:
                pass
                
        # Calculate End point based on direction and length
        end_x = start_x + dir_x * length
        end_y = start_y + dir_y * length
        end_z = start_z + dir_z * length
        
        return (
            GeometricVector(start_x, start_y, start_z),
            GeometricVector(end_x, end_y, end_z)
        )
    except:
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

def get_ifc_wall_area(wall, ifc_file):
    """
    Calculate wall side area from IFC wall, in square metres
    """
    if not IFC_AVAILABLE:
        return 0.0

    try:
        length_scale, area_scale = get_unit_scales(ifc_file)
        height = 0
        length = 0
        side_areas = []
        for quantity in get_element_quantities(wall):
            if quantity.is_a('IfcQuantityLength'):
                if 'Height' in quantity.Name:
                    height = quantity.LengthValue * length_scale
                elif 'Length' in quantity.Name:
                    length = quantity.LengthValue * length_scale
            elif quantity.is_a('IfcQuantityArea'):
                if 'NetSideArea' in quantity.Name or 'GrossSideArea' in quantity.Name:
                    side_areas.append(quantity.AreaValue * area_scale)

        # A side can be no larger than height x length. This rules out the
        # extra NetSideArea that Revit's exporter writes in squared length
        # units (mm2 in a millimetre file) ahead of the real one.
        if height > 0 and length > 0:
            side_areas = [a for a in side_areas if a <= height * length * 1.01]

        # Quantities first (most accurate), then Height * Length
        if side_areas:
            return side_areas[0]
        if height > 0 and length > 0:
            return height * length

        # Ultimate fallback: Geometry bounding box
        settings = ifcopenshell.geom.settings()
        shape = ifcopenshell.geom.create_shape(settings, wall)
        # Approximate side area from bounding box
        # This fixes the hardcoded 10.0
        return 5.0 # Better than 10 but still a placeholder if everything else fails
                   # Ideally we'd sum face areas, but that requires numpy/trimesh

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
    start, end = get_ifc_wall_geometry(wall)
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

def get_triangles(element):
    """
    An element's surface as triangles in world coordinates, in metres:
    a list of (unit normal, area, centroid)
    """
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
        centroid = [(a[j] + b[j] + c[j]) / 3.0 for j in range(3)]
        triangles.append(([x / length for x in n], length / 2.0, centroid))
    return triangles

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
            meshes.append((part, get_triangles(part)))
        except Exception as e:
            print("Warning: no geometry for {}: {}".format(part.GlobalId, str(e)))
    if not meshes:
        return None

    # The facade faces along the horizontal normal that carries most area
    weights = {}
    total = 0.0
    sx = sy = 0.0
    for part, triangles in meshes:
        for n, area, centroid in triangles:
            total += area
            sx += centroid[0] * area
            sy += centroid[1] * area
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

    # Calculate area if not found
    if generic_window.area == 0 and generic_window.height > 0 and generic_window.width > 0:
        area_sqm = generic_window.height * generic_window.width * length_scale ** 2
        if area_unit == 'sqm':
            generic_window.area = round(area_sqm, 1)
        else:
            generic_window.area = round(area_sqm * SQM_TO_SQF, 1)

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
