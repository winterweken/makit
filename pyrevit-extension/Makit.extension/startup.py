# -*- coding: utf-8 -*-
"""Makit Extension Startup Script
Starts the HTTP server for external tool communication
"""

import sys
import os
import threading

# Add lib directory to path
lib_path = os.path.join(os.path.dirname(__file__), 'lib')
if lib_path not in sys.path:
    sys.path.insert(0, lib_path)

from makit_server import MakitServer

# Global server instance
_server = None

def start_server():
    """Start the Makit HTTP server"""
    global _server

    if _server is None or not _server.is_running():
        _server = MakitServer(port=8888)
        server_thread = threading.Thread(target=_server.start)
        server_thread.daemon = True
        server_thread.start()
        print("Makit Server started on http://localhost:8888")
    else:
        print("Makit Server is already running")

# Start server on extension load
start_server()
