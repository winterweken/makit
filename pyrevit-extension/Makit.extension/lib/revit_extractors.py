# -*- coding: utf-8 -*-
"""Revit-specific extraction functions that convert to generic format"""

from Autodesk.Revit.DB import (
    BuiltInCategory, BuiltInParameter, FilteredElementCollector,
    XYZ
)
from pyrevit import revit

# Import generic models
from geometry_models import (
    GeometricVector, GenericWall, GenericWindow, BuildingModel
)

# Constants
SQF_TO_SQM = 10.7639  # Square feet to square meters


def get_workset_name(element):
    """Get workset name for an element"""
    if hasattr(element, "WorksetId"):
        try:
            workset = element.Document.GetWorksetTable().GetWorkset(element.WorksetId)
            return workset.Name
        except:
            pass
    return ""


def get_project_north_angle(doc):
    """
    Get the angle between project north and true north in radians

    Args:
        doc: Revit Document

    Returns:
        Angle in radians (negated for correct transformation)
    """
    try:
        return doc.ActiveProjectLocation.get_ProjectPosition(XYZ(0, 0, 0)).Angle * -1
    except:
        return 0.0


def extract_revit_wall_to_generic(wall, doc, area_unit='sqm'):
    """
    Extract a Revit wall element and convert to GenericWall

    Args:
        wall: Revit Wall element
        doc: Revit Document
        area_unit: 'sqm' or 'sqf' for area units

    Returns:
        GenericWall object
    """
    generic_wall = GenericWall(
        id=wall.Id.IntegerValue,
        name=wall.Name
    )

    # Get orientation
    try:
        orientation_xyz = wall.Orientation.Normalize()
        generic_wall.orientation = GeometricVector.from_revit_xyz(orientation_xyz)
    except:
        pass

    # Get area
    try:
        area_param = wall.get_Parameter(BuiltInParameter.HOST_AREA_COMPUTED)
        if area_param and area_param.HasValue:
            area_sqf = area_param.AsDouble()
            if area_unit == 'sqm':
                generic_wall.area = round(area_sqf / SQF_TO_SQM, 1)
            else:
                generic_wall.area = round(area_sqf, 1)
    except:
        pass

    # Get wall type (exterior/interior)
    try:
        wall_type_param = wall.WallType.get_Parameter(BuiltInParameter.FUNCTION_PARAM)
        if wall_type_param:
            generic_wall.wall_type = wall_type_param.AsValueString() or ""
    except:
        pass

    # Get workset
    generic_wall.workset = get_workset_name(wall)

    # Check if curtain wall
    generic_wall.is_curtain_wall = getattr(wall, 'CurtainGrid', None) is not None

    # Get level
    try:
        level_param = wall.get_Parameter(BuiltInParameter.WALL_BASE_CONSTRAINT)
        if level_param:
            level_id = level_param.AsElementId()
            level = doc.GetElement(level_id)
            if level:
                generic_wall.level = level.Name
    except:
        pass

    # Get dimensions
    try:
        # Height
        height_param = wall.get_Parameter(BuiltInParameter.WALL_USER_HEIGHT_PARAM)
        if height_param and height_param.HasValue:
            generic_wall.height = height_param.AsDouble()

        # Length
        length_param = wall.get_Parameter(BuiltInParameter.CURVE_ELEM_LENGTH)
        if length_param and length_param.HasValue:
            generic_wall.length = length_param.AsDouble()

        # Width
        width_param = wall.get_Parameter(BuiltInParameter.WALL_ATTR_WIDTH_PARAM)
        if width_param and width_param.HasValue:
            generic_wall.width = width_param.AsDouble()
    except:
        pass

    # Store additional properties
    try:
        comments_param = wall.get_Parameter(BuiltInParameter.ALL_MODEL_INSTANCE_COMMENTS)
        if comments_param and comments_param.HasValue:
            generic_wall.properties['comments'] = comments_param.AsString() or ""
    except:
        pass

    return generic_wall


def extract_revit_window_to_generic(window, doc, area_unit='sqm'):
    """
    Extract a Revit window element and convert to GenericWindow

    Args:
        window: Revit Window element
        doc: Revit Document
        area_unit: 'sqm' or 'sqf' for area units

    Returns:
        GenericWindow object
    """
    generic_window = GenericWindow(
        id=window.Id.IntegerValue,
        name=window.Name
    )

    # Get host wall ID
    try:
        host = window.Host
        if host:
            generic_window.host_id = str(host.Id.IntegerValue)
    except:
        pass

    # Get window dimensions from type
    try:
        window_type = doc.GetElement(window.GetTypeId())
        if window_type:
            height_param = window_type.get_Parameter(BuiltInParameter.WINDOW_HEIGHT)
            width_param = window_type.get_Parameter(BuiltInParameter.WINDOW_WIDTH)

            if height_param and height_param.HasValue:
                generic_window.height = height_param.AsDouble()

            if width_param and width_param.HasValue:
                generic_window.width = width_param.AsDouble()

            # Calculate area
            if generic_window.height > 0 and generic_window.width > 0:
                area_sqf = generic_window.height * generic_window.width
                if area_unit == 'sqm':
                    generic_window.area = round(area_sqf / SQF_TO_SQM, 1)
                else:
                    generic_window.area = round(area_sqf, 1)
    except:
        pass

    # Get level
    try:
        level_param = window.get_Parameter(BuiltInParameter.FAMILY_LEVEL_PARAM)
        if level_param:
            level_id = level_param.AsElementId()
            level = doc.GetElement(level_id)
            if level:
                generic_window.level = level.Name
    except:
        pass

    return generic_window


def extract_building_model_from_revit(options=None):
    """
    Extract a complete building model from Revit in generic format

    Args:
        options: Dictionary of extraction options
            - 'workset': Filter by workset name (e.g., 'QAL_ENVELOPE')
            - 'wallType': Filter by wall type (e.g., 'Exterior')
            - 'areaUnit': 'sqm' or 'sqf' (default: 'sqm')
            - 'includeWindows': Include windows (default: True)

    Returns:
        BuildingModel object
    """
    doc = revit.doc
    options = options or {}

    # Extract options
    workset_filter = options.get('workset')
    wall_type_filter = options.get('wallType')
    area_unit = options.get('areaUnit', 'sqm')
    include_windows = options.get('includeWindows', True)

    # Create building model
    building_model = BuildingModel()
    building_model.project_north = get_project_north_angle(doc)
    building_model.units = 'm' if area_unit == 'sqm' else 'ft'

    # Add metadata
    building_model.metadata = {
        'projectName': doc.Title,
        'isWorkshared': doc.IsWorkshared
    }

    # Collect all walls
    wall_collector = FilteredElementCollector(doc)\
        .OfCategory(BuiltInCategory.OST_Walls)\
        .WhereElementIsNotElementType()

    wall_id_to_generic = {}  # Map Revit ID to GenericWall

    for wall in wall_collector:
        try:
            # Apply filters
            if wall_type_filter:
                wall_type_param = wall.WallType.get_Parameter(
                    BuiltInParameter.FUNCTION_PARAM)
                if not wall_type_param or wall_type_param.AsValueString() != wall_type_filter:
                    continue

            if workset_filter:
                if get_workset_name(wall) != workset_filter:
                    continue

            # Extract wall
            generic_wall = extract_revit_wall_to_generic(wall, doc, area_unit)
            building_model.walls.append(generic_wall)
            wall_id_to_generic[str(wall.Id.IntegerValue)] = generic_wall

        except Exception as e:
            print("Error extracting wall {}: {}".format(wall.Id, str(e)))

    # Collect windows if requested
    if include_windows:
        window_collector = FilteredElementCollector(doc)\
            .OfCategory(BuiltInCategory.OST_Windows)\
            .WhereElementIsNotElementType()

        for window in window_collector:
            try:
                # Apply workset filter
                if workset_filter:
                    if get_workset_name(window) != workset_filter:
                        continue

                # Extract window
                generic_window = extract_revit_window_to_generic(window, doc, area_unit)

                # Add to building model
                building_model.windows.append(generic_window)

                # Also add to host wall's window list
                if generic_window.host_id and generic_window.host_id in wall_id_to_generic:
                    host_wall = wall_id_to_generic[generic_window.host_id]
                    host_wall.windows.append(generic_window)

            except Exception as e:
                print("Error extracting window {}: {}".format(window.Id, str(e)))

    return building_model


def analyze_walls_direct_from_revit(options=None):
    """
    HYBRID APPROACH: Extract from Revit and immediately analyze

    This combines extraction and analysis in one step for efficiency
    when working directly in Revit.

    Args:
        options: Extraction and analysis options combined

    Returns:
        Analysis results dictionary
    """
    from analysis_engine import analyze_wall_orientations, generate_orientation_report

    # Extract building model
    building_model = extract_building_model_from_revit(options)

    # Analyze
    stats = analyze_wall_orientations(building_model)

    # Generate report
    report = generate_orientation_report(stats)

    # Return combined results
    return {
        'stats': stats,
        'report': report,
        'buildingModel': building_model.to_dict()  # Include for caching/reuse
    }
