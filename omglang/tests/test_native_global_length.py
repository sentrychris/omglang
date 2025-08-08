"""
# Test for the global length built-in function in OMG language
"""
import subprocess
from pathlib import Path

from omglang.compiler import compile_source


def find_project_root(marker: str = "omg.py") -> Path:
    """Locate project root by ascending directories until marker file is found."""
    path = Path(__file__).resolve()
    for parent in path.parents:
        if (parent / marker).exists():
            return parent
    raise RuntimeError("Could not find project root")


def test_builtin_length_sees_global_list(tmp_path):
    """Functions should access global variables when calling built-ins."""
    source = (
        "alloc arr := [1, 2, 3]\n"
        "proc show() { emit length(arr) }\n"
        "show()\n"
    )
    bc = compile_source(source, "<test>")
    bc_file = tmp_path / "prog.omgb"
    bc_file.write_bytes(bc.encode("utf-8"))

    root = find_project_root()
    result = subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(root / "runtime" / "Cargo.toml"),
            str(bc_file),
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    assert result.stdout.strip().splitlines()[-1] == "3"
