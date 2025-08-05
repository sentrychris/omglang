import os
import sys
from pathlib import Path

import pytest

sys.path.append(os.path.dirname(os.path.dirname(__file__)))

from core.lexer import tokenize
from core.parser import Parser
from core.interpreter import Interpreter


def run_file(path: Path) -> Interpreter:
    code = path.read_text()
    interpreter = Interpreter(str(path))
    interpreter.check_header(code)
    tokens, token_map = tokenize(code)
    parser = Parser(tokens, token_map, str(path))
    ast = parser.parse()
    interpreter.execute(ast)
    return interpreter


def test_basic_import(tmp_path: Path):
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

