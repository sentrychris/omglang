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
    // --- Data / conversion -------------------------------------------------
    {
        name: 'length',
        signature: 'length(x)',
        detail: 'Number of elements in a list, or characters in a string.'
    },
    {
        name: 'chr',
        signature: 'chr(n)',
        detail: 'Single-character string for byte value `n` (low 8 bits).'
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
            'namespaces). Idempotent on an already-frozen dict.'
    },
    {
        name: 'string_bytes',
        signature: 'string_bytes(s)',
        detail:
            'List of UTF-8 byte values (0–255) for `s`. Inverse of ' +
            '`bytes_to_string`.'
    },
    {
        name: 'bytes_to_string',
        signature: 'bytes_to_string(bs)',
        detail:
            'UTF-8 string from a list of byte ints (0–255). Inverse of ' +
            '`string_bytes`. Raises ValueError if the bytes are not ' +
            'valid UTF-8.'
    },
    {
        name: 'dict_keys',
        signature: 'dict_keys(d)',
        detail:
            'List of keys from a dict (or frozen dict). Order is ' +
            'unspecified; sort if you need determinism.'
    },
    {
        name: 'list_repeat',
        signature: 'list_repeat(item, count)',
        detail:
            'Fresh list of length `count` with every slot holding ' +
            '`item`. Use for amortised-O(1) buffer growth instead of ' +
            'repeated `xs + [v]` (which is O(n²)).'
    },

    // --- Errors ------------------------------------------------------------
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

    // --- Process / IO ------------------------------------------------------
    {
        name: 'print',
        signature: 'print(s)',
        detail:
            'Write `s` to stdout with no trailing newline (use `emit` ' +
            'for newline-terminated output). Flushes immediately.'
    },
    {
        name: 'exit',
        signature: 'exit(code)',
        detail: 'Terminate the process with integer exit `code`.'
    },
    {
        name: 'exit_with_error',
        signature: 'exit_with_error(msg)',
        detail:
            'Print `msg` to stderr verbatim (no kind prefix) and exit ' +
            '1. Bypasses `try` / `except`.'
    },
    {
        name: 'getpid',
        signature: 'getpid()',
        detail:
            'Current process ID. Useful for unique tempfile paths.'
    },
    {
        name: 'subprocess',
        signature: 'subprocess(argv)',
        detail:
            'Run `argv` (list of strings) as a child process; stdio is ' +
            'inherited. Returns the exit code (or 128+signal if killed). ' +
            'Raises ValueError if exec fails.'
    },
    {
        name: 'stdin_readline',
        signature: 'stdin_readline()',
        detail:
            'Read one line from stdin without the trailing newline. ' +
            'Returns `false` on EOF.'
    },
    {
        name: 'stdin_read',
        signature: 'stdin_read()',
        detail:
            'Slurp all of stdin to EOF as a UTF-8 string. Empty string ' +
            'if stdin is already at EOF.'
    },
    {
        name: 'stdin_read_bytes',
        signature: 'stdin_read_bytes()',
        detail:
            'Slurp all of stdin to EOF as a list of byte ints (0–255).'
    },

    // --- Filesystem --------------------------------------------------------
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
        name: 'is_dir',
        signature: 'is_dir(path)',
        detail: 'Boolean: is `path` a directory?'
    },
    {
        name: 'read_dir',
        signature: 'read_dir(path)',
        detail:
            'List of entry names in `path` (no `.` / `..`). Sorted ' +
            'lexicographically.'
    },
    {
        name: 'make_dir',
        signature: 'make_dir(path)',
        detail:
            'Create `path` and any intermediate directories (`mkdir -p` ' +
            'semantics). Returns `true` on success or if it already ' +
            'exists; raises ValueError on real failures.'
    },

    // --- File handles ------------------------------------------------------
    {
        name: 'file_open',
        signature: 'file_open(path, mode)',
        detail:
            'Open a file. `mode` is one of `r`, `rb`, `w`, `wb`, `a`, ' +
            '`ab`, `rb+`, `wb+`. The `+` modes are binary read+write ' +
            '(use with `file_seek` / `file_tell`). Returns an integer ' +
            'handle.'
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
        name: 'file_seek',
        signature: 'file_seek(handle, offset)',
        detail:
            'Seek to absolute byte `offset` in a binary file. Returns ' +
            'the new position. Pair with `file_tell` for random access.'
    },
    {
        name: 'file_tell',
        signature: 'file_tell(handle)',
        detail: 'Current absolute byte position in an open file.'
    },
    {
        name: 'file_close',
        signature: 'file_close(handle)',
        detail: 'Close an opened file handle.'
    },

    // --- TCP networking ----------------------------------------------------
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

    // --- Process control ---------------------------------------------------
    {
        name: 'fork',
        signature: 'fork()',
        detail:
            'POSIX fork. Returns 0 in the child process and the new ' +
            'child PID in the parent. Children auto-reap (SIGCHLD set ' +
            'to SIG_IGN), so no wait() is required.'
    },

    // --- Numeric / math ----------------------------------------------------
    {
        name: 'int',
        signature: 'int(x)',
        detail:
            'Convert to int. Floats truncate toward zero; strings parse ' +
            'as int.'
    },
    {
        name: 'float',
        signature: 'float(x)',
        detail:
            'Convert to float. Ints widen; strings parse as float.'
    },
    {
        name: 'floor',
        signature: 'floor(x)',
        detail: 'Largest integer ≤ x.'
    },
    {
        name: 'ceil',
        signature: 'ceil(x)',
        detail: 'Smallest integer ≥ x.'
    },
    {
        name: 'round',
        signature: 'round(x)',
        detail: "Round to nearest integer using banker's rounding (ties to even)."
    },
    {
        name: 'abs',
        signature: 'abs(x)',
        detail: 'Absolute value of `x`. Returns the same type (int or float).'
    },
    {
        name: 'sqrt',
        signature: 'sqrt(x)',
        detail: 'Square root of `x` as a float. Raises ValueError if `x` < 0.'
    },
    {
        name: 'pow',
        signature: 'pow(a, b)',
        detail:
            'Raise `a` to the power `b`. int**non_negative_int stays ' +
            'int (overflow-checked); otherwise widens to float.'
    },
    {
        name: 'log',
        signature: 'log(x)',
        detail: 'Natural logarithm of `x`. Raises ValueError if `x` ≤ 0.'
    },
    {
        name: 'sin',
        signature: 'sin(x)',
        detail: 'Sine of `x` (radians) as a float.'
    },
    {
        name: 'cos',
        signature: 'cos(x)',
        detail: 'Cosine of `x` (radians) as a float.'
    },
    {
        name: 'tan',
        signature: 'tan(x)',
        detail: 'Tangent of `x` (radians) as a float.'
    },
    {
        name: 'bits_to_float',
        signature: 'bits_to_float(i)',
        detail:
            'Reinterpret an i64 as the IEEE-754 bit pattern of an f64. ' +
            'Inverse of `float_bits`.'
    },
    {
        name: 'float_bits',
        signature: 'float_bits(s)',
        detail:
            'Parse numeric string `s` and return the i64 of its IEEE-754 ' +
            'bit pattern. Inverse of `bits_to_float`.'
    },

    // --- Meta --------------------------------------------------------------
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
