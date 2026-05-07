"""
Tests for module imports in OMG Language
"""
from pathlib import Path

import pytest

from omglang.tests.utils import run_file


def test_basic_import(tmp_path: Path):
    """
    Test that a basic import works correctly.
    """
    utils_source = (
        ";;;omg\n"
        "alloc MAGIC := 40\n"
        "MAGIC := MAGIC + 2\n"
        "proc add(a, b) { return a + b }\n"
    )
    utils_file = tmp_path / "utils.omg"
    utils_file.write_text(utils_source)

    main_source = (
        ";;;omg\n"
        f"import \"{utils_file.name}\" as utils\n"
        "facts utils.add(1,2) == 3\n"
        "facts utils.MAGIC == 42\n"
    )
    main_file = tmp_path / "main.omg"
    main_file.write_text(main_source)

    run_file(main_file)


def test_import_read_only(tmp_path: Path):
    """
    Test that imported modules are read-only and cannot be modified.
    """
    mod_source = ";;;omg\nalloc X := 1\n"
    mod_file = tmp_path / "mod.omg"
    mod_file.write_text(mod_source)

    main_source = (
        ";;;omg\n"
        f"import \"{mod_file.name}\" as m\n"
        "m.X := 2\n"
    )
    main_file = tmp_path / "main.omg"
    main_file.write_text(main_source)

    with pytest.raises(TypeError):
        run_file(main_file)


def test_circular_import(tmp_path: Path):
    """
    Test that circular imports raise a RuntimeError.
    """
    a_file = tmp_path / "a.omg"
    b_file = tmp_path / "b.omg"

    a_file.write_text(
        ";;;omg\nimport \"b.omg\" as b\nalloc A := 1\n"
    )
    b_file.write_text(
        ";;;omg\nimport \"a.omg\" as a\nalloc B := 2\n"
    )

    main_source = ";;;omg\nimport \"a.omg\" as a\n"
    main_file = tmp_path / "main.omg"
    main_file.write_text(main_source)

    with pytest.raises(RuntimeError):
        run_file(main_file)


def test_recursive_import_function(tmp_path: Path):
    """
    Functions from imported modules should support recursion and be callable inside other functions.
    """
    math_source = (
        ";;;omg\n"
        "proc factorial(n) {\n"
        "    if n <= 1 {\n"
        "        return 1\n"
        "    } else {\n"
        "        return n * factorial(n - 1)\n"
        "    }\n"
        "}\n"
    )
    math_file = tmp_path / "math.omg"
    math_file.write_text(math_source)

    main_source = (
        ";;;omg\n"
        f"import \"{math_file.name}\" as math\n"
        "facts math.factorial(5) == 120\n"
        "proc apply(n) { return math.factorial(n) }\n"
        "facts apply(3) == 6\n"
    )
    main_file = tmp_path / "main.omg"
    main_file.write_text(main_source)

    run_file(main_file)
