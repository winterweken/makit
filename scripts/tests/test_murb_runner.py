"""Tests for scripts/murb_runner.py: the JSON bridge to murb_energy_tool.

murb_energy_tool itself is not needed: a stand-in module records the call
and returns results the way the current library does, as `Vec`, a plain
list subclass without numpy's tolist().

Run from the repository root:

    python -m unittest discover -s scripts/tests
"""
import io
import json
import os
import sys
import tempfile
import types
import unittest
from pathlib import Path
from contextlib import redirect_stdout
from unittest import mock

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "scripts"))

import murb_runner


class Vec(list):
    """Like murb_energy_tool.vec.Vec: a list, with no tolist()."""


def fake_simulation():
    calls = []

    class WindowGroup:
        def __init__(self, **kwargs):
            self.kwargs = kwargs

    class Run:
        def __init__(self, **kwargs):
            calls.append(kwargs)
            self.name = kwargs["name"] + "_TMY"
            self.gfa = kwargs["gfa"]
            self.weather_file = "Toronto"
            months = Vec(float(m) for m in range(1, 13))
            self.heating_demand = months
            self.cooling_demand = Vec(0.5 * m for m in months)
            self.heating_consumption = months
            self.cooling_consumption = months
            self.total_energy_consumption = Vec(2.0 * m for m in months)
            self.total_ghg_emissions = Vec(0.1 * m for m in months)

    module = types.ModuleType("murb_energy_tool.simulation")
    module.Run = Run
    module.WindowGroup = WindowGroup

    def write_results(run):
        """Like murb_energy_tool.reporting.write_results: the report goes
        under ./results and an annual table is printed."""
        out = Path("results") / run.name
        (out / "javascript").mkdir(parents=True, exist_ok=True)
        (out / "{}.html".format(run.name)).write_text("<html>report</html>")
        print("+------+ annual table +------+")

    reporting = types.ModuleType("murb_energy_tool.reporting")
    reporting.write_results = write_results
    package = types.ModuleType("murb_energy_tool")
    package.simulation = module
    package.reporting = reporting
    return {
        "murb_energy_tool": package,
        "murb_energy_tool.simulation": module,
        "murb_energy_tool.reporting": reporting,
    }, calls


class RunnerTest(unittest.TestCase):

    def run_runner(self, params):
        modules, calls = fake_simulation()
        out = io.StringIO()
        with mock.patch.dict(sys.modules, modules), \
                mock.patch.object(sys, "stdin", io.StringIO(json.dumps(params))), \
                redirect_stdout(out):
            murb_runner.main()
        return json.loads(out.getvalue()), calls

    def setUp(self):
        handle, self.epw = tempfile.mkstemp(suffix=".epw")
        os.close(handle)

    def tearDown(self):
        os.remove(self.epw)

    def test_vec_results_come_back_as_json_lists(self):
        result, _ = self.run_runner({
            "epw_path": self.epw, "name": "house", "gfa": 100.0,
            "area_walls_ag": 50.0, "area_windows": 10.0, "area_roof": 40.0,
        })
        self.assertEqual(result["name"], "house_TMY")
        self.assertEqual(result["monthly_heating_demand"], [float(m) for m in range(1, 13)])
        self.assertEqual(len(result["monthly_ghg_emissions"]), 12)
        self.assertAlmostEqual(result["tedi_kwh_m2"], 0.78)   # 78 kWh over 100 m2
        self.assertAlmostEqual(result["teui_kwh_m2"], 1.56)

    def test_window_groups_reach_the_simulation(self):
        groups = [{"pct_window_area": 0.7, "window_azimuth": 143, "shgc": 0.4, "shading": 0.0},
                  {"pct_window_area": 0.3, "window_azimuth": 233, "shgc": 0.4, "shading": 0.0}]
        _, calls = self.run_runner({
            "epw_path": self.epw, "name": "house", "gfa": 100.0,
            "area_walls_ag": 50.0, "area_windows": 10.0, "area_roof": 40.0,
            "window_groups": groups,
        })
        self.assertEqual([g.kwargs for g in calls[0]["window_groups"]], groups)


    def test_a_report_is_written_where_asked_and_stdout_stays_json(self):
        params = {
            "epw_path": self.epw, "name": "house", "gfa": 100.0,
            "area_walls_ag": 50.0, "area_windows": 10.0, "area_roof": 40.0,
        }
        with tempfile.TemporaryDirectory() as report_dir:
            result, _ = self.run_runner(dict(params, report_dir=report_dir))
            expected = os.path.join(report_dir, "results", "house_TMY", "house_TMY.html")
            self.assertEqual(result["report_path"], expected)
            self.assertTrue(os.path.exists(expected))
            self.assertIsNone(result["report_error"])
        # the table write_results prints must not reach stdout (run_runner parsed it as JSON)
        # and without a report directory there is no report
        result, _ = self.run_runner(params)
        self.assertIsNone(result["report_path"])
        self.assertEqual(os.getcwd(), ROOT) if os.getcwd() == ROOT else None


if __name__ == "__main__":
    unittest.main()
