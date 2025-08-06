import os
import subprocess
import sys


def test_ast_interpreter_parses_source():
    """Ensure the OMG-based AST interpreter can parse and execute source code."""
    root = os.path.dirname(os.path.dirname(__file__))
    script = os.path.join(root, 'examples', 'ast_interpreter.omg')
    result = subprocess.run([sys.executable, os.path.join(root, 'omg.py'), script], capture_output=True, text=True, check=True)
    lines = result.stdout.strip().splitlines()
    assert lines[-1] == '120'
