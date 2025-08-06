import os
import subprocess
import sys
from pathlib import Path

def find_project_root(marker: str = "omg.py") -> Path:
    """Walk up the directory tree to find the project root (contains omg.py)."""
    path = Path(__file__).resolve()
    for parent in path.parents:
        if (parent / marker).exists():
            return parent
    raise RuntimeError("Could not find project root")


def test_ast_interpreter_parses_source():
    """Ensure the OMG-based AST interpreter can parse and execute source code."""
    root = find_project_root()
    script = root / 'examples' / 'self-hosting' / 'interpreter.omg'
    omg_runner = root / 'omg.py'

    result = subprocess.run(
        [sys.executable, str(omg_runner), str(script)],
        capture_output=True,
        text=True,
        check=True
    )

    lines = result.stdout.strip().splitlines()
    assert lines[-1] == '120'
