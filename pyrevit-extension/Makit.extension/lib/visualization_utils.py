# -*- coding: utf-8 -*-
"""Utilities for generating simple 2D visualizations of building models"""

import math
import json
from analysis_engine import calculate_wall_orientation


def generate_elevation_visualization(building_model, direction_stats):
    """
    Generate simple 2D elevation views or isometric view for walls and windows

    Args:
        building_model: BuildingModel object with walls and windows
        direction_stats: Dictionary with wall orientation statistics

    Returns:
        Dictionary with visualization geometry
    """
    # For complex buildings (>50 walls), use isometric view
    if len(building_model.walls) > 50:
        return generate_isometric_view(building_model, direction_stats)

    # For simple buildings, use elevation views
    visualizations = {}

    # Group walls by direction using the same logic as analysis
    walls_by_direction = {}
    for wall in building_model.walls:
        if not wall.orientation:
            continue

        # Use the same direction calculation as the analysis engine
        direction = calculate_wall_orientation(wall, building_model.project_north, wall.is_curtain_wall)
        if direction not in walls_by_direction:
            walls_by_direction[direction] = []
        walls_by_direction[direction].append(wall)

    # Generate visualization for each direction
    for direction, walls in walls_by_direction.items():
        vis_data = create_elevation_view(walls, direction)
        visualizations[direction] = vis_data

    return visualizations


def generate_isometric_view(building_model, direction_stats):
    """
    Generate direction-specific elevation views for each compass direction

    Args:
        building_model: BuildingModel object with walls and windows
        direction_stats: Dictionary with wall orientation statistics

    Returns:
        Dictionary with visualization geometry grouped by direction
    """
    visualizations = {}

    # Group walls by direction
    walls_by_direction = {}
    for wall in building_model.walls:
        if not wall.orientation:
            continue

        direction = calculate_wall_orientation(wall, building_model.project_north, wall.is_curtain_wall)
        if direction not in walls_by_direction:
            walls_by_direction[direction] = []
        walls_by_direction[direction].append(wall)

    # Create elevation view for each direction
    for direction, walls in walls_by_direction.items():
        vis_data = create_detailed_elevation(walls, direction, building_model.windows)
        if vis_data:
            visualizations[direction] = vis_data

    return visualizations


def create_detailed_elevation(walls, direction, all_windows):
    """
    Create a detailed elevation view for a specific direction

    Args:
        walls: List of walls facing this direction
        direction: Direction name
        all_windows: All windows in the building

    Returns:
        Dictionary with faces and stats for this direction
    """
    faces = []
    wall_data_list = []

    # Layout walls horizontally in elevation view
    x_offset = 0
    scale = 10.0

    total_wall_area = 0
    total_window_area = 0
    wall_count = 0
    window_count = 0

    for wall in walls:
        if not wall.length or wall.length == 0:
            continue

        length = wall.length * scale
        height = (wall.height if wall.height > 0 else 3.0) * scale

        # Get windows for this wall
        wall_windows = [w for w in all_windows if w.host_id == wall.id]

        # Wall corners (simple rectangle in 2D)
        corners = [
            (x_offset, 0),
            (x_offset + length, 0),
            (x_offset + length, height),
            (x_offset, height)
        ]

        # Add wall face
        wall_data_list.append({
            'corners': corners,
            'windows': wall_windows,
            'type': 'wall_face'
        })

        # Track stats
        total_wall_area += wall.area
        wall_count += 1

        # Add window faces on this wall
        if wall_windows:
            num_windows = len(wall_windows)
            window_width = length * 0.25  # Window takes 25% of wall width
            window_height = height * 0.4   # Window takes 40% of wall height
            spacing = length / (num_windows + 1)

            for i, window in enumerate(wall_windows):
                win_x = x_offset + spacing * (i + 1) - window_width / 2
                win_y = height * 0.3  # Start at 30% of wall height

                window_corners = [
                    (win_x, win_y),
                    (win_x + window_width, win_y),
                    (win_x + window_width, win_y + window_height),
                    (win_x, win_y + window_height)
                ]

                wall_data_list.append({
                    'corners': window_corners,
                    'type': 'window_face'
                })

                total_window_area += window.area
                window_count += 1

        # Move to next wall position
        x_offset += length + (2 * scale)  # Small gap

    # Create faces from wall data (no isometric projection, just 2D elevation)
    for wall_data in wall_data_list:
        faces.append({
            'points': wall_data['corners'],
            'type': wall_data['type']
        })

    # Calculate WWR
    wwr = 0.0
    if total_wall_area > 0:
        wwr = (total_window_area / (total_wall_area + total_window_area)) * 100

    return {
        'direction': direction,
        'faces': faces,
        'stats': {
            'walls': wall_count,
            'windows': window_count,
            'wall_area': total_wall_area,
            'window_area': total_window_area,
            'wwr': wwr
        }
    }


def distance_2d(p1, p2):
    """Calculate 2D distance between points"""
    return math.sqrt((p2[0] - p1[0])**2 + (p2[1] - p1[1])**2)


def isometric_project(x, y, z):
    """
    Convert 3D coordinates to 2D isometric projection

    Args:
        x, y, z: 3D coordinates

    Returns:
        (iso_x, iso_y): 2D isometric coordinates
    """
    # Standard isometric projection angles
    # 30 degrees for x-axis, 30 degrees for y-axis
    iso_x = (x - y) * math.cos(math.radians(30))
    iso_y = (x + y) * math.sin(math.radians(30)) - z

    return iso_x, iso_y


def create_elevation_view(walls, direction):
    """
    Create a simple elevation view for walls facing a direction

    Args:
        walls: List of GenericWall objects
        direction: Cardinal direction string

    Returns:
        Dictionary with lines representing walls and windows
    """
    lines = []

    # Calculate total width and max height
    x_offset = 0
    max_height = max([w.height for w in walls]) if walls else 10.0

    # Use a fixed scale (meters to arbitrary units)
    scale = 10.0

    for wall in walls:
        # Wall dimensions
        width = wall.length * scale if wall.length > 0 else 10 * scale
        # Use default height if not available (typical floor height ~3m)
        default_height = 30.0 if wall.height == 0 else wall.height
        height = default_height * scale

        # Draw wall outline (rectangle)
        # Bottom line
        lines.append({
            'x1': x_offset,
            'y1': 0,
            'x2': x_offset + width,
            'y2': 0,
            'type': 'wall'
        })
        # Right line
        lines.append({
            'x1': x_offset + width,
            'y1': 0,
            'x2': x_offset + width,
            'y2': height,
            'type': 'wall'
        })
        # Top line
        lines.append({
            'x1': x_offset + width,
            'y1': height,
            'x2': x_offset,
            'y2': height,
            'type': 'wall'
        })
        # Left line
        lines.append({
            'x1': x_offset,
            'y1': height,
            'x2': x_offset,
            'y2': 0,
            'type': 'wall'
        })

        # Draw windows on this wall
        if wall.windows:
            window_count = len(wall.windows)
            window_spacing = width / (window_count + 1)

            for i, window in enumerate(wall.windows):
                # Calculate window position
                win_x = x_offset + window_spacing * (i + 1)
                win_width = window.width * scale if window.width > 0 else width * 0.15
                win_height = window.height * scale if window.height > 0 else height * 0.4

                # Center window horizontally in its space
                win_x -= win_width / 2

                # Position window vertically (centered, or at sill height)
                win_y = height * 0.3  # Typical sill height ratio

                # Draw window rectangle
                lines.append({
                    'x1': win_x,
                    'y1': win_y,
                    'x2': win_x + win_width,
                    'y2': win_y,
                    'type': 'window'
                })
                lines.append({
                    'x1': win_x + win_width,
                    'y1': win_y,
                    'x2': win_x + win_width,
                    'y2': win_y + win_height,
                    'type': 'window'
                })
                lines.append({
                    'x1': win_x + win_width,
                    'y1': win_y + win_height,
                    'x2': win_x,
                    'y2': win_y + win_height,
                    'type': 'window'
                })
                lines.append({
                    'x1': win_x,
                    'y1': win_y + win_height,
                    'x2': win_x,
                    'y2': win_y,
                    'type': 'window'
                })

        # Move to next wall position
        x_offset += width + (5 * scale)  # Small gap between walls

    return {
        'direction': direction,
        'lines': lines,
        'width': x_offset,
        'height': max_height * scale if max_height else 100
    }


def export_visualization_json(visualizations, output_file):
    """Export visualization data to JSON file"""
    with open(output_file, 'w') as f:
        json.dump(visualizations, f, indent=2)
