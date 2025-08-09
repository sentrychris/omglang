"""
Parse step binary.

Sanity check a compiled OMG interpreter binary by walking the header, the function table,
then the bytecode stream, advancing an index accroding to each opcode’s operand format.
Any overrun or unknown opcode is reported.

Verifies:
- Header layout: 4 bytes magic, then function table, then a <I `code_len`, then `code_len`
  opcodes with well-formed operands.
- All operand lengths are within bounds.
- All opcodes are known (warns on unknown).

Assumes:
- Function names/param names are UTF-8 and sensible.
- `addr` values actually point inside the code section.
- `code_len` is the instruction count, not byte size.
- `PUSH_INT` is always 8 bytes, little-endian.
- No alignment/padding between records.
"""

import os
import struct
import sys

from omglang.compiler import REV_OPCODES, MAGIC_HEADER

def verify_interpreter(interp_bin):
    """
    Verify the interpreter binary contains full opcode instructions
    """
    # Read interpreter.omgb as bytes
    with open(interp_bin, 'rb') as f:
        data=f.read()
    print('len', len(data))

    # Verify magic header
    magic_header = data[:4]
    if magic_header != MAGIC_HEADER:
        raise ValueError(f"Bad magic: {data[:4]!r}")
    print(f"4-byte header {data[:4]!r} is valid")

    idx = 4 # advance 4-bytes

    # Read the function table
    func_count=struct.unpack_from('<I', data, idx)[0]
    idx += 4
    print('func_count', func_count)

    for i in range(func_count):
        # Func name is stored as length-prefixed bytes
        name_len = struct.unpack_from('<I', data,idx)[0]
        idx += 4
        name = data[idx:idx+name_len]
        idx += name_len
        # params are stored as length-prefixed byte strings
        param_count = struct.unpack_from('<I', data,idx)[0]
        idx += 4
        for _ in range(param_count):
            p_len = struct.unpack_from('<I', data, idx)[0]
            idx += 4
            idx += p_len
        # entry points are not validated yet
        addr = struct.unpack_from('<I', data, idx)[0]
        idx += 4
        print('func', i, name, param_count, addr)

    # Code section header
    code_len = struct.unpack_from('<I', data, idx)[0]
    idx += 4
    print('code_len', code_len)
    # Interpreted as the number of instructions (not bytes). The loop
    # runs the length of the code unless a bounds error occurs.
    for ins in range(code_len):
        if idx >= len(data):
            print('unexpected end at instruction', ins, 'idx', idx)
            break
        op = data[idx] # read 1-byte opcode
        idx += 1
        name = REV_OPCODES.get(op, f'UNKNOWN_{op}') # map opcode to mnemonic
        if name == 'UNKNOWN_'+str(op): # if unknown then warn
            print('unknown opcode', op, 'at', ins)
        # advance instruction index 
        if name == 'PUSH_INT':
            if idx+8>len(data): # 64-bit integer...
                print('int beyond')
                break
            idx += 8            #...so advance 8 bytes
        elif name == 'PUSH_STR':
            if idx+4 > len(data):
                print('str len beyond')
                break
            slen = struct.unpack_from('<I', data, idx)[0]
            idx += 4
            if idx+slen>len(data): # str, read <I length...
                print('str beyond at', ins, 'slen', slen, 'idx', idx)
                break
            idx += slen            # ...advance <I length bytes
        elif name == 'PUSH_BOOL':
            idx += 1                # bool... advance 1 byte
        elif name in {'BUILD_LIST', 'BUILD_DICT', 'CALL_VALUE', 'JUMP', 'JUMP_IF_FALSE', 'BUILTIN'}:
            if idx+4 > len(data): # one-u32 immediates (exc. BUILTIN)
                print('arg beyond at', ins)
                break
            if name == 'BUILTIN': # <i length + bytes + <I argc, two-u32s + blob
                slen = struct.unpack_from('<I', data, idx)[0]
                idx += 4
                if idx+slen > len(data):
                    print('builtin name beyond')
                    break
                idx += slen
                if idx+4 > len(data):
                    print('builtin argc beyond')
                    break
                idx += 4
            else:
                idx += 4
        elif name in {'LOAD', 'STORE', 'CALL', 'TCALL', 'ATTR', 'STORE_ATTR'}:
            if idx+4 > len(data): # <i length + bytes
                print('string arg len beyond')
                break
            slen = struct.unpack_from('<I', data, idx)[0]
            idx += 4
            if idx+slen > len(data):
                print('string arg beyond at', ins, 'slen', slen, 'idx', idx)
                break
            idx += slen
        else:
            # no args
            pass
    else:
        print('finished all instructions, idx', idx, 'len', len(data))

if __name__ == "__main__":
    verify_interpreter(os.path.join(
        os.path.dirname(os.path.dirname(__file__)),
        'runtime',
        'interpreter.omgb'
    ) if len(sys.argv) == 1 else sys.argv[1]
)
