# -*- coding: utf-8 -*-
"""Makit HTTP Server for Revit API access"""

import json
import threading
from BaseHTTPServer import HTTPServer, BaseHTTPRequestHandler

# Import Revit API (will be available in pyRevit context)
try:
    from Autodesk.Revit.DB import *
    from pyrevit import revit, HOST_APP
    REVIT_AVAILABLE = True
except ImportError:
    REVIT_AVAILABLE = False
    print("Warning: Revit API not available")


class MakitRequestHandler(BaseHTTPRequestHandler):
    """HTTP request handler for Makit API"""

    def _send_json(self, data, status=200):
        """Send JSON response"""
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.end_headers()
        self.wfile.write(json.dumps(data))

    def _send_error_json(self, message, status=500):
        """Send error response"""
        self._send_json({'error': message}, status)

    def _get_post_data(self):
        """Parse POST data as JSON"""
        content_length = int(self.headers.getheader('content-length', 0))
        post_data = self.rfile.read(content_length)
        try:
            return json.loads(post_data) if post_data else {}
        except ValueError:
            return {}

    def do_GET(self):
        """Handle GET requests"""
        if self.path == '/health':
            self._send_json({'status': 'ok', 'revit_available': REVIT_AVAILABLE})

        elif self.path == '/api/project/info':
            try:
                from geometry_extractor import get_project_info
                info = get_project_info()
                self._send_json(info)
            except Exception as e:
                self._send_error_json(str(e))

        else:
            self._send_error_json('Not found', 404)

    def do_POST(self):
        """Handle POST requests"""
        options = self._get_post_data()

        if self.path == '/api/geometry/walls':
            try:
                from geometry_extractor import extract_walls
                result = extract_walls(options)
                self._send_json(result)
            except Exception as e:
                self._send_error_json(str(e))

        elif self.path == '/api/geometry/floors':
            try:
                from geometry_extractor import extract_floors
                result = extract_floors(options)
                self._send_json(result)
            except Exception as e:
                self._send_error_json(str(e))

        elif self.path == '/api/geometry/rooms':
            try:
                from geometry_extractor import extract_rooms
                result = extract_rooms(options)
                self._send_json(result)
            except Exception as e:
                self._send_error_json(str(e))

        elif self.path == '/api/analysis/wall-orientations':
            try:
                from revit_extractors import analyze_walls_direct_from_revit
                result = analyze_walls_direct_from_revit(options)
                self._send_json(result)
            except Exception as e:
                self._send_error_json(str(e))

        elif self.path == '/api/extraction/building-model':
            try:
                from revit_extractors import extract_building_model_from_revit
                building_model = extract_building_model_from_revit(options)
                self._send_json(building_model.to_dict())
            except Exception as e:
                self._send_error_json(str(e))

        elif self.path == '/api/analysis/generic':
            try:
                from analysis_engine import analyze_wall_orientations, generate_orientation_report
                # Expects pre-extracted building model in options
                stats = analyze_wall_orientations(options.get('buildingModel', {}))
                report = generate_orientation_report(stats)
                self._send_json({
                    'stats': stats,
                    'report': report
                })
            except Exception as e:
                self._send_error_json(str(e))

        else:
            self._send_error_json('Not found', 404)

    def log_message(self, format, *args):
        """Override to customize logging"""
        print("[Makit Server] %s" % (format % args))


class MakitServer:
    """Makit HTTP Server wrapper"""

    def __init__(self, host='localhost', port=8888):
        self.host = host
        self.port = port
        self.server = None
        self._running = False

    def start(self):
        """Start the HTTP server"""
        try:
            self.server = HTTPServer((self.host, self.port), MakitRequestHandler)
            self._running = True
            print("Makit Server listening on http://%s:%d" % (self.host, self.port))
            self.server.serve_forever()
        except Exception as e:
            print("Failed to start Makit Server: %s" % str(e))
            self._running = False

    def stop(self):
        """Stop the HTTP server"""
        if self.server:
            self.server.shutdown()
            self._running = False

    def is_running(self):
        """Check if server is running"""
        return self._running
