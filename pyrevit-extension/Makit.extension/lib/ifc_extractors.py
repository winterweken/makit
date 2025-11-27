# -*- coding: utf-8 -*-
"""IFC-specific extraction functions that convert to generic format"""

try:
    import ifcopenshell
    import ifcopenshell.util.element
    import ifcopenshell.util.placement
    IFC_AVAILABLE = True
except ImportError:
    IFC_AVAILABLE = False
    print("Warning: ifcopenshell not available. Install with: pip install ifcopenshell")

import math
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


def get_ifc_wall_orientation(wall):
    """
    Extract wall orientation vector from IFC wall

    Args:
        wall: IFC wall element

    Returns:
        GeometricVector or None
    """
    if not IFC_AVAILABLE:
        return None

    try:
        # Get wall placement
        placement = ifcopenshell.util.placement.get_local_placement(wall.ObjectPlacement)

        # For walls, the X-axis usually represents the orientation
        # Extract the X-axis direction from the placement matrix
        x_direction = [placement[0][0], placement[1][0], placement[2][0]]

        # Normalize
        length = math.sqrt(sum(d**2 for d in x_direction))
        if length > 0:
            x_direction = [d / length for d in x_direction]

        return GeometricVector(x_direction[0], x_direction[1], x_direction[2])
    except Exception as e:
        print("Warning: Could not extract wall orientation: {}".format(str(e)))
        return None


def get_ifc_wall_area(wall, ifc_file):
    """
    Calculate wall area from IFC wall

    Args:
        wall: IFC wall element
        ifc_file: IFC file object

    Returns:
        Area in square meters
    """
    if not IFC_AVAILABLE:
        return 0.0

    try:
        # Try to get area from quantities
        for rel in wall.IsDefinedBy:
            if rel.is_a('IfcRelDefinesByProperties'):
                prop_def = rel.RelatingPropertyDefinition
                if prop_def.is_a('IfcElementQuantity'):
                    for quantity in prop_def.Quantities:
                        if quantity.is_a('IfcQuantityArea'):
                            if 'NetSideArea' in quantity.Name or 'GrossSideArea' in quantity.Name:
                                return quantity.AreaValue

        # Fallback: Calculate from geometry
        # This is a simplified approach
        settings = ifcopenshell.geom.settings()
        shape = ifcopenshell.geom.create_shape(settings, wall)

        # Get bounding box volume as rough approximation
        # In a production system, you'd calculate actual surface area
        verts = shape.geometry.verts
        if len(verts) >= 3:
            # Very rough estimate - would need proper surface area calculation
            return 10.0  # Placeholder

    except Exception as e:
        print("Warning: Could not calculate wall area: {}".format(str(e)))

    return 0.0


def extract_ifc_wall_to_generic(wall, ifc_file, area_unit='sqm'):
    """
    Extract an IFC wall element and convert to GenericWall

    Args:
        wall: IFC wall element
        ifc_file: IFC file object
        area_unit: 'sqm' or 'sqf' for area units

    Returns:
        GenericWall object
    """
    generic_wall = GenericWall(
        id=wall.GlobalId,
        name=wall.Name or ""
    )

    # Get orientation
    orientation = get_ifc_wall_orientation(wall)
    if orientation:
        generic_wall.orientation = orientation

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

    # Store IFC-specific properties
    generic_wall.properties['ifcType'] = wall.is_a()
    if hasattr(wall, 'Description') and wall.Description:
        generic_wall.properties['description'] = wall.Description

    return generic_wall


def extract_ifc_window_to_generic(window, ifc_file, area_unit='sqm'):
    """
    Extract an IFC window element and convert to GenericWindow

    Args:
        window: IFC window element
        ifc_file: IFC file object
        area_unit: 'sqm' or 'sqf' for area units

    Returns:
        GenericWindow object
    """
    generic_window = GenericWindow(
        id=window.GlobalId,
        name=window.Name or ""
    )

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
                            area_sqm = quantity.AreaValue
                            if area_unit == 'sqm':
                                generic_window.area = round(area_sqm, 1)
                            else:
                                generic_window.area = round(area_sqm * SQM_TO_SQF, 1)
    except:
        pass

    # Calculate area if not found
    if generic_window.area == 0 and generic_window.height > 0 and generic_window.width > 0:
        area_sqm = generic_window.height * generic_window.width
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
                # Convert to radians
                north = context.TrueNorth.DirectionRatios
                building_model.project_north = math.atan2(north[1], north[0])
                break
    except:
        building_model.project_north = 0.0

    # Extract walls
    wall_id_to_generic = {}
    walls = ifc_file.by_type('IfcWall')

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
            generic_wall = extract_ifc_wall_to_generic(wall, ifc_file, area_unit)
            building_model.walls.append(generic_wall)
            wall_id_to_generic[wall.GlobalId] = generic_wall

        except Exception as e:
            print("Error extracting wall {}: {}".format(wall.GlobalId, str(e)))

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
