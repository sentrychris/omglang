import pytest
from omglang.compiler import compile_source, disassemble

@pytest.mark.parametrize(
    "call, kind",
    [
        ("panic(\"boom\")", "Generic"),
        ("raise(\"boom\")", "Generic"),
        ("_omg_vm_syntax_error_handle(\"boom\")", "Syntax"),
        ("_omg_vm_type_error_handle(\"boom\")", "Type"),
        ("_omg_vm_undef_ident_error_handle(\"boom\")", "UndefinedIdent"),
        ("_omg_vm_value_error_handle(\"boom\")", "Value"),
        ("_omg_vm_module_import_error_handle(\"boom\")", "ModuleImport"),
    ],
)
def test_raise_compiles_to_raise_kind(call: str, kind: str) -> None:
    bc = compile_source(call)
    lines = disassemble(bc).splitlines()
    assert "PUSH_STR boom" in lines
    assert f"RAISE {kind}" in lines
    assert lines.index("PUSH_STR boom") < lines.index(f"RAISE {kind}")
