# -*- coding: utf-8 -*-
"""Geometry extraction from Revit models"""

from Autodesk.Revit.DB import *
from pyrevit import revit, HOST_APP

# Get active document
doc = revit.doc


def point_to_dict(point):
    """Convert XYZ point to dictionary"""
    return {
        'x': point.X,
        'y': point.Y,
        'z': point.Z
    }


def line_to_dict(line):
    """Convert Line to dictionary"""
    return {
        'start': point_to_dict(line.GetEndPoint(0)),
        'end': point_to_dict(line.GetEndPoint(1))
    }


def get_element_parameters(element):
    """Extract parameters from an element"""
    params = {}
    for param in element.Parameters:
        if param.HasValue:
            try:
                if param.StorageType == StorageType.String:
                    params[param.Definition.Name] = param.AsString()
                elif param.StorageType == StorageType.Integer:
                    params[param.Definition.Name] = param.AsInteger()
                elif param.StorageType == StorageType.Double:
                    params[param.Definition.Name] = param.AsDouble()
                elif param.StorageType == StorageType.ElementId:
                    params[param.Definition.Name] = param.AsElementId().IntegerValue
            except:
                pass
    return params


def get_bounding_box(element):
    """Get bounding box of an element"""
    try:
        bbox = element.get_BoundingBox(None)
        if bbox:
            return {
                'min': point_to_dict(bbox.Min),
                'max': point_to_dict(bbox.Max)
            }
    except:
        pass
    return {'min': {'x': 0, 'y': 0, 'z': 0}, 'max': {'x': 0, 'y': 0, 'z': 0}}


def extract_element_geometry(element):
    """Extract geometry lines from an element"""
    lines = []

    try:
        options = Options()
        options.DetailLevel = ViewDetailLevel.Fine
        geom = element.get_Geometry(options)

        if geom:
            for geom_obj in geom:
                if isinstance(geom_obj, Line):
                    lines.append(line_to_dict(geom_obj))
                elif isinstance(geom_obj, GeometryInstance):
                    inst_geom = geom_obj.GetInstanceGeometry()
                    for inst_obj in inst_geom:
                        if isinstance(inst_obj, Line):
                            lines.append(line_to_dict(inst_obj))
    except:
        pass

    return lines


def get_project_info():
    """Get project information"""
    proj_info = doc.ProjectInformation

    return {
        'name': doc.Title,
        'number': proj_info.Number if hasattr(proj_info, 'Number') else '',
        'address': proj_info.Address if hasattr(proj_info, 'Address') else '',
        'author': proj_info.Author if hasattr(proj_info, 'Author') else '',
        'clientName': proj_info.ClientName if hasattr(proj_info, 'ClientName') else '',
        'revitVersion': HOST_APP.version,
        'isWorkshared': doc.IsWorkshared,
        'centralPath': doc.GetWorksharingCentralModelPath().ToString() if doc.IsWorkshared else ''
    }


def extract_walls(options):
    """Extract wall elements from the model"""
    walls_collector = FilteredElementCollector(doc)\
        .OfCategory(BuiltInCategory.OST_Walls)\
        .WhereElementIsNotElementType()

    # Filter by level if specified
    level_name = options.get('level')
    if level_name:
        walls_collector = walls_collector.WherePasses(
            ElementLevelFilter(get_level_by_name(level_name).Id)
        )

    elements = []
    for wall in walls_collector:
        try:
            level_param = wall.get_Parameter(BuiltInParameter.WALL_BASE_CONSTRAINT)
            level_name = ''
            if level_param:
                level_id = level_param.AsElementId()
                level = doc.GetElement(level_id)
                if level:
                    level_name = level.Name

            element_data = {
                'id': wall.Id.IntegerValue,
                'category': wall.Category.Name if wall.Category else 'Wall',
                'name': wall.Name,
                'level': level_name,
                'lines': extract_element_geometry(wall),
                'boundingBox': get_bounding_box(wall),
                'parameters': get_element_parameters(wall)
            }
            elements.append(element_data)
        except Exception as e:
            print("Error processing wall %s: %s" % (wall.Id, str(e)))

    return {
        'elements': elements,
        'count': len(elements),
        'message': 'Successfully extracted %d walls' % len(elements)
    }


def extract_floors(options):
    """Extract floor elements from the model"""
    floors_collector = FilteredElementCollector(doc)\
        .OfCategory(BuiltInCategory.OST_Floors)\
        .WhereElementIsNotElementType()

    elements = []
    for floor in floors_collector:
        try:
            level_param = floor.get_Parameter(BuiltInParameter.LEVEL_PARAM)
            level_name = ''
            if level_param:
                level_id = level_param.AsElementId()
                level = doc.GetElement(level_id)
                if level:
                    level_name = level.Name

            element_data = {
                'id': floor.Id.IntegerValue,
                'category': floor.Category.Name if floor.Category else 'Floor',
                'name': floor.Name,
                'level': level_name,
                'lines': extract_element_geometry(floor),
                'boundingBox': get_bounding_box(floor),
                'parameters': get_element_parameters(floor)
            }
            elements.append(element_data)
        except Exception as e:
            print("Error processing floor %s: %s" % (floor.Id, str(e)))

    return {
        'elements': elements,
        'count': len(elements),
        'message': 'Successfully extracted %d floors' % len(elements)
    }


def extract_rooms(options):
    """Extract room elements from the model"""
    rooms_collector = FilteredElementCollector(doc)\
        .OfCategory(BuiltInCategory.OST_Rooms)\
        .WhereElementIsNotElementType()

    include_unplaced = options.get('includeUnplaced', False)

    rooms = []
    for room in rooms_collector:
        try:
            # Skip unplaced rooms if not requested
            if not include_unplaced and room.Area == 0:
                continue

            # Get room boundaries
            boundaries = []
            boundary_options = SpatialElementBoundaryOptions()
            boundary_segments = room.GetBoundarySegments(boundary_options)

            for segment_list in boundary_segments:
                for segment in segment_list:
                    curve = segment.GetCurve()
                    if isinstance(curve, Line):
                        boundaries.append(line_to_dict(curve))

            level = doc.GetElement(room.LevelId)
            level_name = level.Name if level else ''

            room_data = {
                'id': room.Id.IntegerValue,
                'name': room.get_Parameter(BuiltInParameter.ROOM_NAME).AsString() or '',
                'number': room.Number or '',
                'level': level_name,
                'area': room.Area,
                'perimeter': room.Perimeter,
                'height': room.UnboundedHeight,
                'boundaries': boundaries,
                'parameters': get_element_parameters(room)
            }
            rooms.append(room_data)
        except Exception as e:
            print("Error processing room %s: %s" % (room.Id, str(e)))

    return {
        'rooms': rooms,
        'count': len(rooms),
        'message': 'Successfully extracted %d rooms' % len(rooms)
    }


def get_level_by_name(level_name):
    """Get level by name"""
    levels = FilteredElementCollector(doc)\
        .OfClass(Level)\
        .WhereElementIsNotElementType()

    for level in levels:
        if level.Name == level_name:
            return level

    return None
