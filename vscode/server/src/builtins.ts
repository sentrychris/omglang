// Reference data for OMG's built-in functions and injected globals.
//
// Keep in step with `cc_builtins` in src/compiler.omg and with
// `rt_bi_init` in src/runtime/x64/builtins.omg (which also gives the
// argument counts). test/builtins.test.ts fails when a builtin the
// compiler knows is missing here, or the other way round, and runs every
// example below through bin/omg.

export type ValueType =
    | 'int' | 'float' | 'number' | 'string' | 'bool' | 'list' | 'dict'
    | 'frozen dict' | 'none' | 'any' | 'never';

export interface ParamDoc {
    name: string;
    doc: string;
}

export interface BuiltinDoc {
    name: string;
    category: string;
    /** One entry per accepted argument count, shortest first. */
    signatures: string[];
    /** Parameters of the longest signature. */
    params: ParamDoc[];
    minArgs: number;
    maxArgs: number;
    /** One sentence, shown in completion lists. */
    summary: string;
    /** More, in Markdown. */
    details?: string;
    returns?: string;
    returnType?: ValueType;
    /** "Kind: when". */
    raises?: string[];
    /** OMG source. A trailing `# → text` is the line that statement prints. */
    example?: string;
    /** False for examples that wait, need a terminal, or depend on the machine. */
    runnable?: boolean;
    /** Where it behaves differently. */
    platform?: string;
    see?: string[];
}

function b(doc: BuiltinDoc): BuiltinDoc {
    return doc;
}

export const BUILTINS: readonly BuiltinDoc[] = [
    // --- Strings and characters ------------------------------------------
    b({
        name: 'length',
        category: 'Strings and collections',
        signatures: ['length(x)'],
        params: [{ name: 'x', doc: 'A string, a list or a dict.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Number of characters in a string, items in a list, or entries in a dict.',
        details: 'Strings are counted in characters (Unicode code points), not bytes. Use `length(string_bytes(s))` for the byte count.',
        returns: 'An int.',
        returnType: 'int',
        raises: ['TypeError: `x` is not a string, list or dict.'],
        example: 'emit length("héllo")          # → 5\nemit length([10, 20, 30])     # → 3\nemit length({a: 1, b: 2})     # → 2'
    }),
    b({
        name: 'chr',
        category: 'Strings and collections',
        signatures: ['chr(n)'],
        params: [{ name: 'n', doc: 'An int. Only the low 8 bits are used.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'One-character string for the code `n` (0 to 255).',
        details: 'The code is taken modulo 256, so `chr` covers ASCII and Latin-1. For other characters build the UTF-8 bytes and use `bytes_to_string`.',
        returns: 'A one-character string.',
        returnType: 'string',
        raises: ['TypeError: `n` is not an int.'],
        example: 'emit chr(65)                  # → A\nemit chr(72) + chr(105)       # → Hi',
        see: ['ascii', 'bytes_to_string']
    }),
    b({
        name: 'ascii',
        category: 'Strings and collections',
        signatures: ['ascii(c)'],
        params: [{ name: 'c', doc: 'A string of exactly one character.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Code point of a one-character string.',
        returns: 'An int.',
        returnType: 'int',
        raises: ['TypeError: `c` is not a string of exactly one character.'],
        example: 'emit ascii("A")               # → 65\nemit ascii("é")               # → 233',
        see: ['chr', 'string_bytes']
    }),
    b({
        name: 'string_bytes',
        category: 'Strings and collections',
        signatures: ['string_bytes(s)'],
        params: [{ name: 's', doc: 'A string.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'The UTF-8 bytes of a string, as a list of ints (0 to 255).',
        details: 'Indexing a string walks it from the start, so scanning a long text is much faster over its bytes.',
        returns: 'A list of ints.',
        returnType: 'list',
        example: 'emit string_bytes("hé")       # → [104, 195, 169]',
        see: ['bytes_to_string']
    }),
    b({
        name: 'bytes_to_string',
        category: 'Strings and collections',
        signatures: ['bytes_to_string(bytes)'],
        params: [{ name: 'bytes', doc: 'A list of ints, each 0 to 255.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Builds a string from a list of UTF-8 bytes.',
        details: 'The inverse of `string_bytes`.',
        returns: 'A string.',
        returnType: 'string',
        example: 'emit bytes_to_string([104, 105])    # → hi',
        see: ['string_bytes', 'chr']
    }),
    b({
        name: 'hex',
        category: 'Strings and collections',
        signatures: ['hex(n)'],
        params: [{ name: 'n', doc: 'An int.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Lowercase hexadecimal text of an int, without a `0x` prefix.',
        details: 'A negative number gets a leading minus sign.',
        returns: 'A string.',
        returnType: 'string',
        raises: ['TypeError: `n` is not an int.'],
        example: 'emit hex(255)                 # → ff\nemit hex(-255)                # → -ff',
        see: ['binary']
    }),
    b({
        name: 'binary',
        category: 'Strings and collections',
        signatures: ['binary(n)', 'binary(n, width)'],
        params: [
            { name: 'n', doc: 'An int.' },
            { name: 'width', doc: 'How many bits to show. Gives the low `width` bits of `n` in two\'s complement, padded with zeros.' }
        ],
        minArgs: 1, maxArgs: 2,
        summary: 'Binary text of an int, optionally as a fixed number of bits.',
        returns: 'A string.',
        returnType: 'string',
        example: 'emit binary(5)                # → 101\nemit binary(5, 8)             # → 00000101\nemit binary(-1, 8)            # → 11111111',
        see: ['hex']
    }),

    // --- Numbers ----------------------------------------------------------
    b({
        name: 'int',
        category: 'Numbers',
        signatures: ['int(x)'],
        params: [{ name: 'x', doc: 'A float, a string of decimal digits (with an optional sign), a bool or an int.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Converts to an int. Floats are cut towards zero; strings are parsed.',
        details: 'A string must be nothing but an optional `-` and digits: no spaces, no decimal point.',
        returns: 'An int.',
        returnType: 'int',
        raises: ['TypeError: the string is not a whole decimal number.'],
        example: 'emit int("42") + 1            # → 43\nemit int(3.9)                 # → 3\nemit int(-3.9)                # → -3',
        see: ['float', 'floor', 'round']
    }),
    b({
        name: 'float',
        category: 'Numbers',
        signatures: ['float(x)'],
        params: [{ name: 'x', doc: 'An int, a string holding a number, or a float.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Converts to a float.',
        returns: 'A float.',
        returnType: 'float',
        raises: ['TypeError: the string is not a number.'],
        example: 'emit float(3)                 # → 3.0\nemit float("1.5")             # → 1.5\nemit float("1e3")             # → 1000.0',
        see: ['int']
    }),
    b({
        name: 'floor',
        category: 'Numbers',
        signatures: ['floor(x)'],
        params: [{ name: 'x', doc: 'A number.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Rounds down, towards minus infinity.',
        returns: 'An int.',
        returnType: 'int',
        example: 'emit floor(2.7)               # → 2\nemit floor(-2.5)              # → -3',
        see: ['ceil', 'round', 'int']
    }),
    b({
        name: 'ceil',
        category: 'Numbers',
        signatures: ['ceil(x)'],
        params: [{ name: 'x', doc: 'A number.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Rounds up, towards plus infinity.',
        returns: 'An int.',
        returnType: 'int',
        example: 'emit ceil(2.1)                # → 3\nemit ceil(-2.5)               # → -2',
        see: ['floor', 'round']
    }),
    b({
        name: 'round',
        category: 'Numbers',
        signatures: ['round(x)'],
        params: [{ name: 'x', doc: 'A number.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Rounds to the nearest whole number; a half goes to the even one.',
        details: 'This is banker\'s rounding: `round(2.5)` is 2 and `round(3.5)` is 4.',
        returns: 'An int.',
        returnType: 'int',
        example: 'emit round(2.4)               # → 2\nemit round(2.5)               # → 2\nemit round(3.5)               # → 4',
        see: ['floor', 'ceil']
    }),
    b({
        name: 'abs',
        category: 'Numbers',
        signatures: ['abs(x)'],
        params: [{ name: 'x', doc: 'A number.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Absolute value. An int stays an int, a float stays a float.',
        returns: 'A number of the same kind as `x`.',
        returnType: 'number',
        example: 'emit abs(-4)                  # → 4\nemit abs(-4.5)                # → 4.5'
    }),
    b({
        name: 'sqrt',
        category: 'Numbers',
        signatures: ['sqrt(x)'],
        params: [{ name: 'x', doc: 'A number, zero or more.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Square root.',
        returns: 'A float.',
        returnType: 'float',
        raises: ['ValueError: `x` is negative.'],
        example: 'emit sqrt(16)                 # → 4.0'
    }),
    b({
        name: 'pow',
        category: 'Numbers',
        signatures: ['pow(a, b)'],
        params: [
            { name: 'a', doc: 'The base.' },
            { name: 'b', doc: 'The exponent.' }
        ],
        minArgs: 2, maxArgs: 2,
        summary: '`a` to the power `b`. OMG has no `**` operator.',
        details: 'An int raised to a non-negative int stays an int. Anything else gives a float.',
        returns: 'An int or a float.',
        returnType: 'number',
        raises: ['ValueError: the int result does not fit in 64 bits.'],
        example: 'emit pow(2, 10)               # → 1024\nemit pow(2, -1)               # → 0.5\nemit pow(2.0, 3)              # → 8.0'
    }),
    b({
        name: 'log',
        category: 'Numbers',
        signatures: ['log(x)'],
        params: [{ name: 'x', doc: 'A number greater than zero.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Natural logarithm (base e).',
        details: 'For another base, divide: `log(x) / log(10)`.',
        returns: 'A float.',
        returnType: 'float',
        raises: ['ValueError: `x` is zero or negative.'],
        example: 'emit log(1)                   # → 0.0'
    }),
    b({
        name: 'sin',
        category: 'Numbers',
        signatures: ['sin(x)'],
        params: [{ name: 'x', doc: 'An angle in radians.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Sine of an angle in radians.',
        returns: 'A float.',
        returnType: 'float',
        example: 'emit sin(0)                   # → 0.0',
        see: ['cos', 'tan']
    }),
    b({
        name: 'cos',
        category: 'Numbers',
        signatures: ['cos(x)'],
        params: [{ name: 'x', doc: 'An angle in radians.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Cosine of an angle in radians.',
        returns: 'A float.',
        returnType: 'float',
        example: 'emit cos(0)                   # → 1.0',
        see: ['sin', 'tan']
    }),
    b({
        name: 'tan',
        category: 'Numbers',
        signatures: ['tan(x)'],
        params: [{ name: 'x', doc: 'An angle in radians.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Tangent of an angle in radians.',
        returns: 'A float.',
        returnType: 'float',
        example: 'emit tan(0)                   # → 0.0',
        see: ['sin', 'cos']
    }),
    b({
        name: 'float_bits',
        category: 'Numbers',
        signatures: ['float_bits(text)'],
        params: [{ name: 'text', doc: 'A string holding a float, such as `"3.14"`.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'The 64 IEEE-754 bits of the float a text stands for, as an int.',
        details: 'The compiler uses it to store float literals. Note that it takes text, not a float.',
        returns: 'An int.',
        returnType: 'int',
        raises: ['ValueError: the text is not a number.'],
        example: 'emit float_bits("1.0")        # → 4607182418800017408',
        see: ['bits_to_float']
    }),
    b({
        name: 'bits_to_float',
        category: 'Numbers',
        signatures: ['bits_to_float(bits)'],
        params: [{ name: 'bits', doc: 'An int holding an IEEE-754 bit pattern.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'The float whose 64 bits are the given int.',
        returns: 'A float.',
        returnType: 'float',
        example: 'emit bits_to_float(4611686018427387904)    # → 2.0',
        see: ['float_bits']
    }),

    // --- Lists and dicts --------------------------------------------------
    b({
        name: 'append',
        category: 'Strings and collections',
        signatures: ['append(list, item)'],
        params: [
            { name: 'list', doc: 'The list to grow.' },
            { name: 'item', doc: 'The value to add at the end.' }
        ],
        minArgs: 2, maxArgs: 2,
        summary: 'Adds an item to the end of a list, in place.',
        details: 'Prefer it to `xs := xs + [v]`, which copies the whole list each time.',
        returns: 'Nothing.',
        returnType: 'none',
        raises: ['TypeError: the first argument is not a list.'],
        example: 'alloc xs := [1]\nappend(xs, 2)\nemit xs                       # → [1, 2]',
        see: ['list_repeat']
    }),
    b({
        name: 'list_repeat',
        category: 'Strings and collections',
        signatures: ['list_repeat(item, count)'],
        params: [
            { name: 'item', doc: 'The value for every slot.' },
            { name: 'count', doc: 'How many slots. Zero or more.' }
        ],
        minArgs: 2, maxArgs: 2,
        summary: 'A new list holding `item` `count` times.',
        details: 'The way to make a buffer of a known size and then fill it by index. Every slot holds the same value: with a list or dict as `item`, all slots share that one object.',
        returns: 'A list.',
        returnType: 'list',
        raises: ['ValueError: `count` is negative.'],
        example: 'emit list_repeat(0, 3)        # → [0, 0, 0]',
        see: ['append']
    }),
    b({
        name: 'dict_keys',
        category: 'Strings and collections',
        signatures: ['dict_keys(d)'],
        params: [{ name: 'd', doc: 'A dict or a frozen dict.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'The keys of a dict, in the order they were added.',
        details: 'To walk a dict: `for k in dict_keys(d) { ... }`.',
        returns: 'A list of strings.',
        returnType: 'list',
        raises: ['TypeError: `d` is not a dict.'],
        example: 'emit dict_keys({b: 1, a: 2})  # → [b, a]',
        see: ['has_key']
    }),
    b({
        name: 'has_key',
        category: 'Strings and collections',
        signatures: ['has_key(d, key)'],
        params: [
            { name: 'd', doc: 'A dict. Anything else gives `false`.' },
            { name: 'key', doc: 'The key to look for. Turned into a string, like every dict key.' }
        ],
        minArgs: 2, maxArgs: 2,
        summary: 'Whether a dict has the key.',
        details: 'Reading a missing key with `d[k]` or `d.k` raises a KeyError, so check first.',
        returns: 'A bool.',
        returnType: 'bool',
        example: 'alloc d := {a: 1}\nemit has_key(d, "a")          # → true\nemit has_key(d, "b")          # → false',
        see: ['dict_keys']
    }),
    b({
        name: 'freeze',
        category: 'Strings and collections',
        signatures: ['freeze(d)'],
        params: [{ name: 'd', doc: 'A dict.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'A read-only copy of a dict.',
        details: 'Writing to a frozen dict raises a FrozenWriteError. Only the top level is frozen: values inside can still change. Module aliases are frozen dicts, and `return freeze({push: push, pop: pop})` is the usual way to hand out an object made of closures.',
        returns: 'A frozen dict.',
        returnType: 'frozen dict',
        raises: ['TypeError: `d` is not a dict.'],
        example: 'alloc point := freeze({x: 1, y: 2})\nemit point.x                  # → 1\ntry {\n    point.x := 5\n} except err {\n    emit "read-only"          # → read-only\n}'
    }),

    // --- Files --------------------------------------------------------------
    b({
        name: 'read_file',
        category: 'Files',
        signatures: ['read_file(path)'],
        params: [{ name: 'path', doc: 'A file path. A relative one is looked up from `current_dir`.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Reads a whole text file.',
        returns: 'The contents, as a string.',
        returnType: 'string',
        raises: ['ModuleImportError: the file cannot be read. Catch it with `try`, or check `file_exists` first.'],
        example: 'if file_exists("notes.txt") {\n    alloc text := read_file("notes.txt")\n    emit length(text)\n}',
        see: ['file_open', 'file_exists']
    }),
    b({
        name: 'file_open',
        category: 'Files',
        signatures: ['file_open(path, mode)'],
        params: [
            { name: 'path', doc: 'A file path. A relative one is looked up from `current_dir`.' },
            { name: 'mode', doc: '`"r"` read, `"w"` write (starts empty), `"a"` append. Add `b` for bytes: `"rb"`, `"wb"`, `"ab"`. `"rb+"` and `"wb+"` read and write.' }
        ],
        minArgs: 2, maxArgs: 2,
        summary: 'Opens a file and returns a handle for the other `file_*` builtins.',
        details: 'Text modes read and write strings. Binary modes read and write lists of bytes (ints 0 to 255). Close the handle with `file_close` when done.',
        returns: 'An int handle.',
        returnType: 'int',
        raises: ['ValueError: the file cannot be opened, the mode is unknown, or too many files are open.'],
        example: 'alloc h := file_open("greeting.txt", "w")\nfile_write(h, "hi from OMG\\n")\nfile_close(h)',
        runnable: false,
        see: ['file_read', 'file_write', 'file_close', 'read_file']
    }),
    b({
        name: 'file_read',
        category: 'Files',
        signatures: ['file_read(handle)', 'file_read(handle, max)'],
        params: [
            { name: 'handle', doc: 'A handle from `file_open`.' },
            { name: 'max', doc: 'Read at most this many bytes, from where the handle stands.' }
        ],
        minArgs: 1, maxArgs: 2,
        summary: 'Reads from an open file: everything that is left, or at most `max` bytes.',
        details: 'A text-mode handle gives a string, a binary one a list of bytes. At the end of the file the result is empty.',
        returns: 'A string or a list of bytes.',
        returnType: 'any',
        raises: ['ValueError: the handle is not open.'],
        example: 'alloc h := file_open("photo.jpg", "rb")\nalloc first := file_read(h, 16)\nfile_close(h)',
        runnable: false,
        see: ['file_open', 'file_seek']
    }),
    b({
        name: 'file_write',
        category: 'Files',
        signatures: ['file_write(handle, data)'],
        params: [
            { name: 'handle', doc: 'A handle from `file_open`, opened for writing.' },
            { name: 'data', doc: 'A string for a text handle; a list of bytes for a binary one.' }
        ],
        minArgs: 2, maxArgs: 2,
        summary: 'Writes to an open file.',
        returns: 'The number of bytes written.',
        returnType: 'int',
        raises: ['ValueError: the handle is not open.'],
        example: 'alloc h := file_open("log.txt", "a")\nfile_write(h, "one more line\\n")\nfile_close(h)',
        runnable: false,
        see: ['file_open']
    }),
    b({
        name: 'file_seek',
        category: 'Files',
        signatures: ['file_seek(handle, offset)'],
        params: [
            { name: 'handle', doc: 'A handle from `file_open`.' },
            { name: 'offset', doc: 'Byte position, counted from the start of the file.' }
        ],
        minArgs: 2, maxArgs: 2,
        summary: 'Moves an open file to a byte position.',
        returns: 'The new position.',
        returnType: 'int',
        example: 'alloc h := file_open("data.bin", "rb")\nfile_seek(h, 128)\nalloc block := file_read(h, 64)\nfile_close(h)',
        runnable: false,
        see: ['file_tell']
    }),
    b({
        name: 'file_tell',
        category: 'Files',
        signatures: ['file_tell(handle)'],
        params: [{ name: 'handle', doc: 'A handle from `file_open`.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'The current byte position in an open file.',
        returns: 'An int.',
        returnType: 'int',
        see: ['file_seek'],
        example: 'alloc h := file_open("notes.txt", "w")\nfile_write(h, "hello")\nemit file_tell(h)             # → 5\nfile_close(h)'
    }),
    b({
        name: 'file_close',
        category: 'Files',
        signatures: ['file_close(handle)'],
        params: [{ name: 'handle', doc: 'A handle from `file_open`.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Closes an open file.',
        returns: 'Nothing.',
        returnType: 'none',
        see: ['file_open'],
        example: 'alloc h := file_open("notes.txt", "w")\nfile_write(h, "saved")\nfile_close(h)\nemit read_file("notes.txt")   # → saved'
    }),
    b({
        name: 'file_exists',
        category: 'Files',
        signatures: ['file_exists(path)'],
        params: [{ name: 'path', doc: 'A file or directory path.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Whether something exists at the path (a file or a directory).',
        returns: 'A bool.',
        returnType: 'bool',
        example: 'if file_exists("config.omg") {\n    emit "found it"\n}',
        see: ['is_dir']
    }),
    b({
        name: 'file_size',
        category: 'Files',
        signatures: ['file_size(path)'],
        params: [{ name: 'path', doc: 'A file path.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'A file\'s length in bytes.',
        returns: 'An int.',
        returnType: 'int',
        raises: ['ValueError: there is nothing at the path.'],
        see: ['file_mtime', 'file_exists'],
        example: 'alloc h := file_open("notes.txt", "w")\nfile_write(h, "hello")\nfile_close(h)\nemit file_size("notes.txt")   # → 5'
    }),
    b({
        name: 'file_mtime',
        category: 'Files',
        signatures: ['file_mtime(path)'],
        params: [{ name: 'path', doc: 'A file path.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'When a file was last changed, in milliseconds since 1970.',
        details: 'On the same clock as `time_ms`.',
        returns: 'An int.',
        returnType: 'int',
        raises: ['ValueError: there is nothing at the path.'],
        see: ['file_size', 'time_ms'],
        example: 'alloc h := file_open("notes.txt", "w")\nfile_close(h)\nalloc age := time_ms() - file_mtime("notes.txt")\nemit age < 5000               # → true'
    }),
    b({
        name: 'is_dir',
        category: 'Files',
        signatures: ['is_dir(path)'],
        params: [{ name: 'path', doc: 'A path.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Whether the path is a directory.',
        returns: 'A bool. `false` when nothing is there.',
        returnType: 'bool',
        see: ['read_dir', 'file_exists'],
        example: 'emit is_dir(".")              # → true\nemit is_dir("no/such/place")  # → false'
    }),
    b({
        name: 'read_dir',
        category: 'Files',
        signatures: ['read_dir(path)'],
        params: [{ name: 'path', doc: 'A directory path.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'The names in a directory, sorted, without `.` and `..`.',
        returns: 'A list of strings.',
        returnType: 'list',
        raises: ['ValueError: the directory cannot be read.'],
        example: 'for name in read_dir(".") {\n    emit name\n}',
        runnable: false,
        see: ['is_dir']
    }),
    b({
        name: 'make_dir',
        category: 'Files',
        signatures: ['make_dir(path)'],
        params: [{ name: 'path', doc: 'The directory to create.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Creates a directory and any missing parents (like `mkdir -p`).',
        returns: '`true`, also when the directory was already there.',
        returnType: 'bool',
        raises: ['ValueError: the directory cannot be created.'],
        example: 'make_dir("build/out/logs")\nemit is_dir("build/out")      # → true'
    }),
    b({
        name: 'chmod',
        category: 'Files',
        signatures: ['chmod(path, mode)'],
        params: [
            { name: 'path', doc: 'A file path.' },
            { name: 'mode', doc: 'Permission bits as an int. 493 is `rwxr-xr-x` (octal 755); 420 is `rw-r--r--` (octal 644).' }
        ],
        minArgs: 2, maxArgs: 2,
        summary: 'Sets a file\'s permission bits.',
        returns: '`true`.',
        returnType: 'bool',
        platform: 'On Windows it only checks that the file exists.',
        example: 'alloc h := file_open("run.sh", "w")\nfile_write(h, "echo hi\\n")\nfile_close(h)\n# rwxr-xr-x, which other languages write as 0755\nemit chmod("run.sh", 0b111101101)     # → true'
    }),

    // --- Console ------------------------------------------------------------
    b({
        name: 'print',
        category: 'Console',
        signatures: ['print(text)'],
        params: [{ name: 'text', doc: 'A string. To print another value, join it to one: `print("" + n)`.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Writes a string to standard output with no newline after it.',
        details: '`emit` prints any value and ends the line; `print` is for prompts, progress and screen drawing.',
        returns: 'Nothing.',
        returnType: 'none',
        raises: ['TypeError: `text` is not a string.'],
        example: 'print("loading")\nprint("...")\nemit " done"                  # → loading... done'
    }),
    b({
        name: 'stdin_readline',
        category: 'Console',
        signatures: ['stdin_readline()'],
        params: [],
        minArgs: 0, maxArgs: 0,
        summary: 'Reads the next line of standard input, without its line end.',
        returns: 'A string, or `false` when the input has ended.',
        returnType: 'any',
        example: 'print("name? ")\nalloc name := stdin_readline()\nif name == false {\n    name := "stranger"\n}\nemit "hello, " + name',
        runnable: false,
        see: ['stdin_read']
    }),
    b({
        name: 'stdin_read',
        category: 'Console',
        signatures: ['stdin_read()'],
        params: [],
        minArgs: 0, maxArgs: 0,
        summary: 'Reads all of standard input, up to its end, as one string.',
        returns: 'A string; empty when there is no input.',
        returnType: 'string',
        see: ['stdin_readline', 'stdin_read_bytes'],
        example: '# cat notes.txt | omg count.omg\nalloc text := stdin_read()\nemit length(text) + " characters"'
    }),
    b({
        name: 'stdin_read_bytes',
        category: 'Console',
        signatures: ['stdin_read_bytes()'],
        params: [],
        minArgs: 0, maxArgs: 0,
        summary: 'Reads all of standard input, up to its end, as a list of bytes.',
        returns: 'A list of ints (0 to 255).',
        returnType: 'list',
        see: ['stdin_read'],
        example: '# omg size.omg < photo.jpg\nalloc data := stdin_read_bytes()\nemit length(data) + " bytes"'
    }),
    b({
        name: 'stdin_set_raw',
        category: 'Console',
        signatures: ['stdin_set_raw(on)'],
        params: [{ name: 'on', doc: '`true` to read keys as they are pressed, with no echo; `false` to go back to line input.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Switches the terminal between line input and key-by-key input.',
        details: 'Games use it with `stdin_read_key`. Switch it off again before the program ends, or the terminal stays that way.',
        returns: 'Nothing.',
        returnType: 'none',
        raises: ['TypeError: `on` is not a bool.'],
        example: 'stdin_set_raw(true)\nalloc key := false\nloop key == false {\n    key := stdin_read_key()\n    sleep_ms(10)\n}\nstdin_set_raw(false)\nemit "you pressed " + key',
        runnable: false,
        see: ['stdin_read_key']
    }),
    b({
        name: 'stdin_read_key',
        category: 'Console',
        signatures: ['stdin_read_key()'],
        params: [],
        minArgs: 0, maxArgs: 0,
        summary: 'One byte of keyboard input if a key is waiting; it never waits.',
        details: 'Arrow keys and other special keys arrive as several bytes, one per call, starting with escape (code 27).',
        returns: 'A one-character string, or `false` when nothing is waiting.',
        returnType: 'any',
        see: ['stdin_set_raw'],
        example: 'alloc key := stdin_read_key()\nif key == false {\n    emit "no key waiting"\n} else {\n    emit "key code " + ascii(key)\n}'
    }),

    // --- Time ---------------------------------------------------------------
    b({
        name: 'time_ms',
        category: 'Time and process',
        signatures: ['time_ms()'],
        params: [],
        minArgs: 0, maxArgs: 0,
        summary: 'The current time, in milliseconds since 1970.',
        details: 'Subtract two readings to time something.',
        returns: 'An int.',
        returnType: 'int',
        example: 'alloc started := time_ms()\nsleep_ms(20)\nemit time_ms() - started >= 20    # → true',
        see: ['sleep_ms']
    }),
    b({
        name: 'sleep_ms',
        category: 'Time and process',
        signatures: ['sleep_ms(ms)'],
        params: [{ name: 'ms', doc: 'How long to pause, in milliseconds.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Pauses the program.',
        returns: 'Nothing.',
        returnType: 'none',
        see: ['time_ms'],
        example: 'alloc start := time_ms()\nsleep_ms(50)\nemit time_ms() - start >= 50  # → true'
    }),

    // --- Process ------------------------------------------------------------
    b({
        name: 'exit',
        category: 'Time and process',
        signatures: ['exit(code)'],
        params: [{ name: 'code', doc: 'The exit status. 0 means success.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Ends the program at once with an exit status.',
        details: 'Nothing after it runs, and `try` does not catch it.',
        returns: 'Does not return.',
        returnType: 'never',
        example: 'if length(args) < 2 {\n    emit "usage: tool <file>"\n    exit(2)\n}',
        runnable: false,
        see: ['exit_with_error']
    }),
    b({
        name: 'exit_with_error',
        category: 'Time and process',
        signatures: ['exit_with_error(message)'],
        params: [{ name: 'message', doc: 'Text for standard error, printed as it is.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Prints a message to standard error and exits with status 1.',
        details: 'No traceback and no error kind in front, and `try` does not catch it. For messages meant for the person running the program.',
        returns: 'Does not return.',
        returnType: 'never',
        see: ['exit', 'panic'],
        example: 'if length(args) < 2 {\n    exit_with_error("usage: greet <name>")\n}\nemit "hello, " + args[1]',
        runnable: false
    }),
    b({
        name: 'getpid',
        category: 'Time and process',
        signatures: ['getpid()'],
        params: [],
        minArgs: 0, maxArgs: 0,
        summary: 'The id of this process.',
        details: 'Handy for temporary file names that do not clash.',
        returns: 'An int.',
        returnType: 'int',
        example: 'emit "running as process " + getpid()'
    }),
    b({
        name: 'executable_path',
        category: 'Time and process',
        signatures: ['executable_path()'],
        params: [],
        minArgs: 0, maxArgs: 0,
        summary: 'Path of the native executable that is running this program.',
        details: 'For a program run with `omg file.omg` that is `omg` itself; for a built program it is the program.',
        returns: 'A string, or `false` where there is no executable (in the browser).',
        returnType: 'any',
        example: 'alloc exe := executable_path()\nif exe == false {\n    emit "no executable here"\n} else {\n    emit "running " + exe\n}'
    }),
    b({
        name: 'subprocess',
        category: 'Time and process',
        signatures: ['subprocess(argv)'],
        params: [{ name: 'argv', doc: 'A list of strings: the program, then its arguments.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Runs another program, waits for it, and returns its exit code.',
        details: 'The child shares this program\'s input and output. No shell is involved, so nothing in `argv` is expanded or split.',
        returns: 'The exit code, as an int.',
        returnType: 'int',
        raises: ['ValueError: the program cannot be started.'],
        example: 'alloc code := subprocess(["git", "status", "--short"])\nif code != 0 {\n    emit "git failed"\n}',
        runnable: false
    }),
    b({
        name: 'fork',
        category: 'Time and process',
        signatures: ['fork()'],
        params: [],
        minArgs: 0, maxArgs: 0,
        summary: 'Splits the process in two (POSIX `fork`).',
        details: 'Finished children are cleaned up on their own; there is no `wait`.',
        returns: '0 in the child; the child\'s process id in the parent.',
        returnType: 'int',
        platform: 'Not on Windows, where it raises a ValueError you can catch.',
        example: 'alloc pid := fork()\nif pid == 0 {\n    emit "in the child"\n    exit(0)\n}\nemit "started child " + pid',
        runnable: false
    }),
    b({
        name: 'interrupted',
        category: 'Time and process',
        signatures: ['interrupted()'],
        params: [],
        minArgs: 0, maxArgs: 0,
        summary: 'How many times the program has been asked to stop (Ctrl-C, SIGTERM).',
        details: 'The first call takes those requests over: from then on they no longer end the program, they are counted here for it to act on. A server uses it to finish what it is doing and close down cleanly.',
        returns: 'An int.',
        returnType: 'int',
        example: 'loop interrupted() == 0 {\n    sleep_ms(100)\n}\nemit "shutting down"',
        runnable: false,
        see: ['tcp_poll']
    }),

    // --- Pseudo-terminals ---------------------------------------------------
    b({
        name: 'pty_spawn',
        category: 'Pseudo-terminals',
        signatures: ['pty_spawn(argv)'],
        params: [{ name: 'argv', doc: 'A list of strings: the program, then its arguments.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Starts a program inside a pseudo-terminal and returns a handle to it.',
        details: 'The program believes it is talking to a terminal. Read what it prints with `pty_read`, type into it with `pty_write`.',
        returns: 'An int handle.',
        returnType: 'int',
        raises: ['ValueError: the program cannot be started.'],
        platform: 'Not on Windows, where it raises a ValueError you can catch.',
        see: ['pty_read', 'pty_write', 'pty_close', 'pty_resize'],
        example: 'alloc sh := pty_spawn(["/bin/sh"])\npty_write(sh, "echo hi\\n")\nsleep_ms(100)\nprint(pty_read(sh))\npty_close(sh)',
        runnable: false
    }),
    b({
        name: 'pty_read',
        category: 'Pseudo-terminals',
        signatures: ['pty_read(handle)'],
        params: [{ name: 'handle', doc: 'A handle from `pty_spawn`.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'What the program has written since the last read. It never waits.',
        returns: 'A string (`""` when there is nothing new), or `false` once the program has gone.',
        returnType: 'any',
        see: ['pty_spawn'],
        example: 'alloc out := pty_read(sh)\nif out == false {\n    emit "the program has exited"\n} elif out != "" {\n    print(out)\n}',
        runnable: false
    }),
    b({
        name: 'pty_write',
        category: 'Pseudo-terminals',
        signatures: ['pty_write(handle, text)'],
        params: [
            { name: 'handle', doc: 'A handle from `pty_spawn`.' },
            { name: 'text', doc: 'What to type into the program.' }
        ],
        minArgs: 2, maxArgs: 2,
        summary: 'Sends text to the program, as if it had been typed.',
        see: ['pty_spawn'],
        returns: 'The number of bytes written.',
        example: 'pty_write(sh, "ls -la\\n")',
        runnable: false
    }),
    b({
        name: 'pty_close',
        category: 'Pseudo-terminals',
        signatures: ['pty_close(handle)'],
        params: [{ name: 'handle', doc: 'A handle from `pty_spawn`.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Closes a pseudo-terminal.',
        see: ['pty_spawn'],
        returns: 'Nothing.',
        example: 'pty_write(sh, "exit\\n")\npty_close(sh)',
        runnable: false
    }),
    b({
        name: 'pty_resize',
        category: 'Pseudo-terminals',
        signatures: ['pty_resize(handle, rows, cols)'],
        params: [
            { name: 'handle', doc: 'A handle from `pty_spawn`.' },
            { name: 'rows', doc: 'Height in character rows.' },
            { name: 'cols', doc: 'Width in character columns.' }
        ],
        minArgs: 3, maxArgs: 3,
        summary: 'Tells the program its terminal has a new size.',
        see: ['pty_spawn'],
        returns: 'Nothing.',
        example: '# 40 rows of 120 columns\npty_resize(sh, 40, 120)',
        runnable: false
    }),

    // --- Networking ---------------------------------------------------------
    b({
        name: 'tcp_listen',
        category: 'Networking',
        signatures: ['tcp_listen(host, port)'],
        params: [
            { name: 'host', doc: 'The address to listen on: `"127.0.0.1"` for this machine only, `"0.0.0.0"` for everyone.' },
            { name: 'port', doc: 'The port number.' }
        ],
        minArgs: 2, maxArgs: 2,
        summary: 'Opens a listening TCP socket.',
        returns: 'An int handle, to pass to `tcp_accept`.',
        returnType: 'int',
        raises: ['ValueError: the address cannot be used (already taken, not allowed).'],
        example: 'alloc server := tcp_listen("127.0.0.1", 8080)\nloop true {\n    alloc conn := tcp_accept(server)\n    tcp_write(conn, "hello\\n")\n    tcp_close(conn)\n}',
        runnable: false,
        see: ['tcp_accept', 'tcp_poll']
    }),
    b({
        name: 'tcp_accept',
        category: 'Networking',
        signatures: ['tcp_accept(listener)'],
        params: [{ name: 'listener', doc: 'A handle from `tcp_listen`.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Takes the next incoming connection, waiting for one if need be.',
        returns: 'An int handle for the connection. `false` when the listener is non-blocking and nobody is waiting.',
        returnType: 'any',
        see: ['tcp_listen', 'tcp_nonblocking'],
        example: 'alloc server := tcp_listen("127.0.0.1", 8080)\nalloc conn := tcp_accept(server)\nemit "connection from " + tcp_peer(conn)\ntcp_close(conn)',
        runnable: false
    }),
    b({
        name: 'tcp_connect',
        category: 'Networking',
        signatures: ['tcp_connect(host, port)'],
        params: [
            { name: 'host', doc: 'An address such as `"127.0.0.1"`.' },
            { name: 'port', doc: 'The port number.' }
        ],
        minArgs: 2, maxArgs: 2,
        summary: 'Opens a TCP connection to another machine or program.',
        returns: 'An int handle.',
        returnType: 'int',
        raises: ['ValueError: the connection cannot be made.'],
        see: ['tcp_read', 'tcp_write'],
        example: 'alloc conn := tcp_connect("127.0.0.1", 8080)\ntcp_write(conn, "GET / HTTP/1.0\\r\\n\\r\\n")\nemit bytes_to_string(tcp_read(conn, 4096))\ntcp_close(conn)',
        runnable: false
    }),
    b({
        name: 'tcp_read',
        category: 'Networking',
        signatures: ['tcp_read(handle, max)'],
        params: [
            { name: 'handle', doc: 'A connection handle.' },
            { name: 'max', doc: 'Read at most this many bytes.' }
        ],
        minArgs: 2, maxArgs: 2,
        summary: 'Reads up to `max` bytes from a connection.',
        details: 'Three different results to tell apart: a list with bytes; `[]` once the other side has closed; `false` when the socket is non-blocking and nothing has arrived yet. Compare with `== false` to separate the last two.',
        returns: 'A list of bytes, or `false`.',
        returnType: 'any',
        example: 'alloc data := tcp_read(conn, 4096)\nif data == false {\n    # nothing yet\n} elif length(data) == 0 {\n    tcp_close(conn)\n} else {\n    emit bytes_to_string(data)\n}',
        runnable: false,
        see: ['tcp_write', 'tcp_nonblocking']
    }),
    b({
        name: 'tcp_write',
        category: 'Networking',
        signatures: ['tcp_write(handle, data)', 'tcp_write(handle, data, offset)'],
        params: [
            { name: 'handle', doc: 'A connection handle.' },
            { name: 'data', doc: 'A string or a list of bytes.' },
            { name: 'offset', doc: 'Start this many bytes into `data`: for carrying on after a partial write.' }
        ],
        minArgs: 2, maxArgs: 3,
        summary: 'Sends a string or bytes over a connection.',
        details: 'On a non-blocking socket it sends what fits; use the return value as the next `offset`.',
        returns: 'The number of bytes sent.',
        returnType: 'int',
        see: ['tcp_read', 'tcp_nonblocking'],
        example: 'alloc sent := tcp_write(conn, "hello\\n")\nemit sent + " bytes sent"',
        runnable: false
    }),
    b({
        name: 'tcp_close',
        category: 'Networking',
        signatures: ['tcp_close(handle)'],
        params: [{ name: 'handle', doc: 'A listener or connection handle.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Closes a socket.',
        returnType: 'none',
        returns: 'Nothing.',
        example: 'tcp_write(conn, "bye\\n")\ntcp_close(conn)',
        runnable: false
    }),
    b({
        name: 'tcp_peer',
        category: 'Networking',
        signatures: ['tcp_peer(handle)'],
        params: [{ name: 'handle', doc: 'A connection handle.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Who is at the other end of a connection.',
        returns: 'A string such as `"203.0.113.9:53012"`.',
        returnType: 'string',
        raises: ['ValueError: the handle is not an open connection.'],
        example: 'alloc conn := tcp_accept(server)\nemit "connection from " + tcp_peer(conn)',
        runnable: false
    }),
    b({
        name: 'tcp_nonblocking',
        category: 'Networking',
        signatures: ['tcp_nonblocking(handle, on)'],
        params: [
            { name: 'handle', doc: 'A listener or connection handle.' },
            { name: 'on', doc: '`true` so that calls on this socket never wait.' }
        ],
        minArgs: 2, maxArgs: 2,
        summary: 'Chooses whether calls on a socket wait.',
        details: 'With it on, `tcp_accept` and `tcp_read` return `false` instead of waiting when there is nothing, and `tcp_write` sends what fits and says how much that was. Sockets start out blocking.',
        see: ['tcp_poll'],
        returns: 'Nothing.',
        example: 'alloc server := tcp_listen("127.0.0.1", 8080)\ntcp_nonblocking(server, true)\nalloc conn := tcp_accept(server)\nif conn == false {\n    emit "nobody is waiting"\n}',
        runnable: false
    }),
    b({
        name: 'tcp_poll',
        category: 'Networking',
        signatures: ['tcp_poll(reads, writes, timeout_ms)'],
        params: [
            { name: 'reads', doc: 'Handles to watch for something to read (for a listener: a connection to accept).' },
            { name: 'writes', doc: 'Handles to watch for room to send.' },
            { name: 'timeout_ms', doc: 'How long to wait. Negative waits without limit; 0 only looks.' }
        ],
        minArgs: 3, maxArgs: 3,
        summary: 'Waits until one of several sockets is ready, or the time is up.',
        details: 'A socket in trouble is reported as ready, so that the read or write that follows finds out. It returns early when the program is interrupted (see `interrupted`).',
        returns: '`[readable, writable]`: two lists of the handles that are ready, in the order they were given.',
        returnType: 'list',
        example: 'alloc ready := tcp_poll([server], [], 1000)\nfor h in ready[0] {\n    alloc conn := tcp_accept(h)\n}',
        runnable: false,
        see: ['tcp_nonblocking', 'interrupted']
    }),

    // --- A window -----------------------------------------------------------
    b({
        name: 'window_open',
        category: 'A window',
        signatures: ['window_open(title, width, height)'],
        params: [
            { name: 'title', doc: 'What the title bar says.' },
            { name: 'width', doc: 'Width of the inside of the window, in pixels.' },
            { name: 'height', doc: 'Its height.' }
        ],
        minArgs: 3, maxArgs: 3,
        summary: 'Opens a window to draw pictures in.',
        details: 'A program has one window at a time. On Linux the runtime speaks to the X server that `DISPLAY` names; on Windows it asks user32. Nothing is linked in for either. Only in a program run by `omg` or built to machine code: built through C or as JavaScript, it raises a `ValueError`, as it does when there is no display.',
        returns: 'A handle for the other `window_` builtins.',
        returnType: 'int',
        example: 'alloc win := window_open("hello", 320, 240)\nalloc pixels := list_repeat(255 * 65536, 320 * 240)\nwindow_draw(win, pixels, 320)\nsleep_ms(2000)\nwindow_close(win)',
        runnable: false,
        see: ['window_draw', 'window_poll', 'window_close']
    }),
    b({
        name: 'window_draw',
        category: 'A window',
        signatures: ['window_draw(window, pixels, width)'],
        params: [
            { name: 'window', doc: 'A handle from `window_open`.' },
            { name: 'pixels', doc: 'A list with one int per pixel, `red * 65536 + green * 256 + blue`, row after row from the top.' },
            { name: 'width', doc: 'How many pixels make a row. The height follows from the length of the list.' }
        ],
        minArgs: 3, maxArgs: 3,
        summary: 'Shows a picture in the window.',
        details: 'The picture does not have to be the size of the window: it is scaled to fit, keeping its shape, with black bars where the shapes differ. The window keeps the picture and repaints it by itself when it has been covered.',
        returns: 'Nothing.',
        example: 'alloc win := window_open("stripes", 256, 100)\nalloc pixels := []\nalloc y := 0\nloop y < 100 {\n    alloc x := 0\n    loop x < 256 {\n        append(pixels, x * 65536 + (255 - x))\n        x := x + 1\n    }\n    y := y + 1\n}\nwindow_draw(win, pixels, 256)',
        runnable: false,
        see: ['window_open', 'window_poll']
    }),
    b({
        name: 'window_poll',
        category: 'A window',
        signatures: ['window_poll(window)'],
        params: [{ name: 'window', doc: 'A handle from `window_open`.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'The next thing that happened to the window, without waiting.',
        details: 'Call it until it returns `false`, every few milliseconds: it is also what keeps the window alive. Keys are named by their character (`"a"`, `"A"`, `"."`), or `"space"`, `"enter"`, `"escape"`, `"tab"`, `"backspace"`, `"delete"`, `"left"`, `"right"`, `"up"`, `"down"`, `"home"`, `"end"`, `"pageup"`, `"pagedown"`, `"f1"` to `"f12"`.',
        returns: '`false` when nothing happened, or a list: `["key", name]`, `["resize", width, height]`, or `["close"]` when the user closed the window.',
        example: 'alloc win := window_open("press q", 200, 100)\nalloc open := true\nloop open {\n    alloc ev := window_poll(win)\n    loop ev != false {\n        if ev[0] == "close" or (ev[0] == "key" and ev[1] == "q") {\n            open := false\n        }\n        ev := window_poll(win)\n    }\n    sleep_ms(10)\n}\nwindow_close(win)',
        runnable: false,
        see: ['window_open', 'window_draw']
    }),
    b({
        name: 'window_close',
        category: 'A window',
        signatures: ['window_close(window)'],
        params: [{ name: 'window', doc: 'A handle from `window_open`.' }],
        minArgs: 1, maxArgs: 1,
        summary: 'Closes the window.',
        details: 'After it another window can be opened. A window still open when the program ends goes with it.',
        returns: 'Nothing.',
        example: 'alloc win := window_open("gone in a second", 200, 100)\nsleep_ms(1000)\nwindow_close(win)',
        runnable: false,
        see: ['window_open']
    }),

    // --- Reflection ---------------------------------------------------------
    b({
        name: 'call_builtin',
        category: 'Reflection',
        signatures: ['call_builtin(name, args)'],
        params: [
            { name: 'name', doc: 'The builtin\'s name, as a string.' },
            { name: 'args', doc: 'A list of its arguments (at most three).' }
        ],
        minArgs: 2, maxArgs: 2,
        summary: 'Calls a builtin by name.',
        details: 'The interpreter written in OMG uses it to pass builtin calls on to the runtime underneath. Rarely useful elsewhere.',
        returns: 'Whatever the builtin returns.',
        returnType: 'any',
        raises: ['TypeError: there is no builtin of that name.'],
        example: 'emit call_builtin("length", ["abc"])    # → 3'
    })
];

/** `panic` and `raise` are not in the builtin table: they compile to a RAISE instruction. */
export const RAISERS: readonly BuiltinDoc[] = [
    b({
        name: 'panic',
        category: 'Errors',
        signatures: ['panic(message)'],
        params: [{ name: 'message', doc: 'What went wrong.' }],
        minArgs: 0, maxArgs: 1,
        summary: 'Raises a RuntimeError with the given message.',
        details: 'Inside `try` the error goes to the `except` block, as the string `"RuntimeError: message"`. Uncaught, the program prints a traceback and exits with status 1. `panic` and `raise` are the same thing.',
        returns: 'Does not return.',
        returnType: 'never',
        example: 'proc divide(a, b) {\n    if b == 0 {\n        panic("division by zero")\n    }\n    return a / b\n}\n\ntry {\n    emit divide(10, 0)\n} except err {\n    emit err                  # → RuntimeError: division by zero\n}',
        see: ['raise', 'exit_with_error']
    }),
    b({
        name: 'raise',
        category: 'Errors',
        signatures: ['raise(message)'],
        params: [{ name: 'message', doc: 'What went wrong.' }],
        minArgs: 0, maxArgs: 1,
        summary: 'Raises a RuntimeError with the given message. The same as `panic`.',
        details: 'Inside `try` the error goes to the `except` block, as the string `"RuntimeError: message"`. Uncaught, the program prints a traceback and exits with status 1.',
        returns: 'Does not return.',
        returnType: 'never',
        example: 'try {\n    raise("not today")\n} except err {\n    emit err                  # → RuntimeError: not today\n}',
        see: ['panic']
    })
];

const BY_NAME = new Map<string, BuiltinDoc>();
for (const d of BUILTINS) BY_NAME.set(d.name, d);
const RAISER_BY_NAME = new Map<string, BuiltinDoc>();
for (const d of RAISERS) RAISER_BY_NAME.set(d.name, d);

export function builtin(name: string): BuiltinDoc | undefined {
    return BY_NAME.get(name);
}

export function raiser(name: string): BuiltinDoc | undefined {
    return RAISER_BY_NAME.get(name);
}

/** Either kind: what a call to `name(...)` means when nothing else is said. */
export function callable(name: string): BuiltinDoc | undefined {
    return BY_NAME.get(name) ?? RAISER_BY_NAME.get(name);
}

export function isBuiltin(name: string): boolean {
    return BY_NAME.has(name);
}

/** Internal raise helpers the toolchain's own sources use. */
export function isInternalRaiser(name: string): boolean {
    return /^_omg_vm_[a-z_]+_error_handle$/.test(name);
}

export interface GlobalDoc {
    name: string;
    type: ValueType;
    summary: string;
    details: string;
    example?: string;
}

export const RESERVED_GLOBALS: readonly GlobalDoc[] = [
    {
        name: 'args',
        type: 'list',
        summary: 'The command line, as a list of strings.',
        details: '`args[0]` is the program itself (the script, or the built executable). What the user typed after it starts at `args[1]`.',
        example: 'if length(args) < 2 {\n    emit "usage: " + args[0] + " <name>"\n} else {\n    emit "hello, " + args[1]\n}'
    },
    {
        name: 'module_file',
        type: 'string',
        summary: 'Path of the running script, or of the executable for a built program.',
        details: 'Useful for finding files that sit next to the program.'
    },
    {
        name: 'current_dir',
        type: 'string',
        summary: 'The directory the program was started from.',
        details: 'Relative paths given to `read_file`, `file_open` and the other file builtins are looked up from here.'
    }
];

const GLOBAL_BY_NAME = new Map<string, GlobalDoc>();
for (const g of RESERVED_GLOBALS) GLOBAL_BY_NAME.set(g.name, g);

export function reservedGlobal(name: string): GlobalDoc | undefined {
    return GLOBAL_BY_NAME.get(name);
}

// --- Raw mode -----------------------------------------------------------------
//
// The x86-64 runtime (src/runtime/x64/*.omg) is written in OMG compiled
// under different rules: every value is one untyped 64-bit word and
// memory is reached through reserved names. See docs/06-runtime.md.

export interface RawOpDoc {
    name: string;
    signature: string;
    argc: number;
    summary: string;
}

function raw(signature: string, summary: string): RawOpDoc {
    const name = signature.slice(0, signature.indexOf('('));
    const inside = signature.slice(signature.indexOf('(') + 1, -1).trim();
    return { name, signature, argc: inside === '' ? 0 : inside.split(',').length, summary };
}

export const RAW_OPS: readonly RawOpDoc[] = [
    raw('__peek8(addr)', 'Reads the byte at a memory address.'),
    raw('__peek16(addr)', 'Reads the 16-bit word at a memory address.'),
    raw('__peek32(addr)', 'Reads the 32-bit word at a memory address.'),
    raw('__peek64(addr)', 'Reads the 64-bit word at a memory address.'),
    raw('__poke8(addr, value)', 'Writes a byte to a memory address.'),
    raw('__poke16(addr, value)', 'Writes a 16-bit word to a memory address.'),
    raw('__poke32(addr, value)', 'Writes a 32-bit word to a memory address.'),
    raw('__poke64(addr, value)', 'Writes a 64-bit word to a memory address.'),
    raw('__memcpy(dst, src, n)', 'Copies `n` bytes from `src` to `dst`.'),
    raw('__memset(dst, byte, n)', 'Fills `n` bytes at `dst` with one byte value.'),
    raw('__memcmp(a, b, n)', 'Compares `n` bytes at two addresses.'),
    raw('__udiv(a, b)', 'Unsigned 64-bit division.'),
    raw('__umod(a, b)', 'Unsigned 64-bit remainder.'),
    raw('__ult(a, b)', 'Unsigned less-than.'),
    raw('__lsr(x, n)', 'Logical (zero-filling) shift right.'),
    raw('__mulhi(a, b)', 'The high 64 bits of a 128-bit product.'),
    raw('__fadd(a, b)', 'Adds two floats given as bit patterns.'),
    raw('__fsub(a, b)', 'Subtracts two floats given as bit patterns.'),
    raw('__fmul(a, b)', 'Multiplies two floats given as bit patterns.'),
    raw('__fdiv(a, b)', 'Divides two floats given as bit patterns.'),
    raw('__flt(a, b)', 'Float less-than.'),
    raw('__fle(a, b)', 'Float less-than-or-equal.'),
    raw('__feq(a, b)', 'Float equality.'),
    raw('__i2f(n)', 'Converts an integer to the bits of a float.'),
    raw('__f2i(f)', 'Converts the bits of a float to an integer.'),
    raw('__fsqrt(f)', 'Float square root.'),
    raw('__fsin(f)', 'Float sine.'),
    raw('__fcos(f)', 'Float cosine.'),
    raw('__ftan(f)', 'Float tangent.'),
    raw('__flog(f)', 'Float natural logarithm.'),
    raw('__fpow(a, b)', 'Float power.'),
    raw('__syscall(number, a, b, c, d, e, f)', 'Makes a Linux system call with six arguments.'),
    raw('__rsp()', 'The stack pointer.'),
    raw('__rbp()', 'The frame pointer.'),
    raw('__unwind(rsp, rbp, target, a, b)', 'Resets the stack and frame pointers and jumps to `target`: how a raised error reaches its handler.'),
    raw('__sym(name)', 'The address of a named proc or data item. Takes a string literal.'),
    raw('__brk()', 'A breakpoint instruction (`int3`).'),
    raw('__import(name)', 'A Windows system function, named as `"kernel32.dll!WriteFile"`. Windows only.')
];

const RAW_BY_NAME = new Map<string, RawOpDoc>();
for (const r of RAW_OPS) RAW_BY_NAME.set(r.name, r);

export function rawOp(name: string): RawOpDoc | undefined {
    const known = RAW_BY_NAME.get(name);
    if (known) return known;
    let m = /^__icall([0-6])$/.exec(name);
    if (m) {
        const n = Number(m[1]);
        const ps = ['addr'].concat(Array.from({ length: n }, (_, i) => 'a' + (i + 1)));
        return { name, signature: name + '(' + ps.join(', ') + ')', argc: n + 1, summary: 'Calls the proc at an address with ' + n + ' argument' + (n === 1 ? '' : 's') + '.' };
    }
    m = /^__wincall(\d|1[0-2])$/.exec(name);
    if (m) {
        const n = Number(m[1]);
        const ps = ['fn'].concat(Array.from({ length: n }, (_, i) => 'a' + (i + 1)));
        return { name, signature: name + '(' + ps.join(', ') + ')', argc: n + 1, summary: 'Calls a Windows system function (from `__import`) with ' + n + ' argument' + (n === 1 ? '' : 's') + '.' };
    }
    return undefined;
}
