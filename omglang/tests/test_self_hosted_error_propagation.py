"""Tests for error reporting in the self-hosted interpreter."""

from pathlib import Path
import subprocess


def find_project_root(marker: str = "omg.py") -> Path:
    """Locate repository root by looking for marker file."""
    path = Path(__file__).resolve()
    for parent in path.parents:
        if (parent / marker).exists():
            return parent
    raise RuntimeError("Could not find project root")


def test_undefined_variable_error(tmp_path):
    """Running a script with an undefined variable should surface a clear error."""
    root = find_project_root()
    script = tmp_path / "bad.omg"
    script.write_text(";;;omg\nemit m\n")
    result = subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(root / "runtime" / "Cargo.toml"),
            str(script),
        ],
        capture_output=True,
        text=True,
    )
    # Error may be reported on stdout or stderr depending on runtime wiring.
    output = result.stdout + result.stderr
    assert "Undefined variable: m" in output
