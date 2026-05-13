// Static reference tables for OMG keywords and built-in functions.
//
// Keep these in lockstep with the runtime: `runtime/src/lexer.rs`'s keyword
// list and `runtime/src/vm/builtins.rs`'s `call_builtin` dispatch.

export const KEYWORDS: readonly string[] = [
    'alloc',
    'and',
    'as',
    'break',
    'elif',
    'else',
    'emit',
    'except',
    'facts',
    'false',
    'if',
    'import',
    'loop',
    'or',
    'proc',
    'return',
    'true',
    'try'
];

export type BuiltinDoc = {
    name: string;
    signature: string;
    detail: string;
};

export const BUILTINS: readonly BuiltinDoc[] = [
    {
        name: 'length',
        signature: 'length(x)',
        detail: 'Number of elements in a list, or characters in a string.'
    },
    {
        name: 'chr',
        signature: 'chr(n)',
        detail: 'Single-character string for byte value `n`.'
    },
    {
        name: 'ascii',
        signature: 'ascii(c)',
        detail: 'Codepoint of single-character string `c`.'
    },
    {
        name: 'hex',
        signature: 'hex(n)',
        detail: 'Lower-case hexadecimal string for integer `n`.'
    },
    {
        name: 'binary',
        signature: 'binary(n[, width])',
        detail:
            'Binary string for integer `n`. With `width`, masks to that ' +
            'many bits and zero-pads.'
    },
    {
        name: 'freeze',
        signature: 'freeze(d)',
        detail:
            'Convert a dict to an immutable frozen dict (used for module ' +
            'namespaces).'
    },
    {
        name: 'panic',
        signature: 'panic(msg)',
        detail: 'Raise a generic runtime error with the given message.'
    },
    {
        name: 'raise',
        signature: 'raise(msg)',
        detail:
            'Raise a runtime error catchable by the nearest enclosing ' +
            '`try` / `except`.'
    },
    {
        name: 'read_file',
        signature: 'read_file(path)',
        detail:
            'Read a UTF-8 file relative to `current_dir`. Returns the ' +
            'contents on success or `false` on I/O error.'
    },
    {
        name: 'file_exists',
        signature: 'file_exists(path)',
        detail: 'Boolean: does the file at `path` exist?'
    },
    {
        name: 'file_open',
        signature: 'file_open(path, mode)',
        detail:
            'Open a file. `mode` is one of `r`, `rb`, `w`, `wb`, `a`, ' +
            '`ab`. Returns an integer handle.'
    },
    {
        name: 'file_read',
        signature: 'file_read(handle)',
        detail:
            'Read the full contents of an opened file. Text mode → ' +
            '`string`; binary mode → `list` of bytes.'
    },
    {
        name: 'file_write',
        signature: 'file_write(handle, data)',
        detail:
            'Write to an opened file. Text mode expects a string; binary ' +
            'mode expects a list of byte ints (0–255). Returns bytes ' +
            'written.'
    },
    {
        name: 'file_close',
        signature: 'file_close(handle)',
        detail: 'Close an opened file handle.'
    },
    {
        name: 'tcp_listen',
        signature: 'tcp_listen(host, port)',
        detail:
            'Bind + listen on `host:port` (e.g. `"127.0.0.1"`, `8080`). ' +
            'Returns a listener handle.'
    },
    {
        name: 'tcp_accept',
        signature: 'tcp_accept(handle)',
        detail:
            'Block until a peer connects to a listener. Returns a new ' +
            'stream handle for that connection.'
    },
    {
        name: 'tcp_connect',
        signature: 'tcp_connect(host, port)',
        detail:
            'Open an outbound TCP connection to `host:port`. Returns a ' +
            'stream handle.'
    },
    {
        name: 'tcp_read',
        signature: 'tcp_read(handle, max_bytes)',
        detail:
            'Read up to `max_bytes` from a stream handle. Returns a list ' +
            'of byte ints (0–255); empty list means EOF.'
    },
    {
        name: 'tcp_write',
        signature: 'tcp_write(handle, data)',
        detail:
            'Write to a stream handle. `data` is a string or a list of ' +
            'byte ints (0–255). Returns bytes written.'
    },
    {
        name: 'tcp_close',
        signature: 'tcp_close(handle)',
        detail: 'Close a listener or stream handle.'
    },
    {
        name: 'fork',
        signature: 'fork()',
        detail:
            'POSIX fork. Returns 0 in the child process and the new ' +
            'child PID in the parent. Children auto-reap (SIGCHLD set ' +
            'to SIG_IGN), so no wait() is required.'
    },
    {
        name: 'call_builtin',
        signature: 'call_builtin(name, args)',
        detail: 'Reflectively dispatch to another built-in by name.'
    }
];

export const BUILTIN_NAMES: ReadonlySet<string> = new Set(
    BUILTINS.map((b) => b.name)
);

/**
 * Reserved globals that the runtime injects into every program. Useful for
 * completion + hover so users see them as known names.
 */
export const RESERVED_GLOBALS: readonly { name: string; detail: string }[] = [
    {
        name: 'args',
        detail:
            'Program arguments as a list of strings. `args[0]` is the ' +
            'script path itself.'
    },
    {
        name: 'module_file',
        detail: 'String: the path to the running script.'
    },
    {
        name: 'current_dir',
        detail:
            'String: the directory used to resolve relative paths in ' +
            'built-ins like `read_file`.'
    }
];
