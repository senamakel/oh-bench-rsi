"""Run the unmodified official harness with the OpenHuman backend registered.

Usage (inside the harness container, from the official checkout's root on
``sys.path``): ``python run_harness.py <official checkout> <harness.py arguments>``.

Importing ``oh_memory`` registers the ``oh_tinymemory`` memory type in the
official registry. The harness is imported as a module and its reader call is
wrapped with ``retry.retry_empty``, so an empty reply is asked again instead
of aborting the evaluation; nothing in the official checkout is edited.
"""

import pathlib
import sys

OFFICIAL = pathlib.Path(sys.argv.pop(1)).resolve()
sys.path.insert(0, str(OFFICIAL))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import oh_memory  # noqa: E402,F401  (registers the backend)
import retry  # noqa: E402
from evaluation import harness  # noqa: E402

harness.call_reader_model_async = retry.retry_empty(harness.call_reader_model_async)
sys.argv[0] = str(OFFICIAL / "evaluation" / "harness.py")
harness.main()
