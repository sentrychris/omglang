"""
Parse step binary
"""

import os
import struct
import sys

from omglang.compiler import REV_OPCODES

INTERP_BINARY = os.path.join(
    os.path.dirname(os.path.dirname(__file__)),
    'runtime',
    'interpreter.omgb'
) if len(sys.argv) == 1 else sys.argv[1]

with open(INTERP_BINARY, 'rb') as f:
    data=f.read()
print('len', len(data))

IDX=4
func_count=struct.unpack_from('<I',data,IDX)[0]
IDX+=4
print('func_count', func_count)

for i in range(func_count):
    name_len = struct.unpack_from('<I',data,IDX)[0]
    IDX += 4
    name = data[IDX:IDX+name_len]
    IDX += name_len
    param_count = struct.unpack_from('<I',data,IDX)[0]
    IDX += 4
    for _ in range(param_count):
        p_len = struct.unpack_from('<I',data,IDX)[0]
        IDX += 4
        IDX += p_len
    addr = struct.unpack_from('<I',data,IDX)[0]
    IDX += 4
    print('func', i, name, param_count, addr)

code_len = struct.unpack_from('<I',data,IDX)[0]
IDX += 4
print('code_len', code_len)
for ins in range(code_len):
    if IDX >= len(data):
        print('unexpected end at instruction', ins, 'IDX', IDX)
        break
    op = data[IDX]
    IDX += 1
    name = REV_OPCODES.get(op, f'UNKNOWN_{op}')
    if name == 'UNKNOWN_'+str(op):
        print('unknown opcode', op, 'at', ins)
    if name == 'PUSH_INT':
        if IDX+8>len(data):
            print('int beyond')
            break
        IDX += 8
    elif name == 'PUSH_STR':
        if IDX+4 > len(data):
            print('str len beyond')
            break
        slen = struct.unpack_from('<I', data, IDX)[0]
        IDX += 4
        if IDX+slen>len(data):
            print('str beyond at', ins, 'slen', slen, 'IDX', IDX)
            break
        IDX += slen
    elif name == 'PUSH_BOOL':
        IDX += 1
    elif name in {'BUILD_LIST', 'BUILD_DICT', 'CALL_VALUE', 'JUMP', 'JUMP_IF_FALSE', 'BUILTIN'}:
        if IDX+4 > len(data):
            print('arg beyond at', ins)
            break
        if name == 'BUILTIN':
            slen = struct.unpack_from('<I', data, IDX)[0]
            IDX += 4
            if IDX+slen > len(data):
                print('builtin name beyond')
                break
            IDX += slen
            if IDX+4 > len(data):
                print('builtin argc beyond')
                break
            IDX += 4
        else:
            IDX += 4
    elif name in {'LOAD', 'STORE', 'CALL', 'TCALL', 'ATTR', 'STORE_ATTR'}:
        if IDX+4 > len(data):
            print('string arg len beyond')
            break
        slen = struct.unpack_from('<I', data, IDX)[0]
        IDX += 4
        if IDX+slen > len(data):
            print('string arg beyond at', ins, 'slen', slen, 'IDX', IDX)
            break
        IDX += slen
    else:
        # no args
        pass
else:
    print('finished all instructions, IDX', IDX, 'len', len(data))
