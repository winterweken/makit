#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Example: Custom analysis using the generic format

This shows how you can write your own analysis scripts
that work with data from ANY source (Revit, IFC, Rhino, etc.)
"""

import sys
import os
import json

# Add lib directory to path
lib_path = os.path.join(
    os.path.dirname(os.path.dirname(os.path.dirname(__file__))),
    'pyrevit-extension/Makit.extension/lib'
)
sys.path.insert(0, lib_path)

from geometry_models import BuildingModel
from analysis_engine import analyze_wall_orientations


def main():
    # Load the generic model
    model_path = 'building-model.json'

    with open(model_path) as f:
        model = BuildingModel.from_dict(json.load(f))

    print("=" * 60)
    print("CUSTOM ANALYSIS EXAMPLE")
    print("=" * 60)
    print()

    # Example 1: Find all walls with descriptions
    print("1. WALLS WITH DESCRIPTIONS:")
    print("-" * 60)
    for wall in model.walls:
        if 'description' in wall.properties:
            print("  • {} ({})".format(
                wall.name,
                wall.properties['description']
            ))

    print()

    # Example 2: Calculate wall area by level
    print("2. WALL AREA BY LEVEL:")
    print("-" * 60)
    by_level = {}
    for wall in model.walls:
        level = wall.level or "Unknown"
        by_level[level] = by_level.get(level, 0) + wall.area

    for level, area in sorted(by_level.items()):
        print("  {}: {:.1f} sq m".format(level, area))

    print()

    # Example 3: Find longest walls
    print("3. TOP 3 LONGEST WALLS:")
    print("-" * 60)
    sorted_walls = sorted(model.walls, key=lambda w: w.length, reverse=True)
    for i, wall in enumerate(sorted_walls[:3], 1):
        print("  {}. {} - {:.1f} m".format(i, wall.name, wall.length / 1000))

    print()

    # Example 4: Use the built-in analysis
    print("4. ORIENTATION ANALYSIS:")
    print("-" * 60)
    stats = analyze_wall_orientations(model)

    for direction in ['North', 'East', 'South', 'West']:
        data = stats['byDirection'][direction]
        if data['wallCount'] > 0:
            print("  {}: {} walls, {:.1f} sq m".format(
                direction,
                data['wallCount'],
                data['wallArea']
            ))

    print()
    print("=" * 60)
    print("This same script works with Revit, IFC, or any other source!")
    print("=" * 60)


if __name__ == '__main__':
    main()
