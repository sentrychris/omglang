"""Test native VM list concatenation and length builtin."""

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


def test_native_handles_list_concat(tmp_path):
    """Ensure the native VM concatenates lists instead of summing lengths."""
    source = (
        "alloc stack := []\n"
        "stack := stack + [1]\n"
        "emit length(stack)\n"
    )
    bc = compile_source(source, "<test>")
    bc_file = tmp_path / "prog.bc"
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
    assert result.stdout.strip().splitlines()[-1] == "1"
