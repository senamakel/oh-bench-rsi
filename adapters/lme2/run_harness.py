"""Run the unmodified official harness with the OpenHuman backend registered.

Usage (inside the harness container, from the official checkout's root on
``sys.path``): ``python run_harness.py <harness.py arguments>``.

Importing ``oh_memory`` registers the ``oh_tinymemory`` memory type in the
official registry; nothing in the official checkout is edited.
"""

import pathlib
import runpy
import sys

OFFICIAL = pathlib.Path(sys.argv.pop(1)).resolve()
sys.path.insert(0, str(OFFICIAL))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import oh_memory  # noqa: E402,F401  (registers the backend)

sys.argv[0] = str(OFFICIAL / "evaluation" / "harness.py")
runpy.run_path(sys.argv[0], run_name="__main__")
