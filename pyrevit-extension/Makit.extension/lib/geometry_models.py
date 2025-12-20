# -*- coding: utf-8 -*-
"""Generic geometry models for cross-platform analysis"""


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
        self.level = ""
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
            'level': self.level,
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
        wall.level = data.get('level', '')
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


class BuildingModel(object):
    """Generic building model container"""

    def __init__(self):
        self.walls = []
        self.windows = []
        self.project_north = 0.0  # radians
        self.units = "m"  # meters
        self.metadata = {}

    def to_dict(self):
        return {
            'walls': [w.to_dict() for w in self.walls],
            'windows': [w.to_dict() for w in self.windows],
            'projectNorth': self.project_north,
            'units': self.units,
            'metadata': self.metadata
        }

    @staticmethod
    def from_dict(data):
        model = BuildingModel()
        model.walls = [GenericWall.from_dict(w) for w in data.get('walls', [])]
        model.windows = [GenericWindow.from_dict(w) for w in data.get('windows', [])]
        model.project_north = data.get('projectNorth', 0.0)
        model.units = data.get('units', 'm')
        model.metadata = data.get('metadata', {})
        return model
