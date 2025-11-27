#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
Standalone IFC Wall Orientation Analysis Tool

This script analyzes IFC files and generates wall orientation and WWR reports.
It demonstrates the hybrid architecture working with IFC instead of Revit.

Usage:
    python analyze_ifc.py model.ifc
    python analyze_ifc.py model.ifc --output results.json
    python analyze_ifc.py model.ifc --storey "Level 1"
    python analyze_ifc.py model.ifc --unit sqf
"""

import sys
import os
import json
import argparse


def main():
    parser = argparse.ArgumentParser(
        description='Analyze wall orientations and WWR from IFC files'
    )

    parser.add_argument('ifc_file', help='Path to IFC file')
    parser.add_argument('--output', '-o', help='Save detailed JSON results to file')
    parser.add_argument('--unit', choices=['sqm', 'sqf'], default='sqm',
                        help='Area unit (default: sqm)')
    parser.add_argument('--storey', help='Filter by building storey name')
    parser.add_argument('--wall-type', help='Filter by wall type')
    parser.add_argument('--extract-only', action='store_true',
                        help='Only extract to generic format, skip analysis')

    args = parser.parse_args()

    # Check if file exists
    if not os.path.exists(args.ifc_file):
        print("Error: IFC file not found: {}".format(args.ifc_file))
        sys.exit(1)

    # Import analysis modules
    try:
        from ifc_extractors import extract_building_model_from_ifc, analyze_ifc_walls
        from analysis_engine import analyze_wall_orientations, generate_orientation_report
    except ImportError as e:
        print("Error: Required modules not found: {}".format(str(e)))
        print("\nMake sure you're running from the lib directory or have the modules in your path")
        sys.exit(1)

    # Build options
    options = {
        'areaUnit': args.unit,
        'includeWindows': True
    }

    if args.storey:
        options['storey'] = args.storey

    if args.wall_type:
        options['wallType'] = args.wall_type

    print("=" * 60)
    print("IFC WALL ORIENTATION ANALYSIS")
    print("=" * 60)
    print("File: {}".format(args.ifc_file))
    print("Unit: {}".format(args.unit))
    if args.storey:
        print("Storey: {}".format(args.storey))
    if args.wall_type:
        print("Wall Type: {}".format(args.wall_type))
    print()

    # Extract building model
    print("Extracting building model from IFC...")
    try:
        building_model = extract_building_model_from_ifc(args.ifc_file, options)
    except Exception as e:
        print("Error extracting IFC: {}".format(str(e)))
        sys.exit(1)

    print("Extracted {} walls and {} windows".format(
        len(building_model.walls),
        len(building_model.windows)
    ))

    # If extract-only, save and exit
    if args.extract_only:
        output_file = args.output or 'building-model.json'
        with open(output_file, 'w') as f:
            json.dump(building_model.to_dict(), f, indent=2)
        print("\nBuilding model saved to: {}".format(output_file))
        print("This generic format can be analyzed by other tools")
        return

    # Analyze
    print("\nAnalyzing orientations...")
    stats = analyze_wall_orientations(building_model)
    report = generate_orientation_report(stats)

    # Print report
    print()
    print(report)

    # Save detailed results if requested
    if args.output:
        result = {
            'stats': stats,
            'report': report,
            'buildingModel': building_model.to_dict()
        }

        with open(args.output, 'w') as f:
            json.dump(result, f, indent=2)

        print("\nDetailed results saved to: {}".format(args.output))


if __name__ == '__main__':
    main()
