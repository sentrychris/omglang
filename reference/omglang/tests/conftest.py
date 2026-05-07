"""
Pytest configuration for OMG Language tests.
"""
from pathlib import Path
import sys


# Ensure the project root is on the Python path for all tests
PROJECT_ROOT = Path(__file__).resolve().parents[2]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
