# -*- coding: utf-8 -*-
"""Start Makit Server"""

__title__ = 'Start\nServer'
__author__ = 'winterweken'

import sys
import os

# Add lib directory to path
lib_path = os.path.join(
    os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(__file__)))),
    'lib'
)
if lib_path not in sys.path:
    sys.path.insert(0, lib_path)

from makit_server import _server
from pyrevit import forms

if _server and _server.is_running():
    forms.alert('Makit Server is already running on http://localhost:8888', exitscript=True)
else:
    import threading
    from makit_server import MakitServer

    server = MakitServer(port=8888)
    server_thread = threading.Thread(target=server.start)
    server_thread.daemon = True
    server_thread.start()

    forms.alert('Makit Server started on http://localhost:8888\n\nYou can now use the makit CLI tool to extract geometry.', exitscript=True)
