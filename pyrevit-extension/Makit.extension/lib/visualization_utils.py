# -*- coding: utf-8 -*-
"""Utilities for generating simple 2D visualizations of building models"""

import math
import json
from analysis_engine import calculate_wall_orientation


def generate_elevation_visualization(building_model, direction_stats):
    """
    Generate visualization geometry from building model
    
    Args:
        building_model: BuildingModel object with walls and windows
        direction_stats: Dictionary with wall orientation statistics

    Returns:
        Dictionary with visualization geometry
    """
    # Always use isometric view now that we have real 3D coordinates
    return generate_isometric_view(building_model, direction_stats)


def generate_isometric_view(building_model, direction_stats):
    """
    Generate direction-specific views preserving 3D relative positioning
    
    Args:
        building_model: BuildingModel object
        direction_stats: Stats dict

    Returns:
        Dictionary with visualization geometry grouped by direction
    """
    visualizations = {}

    # Group walls by direction
    walls_by_direction = {}
    
    # Add an "Overview" bucket for the full 3D experience
    walls_by_direction['Overview'] = []
    
    for wall in building_model.walls:
        # Add to overview
        walls_by_direction['Overview'].append(wall)
        
        if not wall.orientation:
            continue

        direction = calculate_wall_orientation(wall, building_model.project_north, wall.is_curtain_wall)
        if direction not in walls_by_direction:
            walls_by_direction[direction] = []
        walls_by_direction[direction].append(wall)

    # Create view for each direction
    for direction, walls in walls_by_direction.items():
        vis_data = create_projected_view(walls, direction, building_model.windows)
        if vis_data:
            visualizations[direction] = vis_data

    return visualizations


def isometric_project(x, y, z):
    """
    Convert 3D coordinates to 2D isometric projection
    Standard isometric view from corner
    """
    # Scale factor
    scale = 3.0
    
    # Standard isometric projection
    # iso_x = (x - y) * cos(30)
    # iso_y = (x + y) * sin(30) - z
    
    cos30 = math.cos(math.radians(30))
    sin30 = math.sin(math.radians(30))
    
    iso_x = (x - y) * cos30 * scale
    iso_y = ((x + y) * sin30 - z) * scale

    return iso_x, iso_y


def create_projected_view(walls, direction, all_windows):
    """
    Create a projected view using actual 3D coordinates
    """
    faces = []
    
    wall_count = 0
    window_count = 0
    total_wall_area = 0
    total_window_area = 0

    for wall in walls:
        # Need start/end points for projection
        if not wall.start_point or not wall.end_point:
            continue

        wall_count += 1
        total_wall_area += wall.area
        
        # Wall corners in 3D
        # p1: Start Bottom
        x1, y1, z1 = wall.start_point.x, wall.start_point.y, wall.start_point.z
        # p2: End Bottom
        x2, y2, z2 = wall.end_point.x, wall.end_point.y, wall.end_point.z
        
        height = wall.height if wall.height > 0 else 3.0
        
        # p3: End Top
        x3, y3, z3 = x2, y2, z2 + height
        # p4: Start Top
        x4, y4, z4 = x1, y1, z1 + height
        
        # Project to 2D
        points = [
            isometric_project(x1, y1, z1),
            isometric_project(x2, y2, z2),
            isometric_project(x3, y3, z3),
            isometric_project(x4, y4, z4)
        ]
        
        faces.append({
            'points': points,
            'type': 'wall_face',
            'is_curtain': wall.is_curtain_wall,
            'wall_type': wall.wall_type
        })
        
        # Process windows hosted by this wall
        wall_windows = [w for w in all_windows if w.host_id == wall.id]
        
        # Vector along wall for window orientation
        wall_dx = x2 - x1
        wall_dy = y2 - y1
        wall_len = math.sqrt(wall_dx**2 + wall_dy**2)
        
        if wall_len > 0:
            dir_x = wall_dx / wall_len
            dir_y = wall_dy / wall_len
        else:
            dir_x, dir_y = 1, 0

        for window in wall_windows:
            window_count += 1
            total_window_area += window.area
            
            # Determine window position
            wx, wy, wz = x1, y1, z1 + 1.0 # Default fallback
            
            if window.position:
                wx, wy, wz = window.position.x, window.position.y, window.position.z
            else:
                # Fallback: Place in center if no position
                # But Phase 1 added position extraction, so this shouldn't be hit often
                # unless window extraction failed to find placement
                mid_dist = wall_len / 2
                wx = x1 + dir_x * mid_dist
                wy = y1 + dir_y * mid_dist
                wz = z1 + (height / 3) # Sill height estimate

            w_width = window.width if window.width > 0 else 1.0
            w_height = window.height if window.height > 0 else 1.5
            
            # Calculate window corners aligned with wall
            # Assume window plane is parallel to wall plane
            
            # Start (Left) Bottom
            wx1 = wx - (dir_x * w_width / 2)
            wy1 = wy - (dir_y * w_width / 2)
            wz1 = wz
            
            # End (Right) Bottom
            wx2 = wx + (dir_x * w_width / 2)
            wy2 = wy + (dir_y * w_width / 2)
            wz2 = wz
            
            # End Top
            wx3 = wx2
            wy3 = wy2
            wz3 = wz + w_height
            
            # Start Top
            wx4 = wx1
            wy4 = wy1
            wz4 = wz + w_height
            
            w_points = [
                isometric_project(wx1, wy1, wz1),
                isometric_project(wx2, wy2, wz2),
                isometric_project(wx3, wy3, wz3),
                isometric_project(wx4, wy4, wz4)
            ]
            
            faces.append({
                'points': w_points,
                'type': 'window_face'
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


def export_visualization_json(visualizations, output_file):
    """Export visualization data to JSON file"""
    with open(output_file, 'w') as f:
        json.dump(visualizations, f, indent=2)
