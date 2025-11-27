# -*- coding: utf-8 -*-
"""Platform-agnostic analysis functions for building geometry"""

import math
from collections import defaultdict


# CONSTANTS
DIRECTIONS = ['North', 'Northeast', 'East', 'Southeast',
              'South', 'Southwest', 'West', 'Northwest']


def rotate_vector_2d(x, y, radians):
    """
    Rotate a 2D vector by given angle in radians
    Used to transform project north to true north
    """
    new_x = x * math.cos(radians) + y * math.sin(radians)
    new_y = -x * math.sin(radians) + y * math.cos(radians)
    return round(new_x, 4), round(new_y, 4)


def classify_orientation(x, y):
    """
    Classify a 2D normalized vector into compass directions

    Args:
        x, y: Normalized 2D vector components

    Returns:
        Direction string (North, Northeast, etc.)
    """
    if x <= 0.3826 and x >= -0.3826 and y <= 1 and y >= 0.9238:
        return "North"
    elif x < 0.8660 and x > 0.3826 and y < 0.9238 and y > 0.5000:
        return "Northeast"
    elif x <= 1 and x >= 0.8660 and y <= 0.5000 and y >= -0.3583:
        return "East"
    elif x < 0.9335 and x > 0.3090 and y < -0.3583 and y > -0.9510:
        return "Southeast"
    elif x <= 0.3090 and x >= -0.3090 and y <= -0.9510 and y >= -1:
        return "South"
    elif x < -0.3090 and x > -0.9335 and y < -0.3583 and y > -0.9510:
        return "Southwest"
    elif x <= -0.8660 and x >= -1 and y <= 0.5000 and y >= -0.3583:
        return "West"
    elif x < -0.3826 and x > -0.8660 and y < 0.9238 and y > 0.5000:
        return "Northwest"
    else:
        return "Unknown"


def calculate_wall_orientation(wall, project_north_angle, invert=False):
    """
    Calculate the compass orientation of a wall

    Args:
        wall: GenericWall object
        project_north_angle: Angle in radians between project and true north
        invert: If True, invert the orientation (for curtain walls)

    Returns:
        Direction string
    """
    if not wall.orientation:
        return "Unknown"

    # Normalize the orientation vector
    normalized = wall.orientation.normalize()
    x, y = normalized.x, normalized.y

    # Transform to true north
    x, y = rotate_vector_2d(x, y, project_north_angle)

    # Invert for curtain walls if needed
    if invert:
        x, y = -x, -y

    return classify_orientation(x, y)


def calculate_wwr(window_area, wall_area):
    """
    Calculate Window-to-Wall Ratio as percentage

    Args:
        window_area: Total window area
        wall_area: Total wall area

    Returns:
        WWR as percentage (0-100)
    """
    total = wall_area + window_area
    if total > 0:
        return round((window_area / total) * 100, 1)
    return 0.0


def analyze_wall_orientations(building_model):
    """
    Analyze wall orientations and calculate WWR by direction

    Args:
        building_model: BuildingModel object or dict

    Returns:
        Analysis results dictionary
    """
    # Handle dict input (from JSON)
    if isinstance(building_model, dict):
        from geometry_models import BuildingModel
        building_model = BuildingModel.from_dict(building_model)

    # Get project north angle
    project_north = building_model.project_north

    # Initialize data structures
    stats = {
        'byDirection': {},
        'totals': {
            'wallCount': 0,
            'wallArea': 0.0,
            'windowCount': 0,
            'windowArea': 0.0
        },
        'projectNorth': project_north,
        'units': building_model.units
    }

    # Initialize direction stats
    for direction in DIRECTIONS:
        stats['byDirection'][direction] = {
            'wallCount': 0,
            'wallArea': 0.0,
            'windowCount': 0,
            'windowArea': 0.0,
            'wwr': 0.0,
            'wallIds': [],
            'windowIds': []
        }

    # Process walls and assign orientations
    wall_orientations = {}  # wall_id -> direction

    for wall in building_model.walls:
        # Calculate orientation
        invert = wall.is_curtain_wall
        direction = calculate_wall_orientation(wall, project_north, invert)
        wall_orientations[wall.id] = direction

        if direction in DIRECTIONS:
            dir_stats = stats['byDirection'][direction]
            dir_stats['wallCount'] += 1
            dir_stats['wallArea'] += wall.area
            dir_stats['wallIds'].append(wall.id)

            stats['totals']['wallCount'] += 1
            stats['totals']['wallArea'] += wall.area

            # Process windows on this wall
            for window in wall.windows:
                dir_stats['windowCount'] += 1
                dir_stats['windowArea'] += window.area
                dir_stats['windowIds'].append(window.id)

                stats['totals']['windowCount'] += 1
                stats['totals']['windowArea'] += window.area

    # Calculate WWR for each direction
    for direction in DIRECTIONS:
        dir_stats = stats['byDirection'][direction]
        dir_stats['wwr'] = calculate_wwr(
            dir_stats['windowArea'],
            dir_stats['wallArea']
        )

    # Calculate overall WWR
    stats['totals']['wwr'] = calculate_wwr(
        stats['totals']['windowArea'],
        stats['totals']['wallArea']
    )

    # Add wall orientations mapping
    stats['wallOrientations'] = wall_orientations

    return stats


def generate_orientation_report(stats):
    """
    Generate a human-readable text report from analysis stats

    Args:
        stats: Analysis results from analyze_wall_orientations()

    Returns:
        Formatted text report string
    """
    lines = []
    lines.append("=" * 50)
    lines.append("WALL ORIENTATION & WWR ANALYSIS")
    lines.append("=" * 50)
    lines.append("")

    # Overall stats
    lines.append("OVERALL SUMMARY")
    lines.append("-" * 50)
    lines.append("Total Walls: {}".format(stats['totals']['wallCount']))
    lines.append("Total Wall Area: {:.1f} sq {}".format(
        stats['totals']['wallArea'], stats['units']))
    lines.append("Total Windows: {}".format(stats['totals']['windowCount']))
    lines.append("Total Window Area: {:.1f} sq {}".format(
        stats['totals']['windowArea'], stats['units']))
    lines.append("Overall WWR: {:.1f}%".format(stats['totals']['wwr']))
    lines.append("")

    # By direction
    lines.append("BY DIRECTION")
    lines.append("-" * 50)

    for direction in DIRECTIONS:
        dir_stats = stats['byDirection'][direction]

        # Skip if no walls in this direction
        if dir_stats['wallCount'] == 0:
            continue

        lines.append("")
        lines.append("{} FACING:".format(direction.upper()))
        lines.append("  Walls: {}".format(dir_stats['wallCount']))
        lines.append("  Wall Area: {:.1f} sq {}".format(
            dir_stats['wallArea'], stats['units']))
        lines.append("  Windows: {}".format(dir_stats['windowCount']))
        lines.append("  Window Area: {:.1f} sq {}".format(
            dir_stats['windowArea'], stats['units']))
        lines.append("  WWR: {:.1f}%".format(dir_stats['wwr']))

    lines.append("")
    lines.append("=" * 50)

    return "\n".join(lines)


def filter_walls_by_criteria(building_model, criteria):
    """
    Filter walls based on various criteria

    Args:
        building_model: BuildingModel object
        criteria: Dictionary of filter criteria
            - 'type': wall type (exterior, interior, etc.)
            - 'workset': workset name
            - 'level': level name
            - 'minArea': minimum area
            - 'maxArea': maximum area

    Returns:
        Filtered BuildingModel
    """
    from geometry_models import BuildingModel

    filtered_model = BuildingModel()
    filtered_model.project_north = building_model.project_north
    filtered_model.units = building_model.units
    filtered_model.metadata = building_model.metadata.copy()

    for wall in building_model.walls:
        # Apply filters
        if 'type' in criteria and wall.wall_type != criteria['type']:
            continue
        if 'workset' in criteria and wall.workset != criteria['workset']:
            continue
        if 'level' in criteria and wall.level != criteria['level']:
            continue
        if 'minArea' in criteria and wall.area < criteria['minArea']:
            continue
        if 'maxArea' in criteria and wall.area > criteria['maxArea']:
            continue

        filtered_model.walls.append(wall)

    return filtered_model
