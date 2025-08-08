"""
Parse step binary
"""

import os
import struct
import sys

from omglang.compiler import REV_OPCODES

def verify_interpreter(interp_bin):
    """
    Verify the interpreter binary contains full opcode instructions
    """
    with open(interp_bin, 'rb') as f:
        data=f.read()
    print('len', len(data))

    idx=4
    func_count=struct.unpack_from('<I',data,idx)[0]
    idx+=4
    print('func_count', func_count)

    for i in range(func_count):
        name_len = struct.unpack_from('<I',data,idx)[0]
        idx += 4
        name = data[idx:idx+name_len]
        idx += name_len
        param_count = struct.unpack_from('<I',data,idx)[0]
        idx += 4
        for _ in range(param_count):
            p_len = struct.unpack_from('<I',data,idx)[0]
            idx += 4
            idx += p_len
        addr = struct.unpack_from('<I',data,idx)[0]
        idx += 4
        print('func', i, name, param_count, addr)

    code_len = struct.unpack_from('<I',data,idx)[0]
    idx += 4
    print('code_len', code_len)
    for ins in range(code_len):
        if idx >= len(data):
            print('unexpected end at instruction', ins, 'idx', idx)
            break
        op = data[idx]
        idx += 1
        name = REV_OPCODES.get(op, f'UNKNOWN_{op}')
        if name == 'UNKNOWN_'+str(op):
            print('unknown opcode', op, 'at', ins)
        if name == 'PUSH_INT':
            if idx+8>len(data):
                print('int beyond')
                break
            idx += 8
        elif name == 'PUSH_STR':
            if idx+4 > len(data):
                print('str len beyond')
                break
            slen = struct.unpack_from('<I', data, idx)[0]
            idx += 4
            if idx+slen>len(data):
                print('str beyond at', ins, 'slen', slen, 'idx', idx)
                break
            idx += slen
        elif name == 'PUSH_BOOL':
            idx += 1
        elif name in {'BUILD_LIST', 'BUILD_DICT', 'CALL_VALUE', 'JUMP', 'JUMP_IF_FALSE', 'BUILTIN'}:
            if idx+4 > len(data):
                print('arg beyond at', ins)
                break
            if name == 'BUILTIN':
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
            if idx+4 > len(data):
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
