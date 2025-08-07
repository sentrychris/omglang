"""
Tests for tail-call optimization in bytecode compiler.
"""
from omglang.bytecode import compile_source


def test_tail_call_emits_tcall():
    """
    Ensure tail recursive calls are emitted as TCALL in bytecode.
    """
    src = """
proc fact(n, acc) {
    if n <= 1 {
        return acc
    }
    return fact(n - 1, acc * n)
}
"""
    bc = compile_source(src)
    lines = bc.splitlines()
    assert "TCALL fact" in lines
    # ensure no regular call/ret sequence for tail recursive call
    assert "CALL fact" not in lines
