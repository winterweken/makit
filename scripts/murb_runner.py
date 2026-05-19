#!/usr/bin/env python3
"""
murb_runner.py — JSON bridge between makit (Rust) and murb_energy_tool (Python).

Protocol:
    1. Reads a JSON object from stdin
    2. Runs murb_energy_tool.simulation.Run with the provided parameters
    3. Writes a JSON results object to stdout

Input schema:
    {
        "epw_path": "/absolute/path/to/file.epw",
        "name": "MyBuilding",
        "province": "ON",
        "gfa": 5000.0,
        "area_walls_ag": 2400.0,
        "area_walls_bg": 0.0,
        "area_windows": 800.0,
        "area_roof": 1000.0,
        "u_walls_ag": 0.273,
        "u_windows": 2.56,
        "u_roof": 0.164,
        "cop_htg": 0.85,
        "cop_clg": 5.2,
        "hrv_efficiency": 0.55,
        "window_groups": [
            {"pct_window_area": 0.25, "window_azimuth": 0, "shgc": 0.4, "shading": 0.0},
            {"pct_window_area": 0.25, "window_azimuth": 90, "shgc": 0.4, "shading": 0.0},
            {"pct_window_area": 0.25, "window_azimuth": 180, "shgc": 0.4, "shading": 0.0},
            {"pct_window_area": 0.25, "window_azimuth": 270, "shgc": 0.4, "shading": 0.0}
        ]
    }

Output schema:
    {
        "name": "MyBuilding_TMY",
        "monthly_heating_demand": [12 floats],
        "monthly_cooling_demand": [12 floats],
        "monthly_heating_consumption": [12 floats],
        "monthly_cooling_consumption": [12 floats],
        "monthly_total_consumption": [12 floats],
        "monthly_ghg_emissions": [12 floats],
        "tedi_kwh_m2": float,
        "teui_kwh_m2": float,
        "ghgi_kg_m2": float,
        "gfa": float,
        "weather_file": str
    }
"""

import json
import os
import shutil
import sys
import tempfile
from pathlib import Path


def main():
    # Read JSON input from stdin
    try:
        raw = sys.stdin.read()
        params = json.loads(raw)
    except json.JSONDecodeError as e:
        print(json.dumps({"error": f"Invalid JSON input: {e}"}), file=sys.stderr)
        sys.exit(1)

    # Validate required fields
    required = ["epw_path", "gfa", "area_walls_ag", "area_windows", "area_roof"]
    for field in required:
        if field not in params:
            print(f"Missing required field: {field}", file=sys.stderr)
            sys.exit(1)

    epw_path = Path(params["epw_path"]).resolve()
    if not epw_path.exists():
        print(f"EPW file not found: {epw_path}", file=sys.stderr)
        sys.exit(1)

    # Import murb (deferred so validation errors come first)
    try:
        from murb_energy_tool import simulation
    except ImportError:
        print(
            "murb_energy_tool not installed. "
            "Install with: pip install -r requirements.txt "
            "(from the murb repository root)",
            file=sys.stderr,
        )
        sys.exit(1)

    # Build WindowGroup objects
    wg_dicts = params.get("window_groups", [
        {"pct_window_area": 0.25, "window_azimuth": 0, "shgc": 0.4, "shading": 0.0},
        {"pct_window_area": 0.25, "window_azimuth": 90, "shgc": 0.4, "shading": 0.0},
        {"pct_window_area": 0.25, "window_azimuth": 180, "shgc": 0.4, "shading": 0.0},
        {"pct_window_area": 0.25, "window_azimuth": 270, "shgc": 0.4, "shading": 0.0},
    ])
    window_groups = [simulation.WindowGroup(**wg) for wg in wg_dicts]

    # Run simulation in a temp directory (murb expects ./input/*.epw)
    original_cwd = os.getcwd()
    with tempfile.TemporaryDirectory(prefix="makit_murb_") as td:
        td_path = Path(td)
        input_dir = td_path / "input"
        input_dir.mkdir()
        shutil.copy2(str(epw_path), str(input_dir / epw_path.name))

        try:
            os.chdir(td_path)
            run = simulation.Run(
                name=params.get("name", "makit_run"),
                province=params.get("province", "ON"),
                gfa=float(params["gfa"]),
                area_walls_ag=float(params["area_walls_ag"]),
                area_walls_bg=float(params.get("area_walls_bg", 0)),
                area_windows=float(params["area_windows"]),
                area_roof=float(params["area_roof"]),
                window_groups=window_groups,
                u_walls_ag=float(params.get("u_walls_ag", 0.273)),
                u_windows=float(params.get("u_windows", 2.56)),
                u_roof=float(params.get("u_roof", 0.164)),
                cop_htg=float(params.get("cop_htg", 0.85)),
                cop_clg=float(params.get("cop_clg", 5.2)),
                cop_dhw=float(params.get("cop_dhw", 0.85)),
                hrv_efficiency=float(params.get("hrv_efficiency", 0.55)),
                silent=True,
            )
        except Exception as e:
            print(f"Simulation failed: {e}", file=sys.stderr)
            sys.exit(1)
        finally:
            os.chdir(original_cwd)

    # Extract results from the Run object
    gfa = float(run.gfa)
    monthly_heating = run.heating_demand.tolist()
    monthly_cooling = run.cooling_demand.tolist()
    monthly_htg_consumption = run.heating_consumption.tolist()
    monthly_clg_consumption = run.cooling_consumption.tolist()
    monthly_total = run.total_energy_consumption.tolist()
    monthly_ghg = run.total_ghg_emissions.tolist()

    tedi = sum(monthly_heating) / gfa if gfa > 0 else 0.0
    teui = sum(monthly_total) / gfa if gfa > 0 else 0.0
    ghgi = sum(monthly_ghg) / gfa if gfa > 0 else 0.0

    result = {
        "name": run.name,
        "monthly_heating_demand": monthly_heating,
        "monthly_cooling_demand": monthly_cooling,
        "monthly_heating_consumption": monthly_htg_consumption,
        "monthly_cooling_consumption": monthly_clg_consumption,
        "monthly_total_consumption": monthly_total,
        "monthly_ghg_emissions": monthly_ghg,
        "tedi_kwh_m2": round(tedi, 2),
        "teui_kwh_m2": round(teui, 2),
        "ghgi_kg_m2": round(ghgi, 2),
        "gfa": gfa,
        "weather_file": getattr(run, "weather_file", "unknown"),
    }

    # Write JSON to stdout
    json.dump(result, sys.stdout, indent=2)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
