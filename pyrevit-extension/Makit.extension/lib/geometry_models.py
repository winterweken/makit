# -*- coding: utf-8 -*-
"""Generic geometry models for cross-platform analysis"""

# Version of the generic JSON format this module writes. Version 2 added
# storeys, slabs and wall elevations; readers treat a missing version as 1.
FORMAT_VERSION = 2


class GeometricVector(object):
    """Generic 3D vector representation"""

    def __init__(self, x, y, z):
        self.x = float(x)
        self.y = float(y)
        self.z = float(z)

    def to_dict(self):
        return {'x': self.x, 'y': self.y, 'z': self.z}

    @staticmethod
    def from_dict(data):
        return GeometricVector(data['x'], data['y'], data['z'])

    @staticmethod
    def from_revit_xyz(xyz):
        """Create from Revit XYZ object"""
        return GeometricVector(xyz.X, xyz.Y, xyz.Z)

    def normalize(self):
        """Return normalized vector"""
        import math
        length = math.sqrt(self.x**2 + self.y**2 + self.z**2)
        if length == 0:
            return GeometricVector(0, 0, 0)
        return GeometricVector(self.x / length, self.y / length, self.z / length)


class GenericWall(object):
    """Generic wall representation - platform agnostic"""

    def __init__(self, id, name=None):
        self.id = str(id)
        self.name = name or ""
        self.orientation = None  # GeometricVector
        self.start_point = None  # GeometricVector
        self.end_point = None    # GeometricVector
        self.area = 0.0  # in square meters
        self.height = 0.0
        self.length = 0.0
        self.width = 0.0
        self.wall_type = ""  # exterior, interior, etc.
        self.workset = ""
        self.is_curtain_wall = False
        self.is_external = None  # True, False (interior) or None (not known)
        self.level = ""
        self.bottom = None  # metres, project z of the wall's underside
        self.top = None  # metres, project z of the wall's top
        self.windows = []  # List of GenericWindow
        self.properties = {}  # Custom properties

    def to_dict(self):
        return {
            'id': self.id,
            'name': self.name,
            'orientation': self.orientation.to_dict() if self.orientation else None,
            'startPoint': self.start_point.to_dict() if self.start_point else None,
            'endPoint': self.end_point.to_dict() if self.end_point else None,
            'area': self.area,
            'height': self.height,
            'length': self.length,
            'width': self.width,
            'type': self.wall_type,
            'workset': self.workset,
            'isCurtainWall': self.is_curtain_wall,
            'isExternal': self.is_external,
            'level': self.level,
            'bottom': self.bottom,
            'top': self.top,
            'windows': [w.to_dict() for w in self.windows],
            'properties': self.properties
        }

    @staticmethod
    def from_dict(data):
        wall = GenericWall(data['id'], data.get('name'))
        if data.get('orientation'):
            wall.orientation = GeometricVector.from_dict(data['orientation'])
        if data.get('startPoint'):
            wall.start_point = GeometricVector.from_dict(data['startPoint'])
        if data.get('endPoint'):
            wall.end_point = GeometricVector.from_dict(data['endPoint'])
        wall.area = data.get('area', 0.0)
        wall.height = data.get('height', 0.0)
        wall.length = data.get('length', 0.0)
        wall.width = data.get('width', 0.0)
        wall.wall_type = data.get('type', '')
        wall.workset = data.get('workset', '')
        wall.is_curtain_wall = data.get('isCurtainWall', False)
        wall.is_external = data.get('isExternal')
        wall.level = data.get('level', '')
        wall.bottom = data.get('bottom')
        wall.top = data.get('top')
        wall.windows = [GenericWindow.from_dict(w) for w in data.get('windows', [])]
        wall.properties = data.get('properties', {})
        return wall


class GenericWindow(object):
    """Generic window representation"""

    def __init__(self, id, name=None):
        self.id = str(id)
        self.name = name or ""
        self.area = 0.0  # in square meters
        self.height = 0.0
        self.width = 0.0
        self.position = None     # GeometricVector (center)
        self.host_id = None  # ID of host wall
        self.level = ""
        self.properties = {}

    def to_dict(self):
        return {
            'id': self.id,
            'name': self.name,
            'area': self.area,
            'height': self.height,
            'width': self.width,
            'position': self.position.to_dict() if self.position else None,
            'hostId': self.host_id,
            'level': self.level,
            'properties': self.properties
        }

    @staticmethod
    def from_dict(data):
        window = GenericWindow(data['id'], data.get('name'))
        window.area = data.get('area', 0.0)
        window.height = data.get('height', 0.0)
        window.width = data.get('width', 0.0)
        if data.get('position'):
            window.position = GeometricVector.from_dict(data['position'])
        window.host_id = data.get('hostId')
        window.level = data.get('level', '')
        window.properties = data.get('properties', {})
        return window


class GenericStorey(object):
    """A building storey: its elevation and the floor area of its plates"""

    def __init__(self, id, name=None):
        self.id = str(id)
        self.name = name or ""
        self.elevation = 0.0  # metres, project z
        self.floor_area = 0.0  # square metres: the footprint union of its floor plates
        self.slab_ids = []  # ids of the GenericSlab plates that make up the floor

    def to_dict(self):
        return {
            'id': self.id,
            'name': self.name,
            'elevation': self.elevation,
            'floorArea': self.floor_area,
            'slabIds': list(self.slab_ids)
        }

    @staticmethod
    def from_dict(data):
        storey = GenericStorey(data['id'], data.get('name'))
        storey.elevation = data.get('elevation', 0.0)
        storey.floor_area = data.get('floorArea', 0.0)
        storey.slab_ids = list(data.get('slabIds', []))
        return storey


class GenericSlab(object):
    """A slab or roof: a floor plate (footprint area, assigned to a storey),
    a roof (up-facing surface area), a stair landing, or another part"""

    def __init__(self, id, name=None):
        self.id = str(id)
        self.name = name or ""
        self.kind = 'floor'  # 'floor', 'roof', 'landing' or 'other'
        self.area = 0.0  # square metres: footprint for floors, surface for roofs
        self.top = None  # metres, project z
        self.bottom = None
        self.storey = None  # name of the storey a floor plate belongs to
        self.slab_type = ""
        self.is_external = None
        self.properties = {}

    def to_dict(self):
        return {
            'id': self.id,
            'name': self.name,
            'kind': self.kind,
            'area': self.area,
            'top': self.top,
            'bottom': self.bottom,
            'storey': self.storey,
            'type': self.slab_type,
            'isExternal': self.is_external,
            'properties': self.properties
        }

    @staticmethod
    def from_dict(data):
        slab = GenericSlab(data['id'], data.get('name'))
        slab.kind = data.get('kind', 'floor')
        slab.area = data.get('area', 0.0)
        slab.top = data.get('top')
        slab.bottom = data.get('bottom')
        slab.storey = data.get('storey')
        slab.slab_type = data.get('type', '')
        slab.is_external = data.get('isExternal')
        slab.properties = data.get('properties', {})
        return slab


class BuildingModel(object):
    """Generic building model container"""

    def __init__(self):
        self.walls = []
        self.windows = []
        self.storeys = []  # List of GenericStorey
        self.slabs = []  # List of GenericSlab (floor plates, roofs, landings)
        self.project_north = 0.0  # radians
        self.units = "m"  # meters
        self.metadata = {}

    def to_dict(self):
        metadata = dict(self.metadata)
        metadata['formatVersion'] = FORMAT_VERSION
        return {
            'walls': [w.to_dict() for w in self.walls],
            'windows': [w.to_dict() for w in self.windows],
            'storeys': [s.to_dict() for s in self.storeys],
            'slabs': [s.to_dict() for s in self.slabs],
            'projectNorth': self.project_north,
            'units': self.units,
            'metadata': metadata
        }

    @staticmethod
    def from_dict(data):
        model = BuildingModel()
        model.walls = [GenericWall.from_dict(w) for w in data.get('walls', [])]
        model.windows = [GenericWindow.from_dict(w) for w in data.get('windows', [])]
        model.storeys = [GenericStorey.from_dict(s) for s in data.get('storeys', [])]
        model.slabs = [GenericSlab.from_dict(s) for s in data.get('slabs', [])]
        model.project_north = data.get('projectNorth', 0.0)
        model.units = data.get('units', 'm')
        model.metadata = data.get('metadata', {})
        return model
