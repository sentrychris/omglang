// Hover text for keywords, operators and the file header: the parts of
// the language that are not a name you can look up.
//
// Every example is run through bin/omg by test/builtins.test.ts.

export interface KeywordDoc {
    word: string;
    /** How it is written. */
    syntax: string;
    summary: string;
    details?: string;
    example?: string;
    runnable?: boolean;
}

export const KEYWORD_DOCS: readonly KeywordDoc[] = [
    {
        word: 'alloc',
        syntax: 'alloc name := value',
        summary: 'Declares a new variable.',
        details:
            'Every new name needs `alloc`; a plain `name := value` only changes a variable that already exists.\n\n' +
            '- At the top level of a file the variable is a global. Declaring the same global twice is a compile error.\n' +
            '- Inside a proc it is local to that proc, from this line to the end of the proc. There is no block scope: an `alloc` inside an `if` or a `loop` is still visible after it.\n' +
            '- An `alloc` in a proc with the name of a global makes a separate local that hides the global.',
        example: 'alloc count := 0\ncount := count + 1\nemit count                    # → 1'
    },
    {
        word: 'proc',
        syntax: 'proc name(a, b) { ... }',
        summary: 'Defines a function.',
        details:
            'Parameters are plain names: no types, no defaults, and a call must pass exactly as many arguments. A proc that ends without `return` gives back nothing.\n\n' +
            'Procs are values: pass them around by name. A proc written inside another proc is a closure and shares the variables the outer proc had declared *above* it.',
        example: 'proc make_counter() {\n    alloc n := 0\n    proc tick() {\n        n := n + 1\n        return n\n    }\n    return tick\n}\n\nalloc next := make_counter()\nemit next()                   # → 1\nemit next()                   # → 2'
    },
    {
        word: 'return',
        syntax: 'return value',
        summary: 'Leaves the proc and hands a value back to the caller.',
        details:
            'The value is required: to leave early with nothing to say, write `return false`. Only allowed inside a proc.\n\n' +
            '`return f(x)` to a top-level proc is a true tail call, so recursion written that way can go as deep as it likes.',
        example: 'proc sign(n) {\n    if n < 0 {\n        return -1\n    }\n    return 1\n}\nemit sign(-5)                 # → -1'
    },
    {
        word: 'if',
        syntax: 'if condition { ... } elif condition { ... } else { ... }',
        summary: 'Runs a block when a condition is truthy.',
        details:
            'Falsy values are `false`, `0`, `0.0`, `""`, `[]`, `{}` and the nothing a proc returns when it has no `return`. Everything else is truthy.\n\n' +
            'The `{` goes on the same line as the condition, and `elif` / `else` on the same line as the `}` before them.',
        example: 'alloc score := 75\nif score >= 90 {\n    emit "A"\n} elif score >= 70 {\n    emit "C"                  # → C\n} else {\n    emit "F"\n}'
    },
    {
        word: 'elif',
        syntax: '} elif condition { ... }',
        summary: 'Another condition to try when the ones before it were falsy.',
        details: 'Must follow the closing `}` on the same line. OMG has no `else if`.',
        example: 'alloc n := 0\nif n < 0 {\n    emit "negative"\n} elif n == 0 {\n    emit "zero"               # → zero\n} else {\n    emit "positive"\n}'
    },
    {
        word: 'else',
        syntax: '} else { ... }',
        summary: 'The block to run when no condition before it held.',
        details: 'Must follow the closing `}` on the same line.',
        example: 'if length(args) > 5 {\n    emit "many"\n} else {\n    emit "few"                # → few\n}'
    },
    {
        word: 'loop',
        syntax: 'loop condition { ... }',
        summary: 'Repeats a block while a condition is truthy.',
        details: 'The `while` of other languages. `loop true { ... }` runs until a `break` or a `return`.',
        example: 'alloc i := 0\nloop i < 3 {\n    i := i + 1\n}\nemit i                        # → 3'
    },
    {
        word: 'for',
        syntax: 'for item in sequence { ... }',
        summary: 'Walks the items of a list or the characters of a string.',
        details:
            'The sequence is evaluated once, before the first turn. The loop variable is declared for you and, like any `alloc`, stays visible after the loop.\n\nA dict cannot be walked directly: walk its keys with `for k in dict_keys(d) { ... }`. For a count, use `loop` with a counter.',
        example: 'alloc total := 0\nfor n in [1, 2, 3] {\n    total := total + n\n}\nemit total                    # → 6'
    },
    {
        word: 'in',
        syntax: 'for item in sequence { ... }',
        summary: 'Separates the loop variable from the sequence in a `for` loop.',
        details: '`in` is not an operator. To test membership, use `has_key(d, k)` for a dict, or a loop for a list.'
    },
    {
        word: 'break',
        syntax: 'break',
        summary: 'Leaves the innermost `loop` or `for` at once.',
        example: 'alloc n := 1\nloop true {\n    if n > 100 {\n        break\n    }\n    n := n * 2\n}\nemit n                        # → 128'
    },
    {
        word: 'continue',
        syntax: 'continue',
        summary: 'Skips the rest of the body and goes on with the next turn of the loop.',
        details: 'In a `loop` that means back to the condition, so move the counter on first.',
        example: 'alloc odd := []\nfor n in [1, 2, 3, 4, 5] {\n    if n % 2 == 0 {\n        continue\n    }\n    append(odd, n)\n}\nemit odd                      # → [1, 3, 5]'
    },
    {
        word: 'emit',
        syntax: 'emit value',
        summary: 'Prints a value and ends the line.',
        details: 'Takes one expression of any kind. Join pieces with `+`, which turns the other side into text when one side is a string. For output without a newline use `print(text)`.',
        example: 'emit "total: " + 42            # → total: 42\nemit [1, "two", 3.0]           # → [1, two, 3.0]'
    },
    {
        word: 'facts',
        syntax: 'facts condition',
        summary: 'Asserts that a condition holds.',
        details: 'Nothing happens when it is truthy. Otherwise it raises an AssertionError, which stops the program unless a `try` catches it. The staple of test files.',
        example: 'facts 1 + 1 == 2\ntry {\n    facts 1 + 1 == 3\n} except err {\n    emit err                  # → AssertionError: assertion failed\n}'
    },
    {
        word: 'try',
        syntax: 'try { ... } except err { ... }',
        summary: 'Runs a block, and hands any error raised inside it to the `except` block.',
        details:
            'The error arrives as a string, `"Kind: message"`. The kinds are RuntimeError (from `panic` and `raise`), TypeError, ValueError, IndexError, KeyError, ZeroDivisionError, AssertionError, UndefinedIdentError, FrozenWriteError and ModuleImportError. The wording after the kind is not part of the language: test for the kind.',
        example: 'try {\n    alloc xs := [1, 2, 3]\n    emit xs[99]\n} except err {\n    emit "caught: " + err[0:10]   # → caught: IndexError\n}'
    },
    {
        word: 'except',
        syntax: '} except err { ... }',
        summary: 'The block that runs when the `try` block before it raised an error.',
        details: 'The name after `except` receives the error as a string, `"Kind: message"`. Leave the name out (`} except { ... }`) when you do not need it.',
        example: 'try {\n    panic("boom")\n} except err {\n    emit err                  # → RuntimeError: boom\n}'
    },
    {
        word: 'import',
        syntax: 'import "path/to/file.omg" as name',
        summary: 'Loads another file and binds its top-level procs and variables under a name.',
        details:
            'The path is relative to the file doing the importing. Use the module with a dot: `name.some_proc(1)`.\n\n' +
            '- A module runs once, however many files import it.\n' +
            '- The alias is read-only.\n' +
            '- A variable is copied when the import runs: later changes inside the module do not show through `name.value`. Export a proc that returns it instead.',
        example: 'import "mathlib.omg" as math\n\nemit math.square(5)',
        runnable: false
    },
    {
        word: 'as',
        syntax: 'import "file.omg" as name',
        summary: 'Gives an imported module its name in this file.'
    },
    {
        word: 'and',
        syntax: 'a and b',
        summary: 'True when both sides are truthy.',
        details: 'The right side is not evaluated when the left is falsy. The result is always a bool, never one of the operands.',
        example: 'emit 1 < 2 and 2 < 3          # → true\nemit "x" and 0                # → false'
    },
    {
        word: 'or',
        syntax: 'a or b',
        summary: 'True when either side is truthy.',
        details: 'The right side is not evaluated when the left is truthy. The result is always a bool, never one of the operands.',
        example: 'emit 1 > 2 or 2 < 3           # → true\nemit "" or 0                  # → false'
    },
    {
        word: 'not',
        syntax: 'not a',
        summary: 'True when the value is falsy.',
        details: 'It binds looser than a comparison and tighter than `and`: `not a == b and c` reads as `(not (a == b)) and c`.',
        example: 'emit not true                 # → false\nemit not 5 > 3                # → false\nemit not []                   # → true'
    },
    {
        word: 'true',
        syntax: 'true',
        summary: 'The bool true.',
        details: 'In arithmetic it counts as 1.'
    },
    {
        word: 'false',
        syntax: 'false',
        summary: 'The bool false.',
        details: 'Also what several builtins return to say "nothing there" (`stdin_readline` at the end of input, a non-blocking `tcp_read`). Test for it with `== false`: an empty list or string is falsy too, but is not equal to `false`.'
    }
];

const KEYWORD_BY_WORD = new Map<string, KeywordDoc>();
for (const k of KEYWORD_DOCS) KEYWORD_BY_WORD.set(k.word, k);

export function keywordDoc(word: string): KeywordDoc | undefined {
    return KEYWORD_BY_WORD.get(word);
}

export interface OperatorDoc {
    symbol: string;
    name: string;
    summary: string;
    example?: string;
}

/** Binding strength, loosest first: the order `parse_expr` descends in. */
export const PRECEDENCE: readonly string[] = [
    'or', 'and', 'not', '== != < <= > >=', '|', '^', '&', '<< >>', '+ -', '* / // %',
    'unary - + ~', 'call, index, slice, .name'
];

export const OPERATOR_DOCS: Record<string, OperatorDoc> = {
    ASSIGN: {
        symbol: ':=',
        name: 'Assignment',
        summary: 'Gives a variable, a dict entry or a list slot a new value. After `alloc` it declares the variable; on its own the name must already exist, otherwise the program stops with an UndefinedIdentError.',
        example: 'alloc xs := [1, 2]\nxs[0] := 9\nalloc d := {}\nd.name := "omg"\nemit xs                       # → [9, 2]'
    },
    PLUS: {
        symbol: '+',
        name: 'Add, or join',
        summary: 'Adds numbers. With a string on either side it joins, turning the other side into text. With two lists it makes a new list of both.',
        example: 'emit 1 + 2                    # → 3\nemit "n = " + 5               # → n = 5\nemit [1] + [2]                # → [1, 2]'
    },
    MINUS: {
        symbol: '-',
        name: 'Subtract, or negate',
        summary: 'Subtracts numbers; in front of a value, negates it. An int result that does not fit in 64 bits raises a ValueError.',
        example: 'emit 10 - 3                   # → 7'
    },
    STAR: {
        symbol: '*',
        name: 'Multiply',
        summary: 'Multiplies numbers. There is no `**`: use `pow(a, b)`.',
        example: 'emit 4 * 5                    # → 20'
    },
    SLASH: {
        symbol: '/',
        name: 'Divide',
        summary: 'True division: the result is always a float. Use `//` to stay in ints. Dividing by zero raises a ZeroDivisionError.',
        example: 'emit 10 / 4                   # → 2.5'
    },
    DSLASH: {
        symbol: '//',
        name: 'Floor divide',
        summary: 'Divides and rounds down, towards minus infinity. Two ints give an int.',
        example: 'emit 10 // 3                  # → 3\nemit -7 // 2                  # → -4'
    },
    PERCENT: {
        symbol: '%',
        name: 'Remainder',
        summary: 'What is left over after `//`. The result has the sign of the divisor.',
        example: 'emit 10 % 3                   # → 1\nemit 7 % -2                   # → -1'
    },
    EQ: {
        symbol: '==',
        name: 'Equal',
        summary: 'Compares by value: lists and dicts are equal when their contents are, and `5 == 5.0` is true.',
        example: 'emit [1, 2] == [1, 2]         # → true'
    },
    NE: {
        symbol: '!=',
        name: 'Not equal',
        summary: 'The opposite of `==`.',
        example: 'emit 1 != 2                   # → true'
    },
    LT: { symbol: '<', name: 'Less than', summary: 'Orders two numbers, or two strings alphabetically.', example: 'emit "abc" < "abd"            # → true' },
    LE: { symbol: '<=', name: 'Less than or equal', summary: 'Orders two numbers, or two strings alphabetically.' },
    GT: { symbol: '>', name: 'Greater than', summary: 'Orders two numbers, or two strings alphabetically.' },
    GE: { symbol: '>=', name: 'Greater than or equal', summary: 'Orders two numbers, or two strings alphabetically.' },
    AMP: { symbol: '&', name: 'Bitwise AND', summary: 'Ints only: a float raises a TypeError. For logic, use `and`.', example: 'emit 6 & 3                    # → 2' },
    PIPE: { symbol: '|', name: 'Bitwise OR', summary: 'Ints only: a float raises a TypeError. For logic, use `or`.', example: 'emit 6 | 3                    # → 7' },
    CARET: { symbol: '^', name: 'Bitwise XOR', summary: 'Ints only. This is not a power operator: use `pow(a, b)`.', example: 'emit 6 ^ 3                    # → 5' },
    TILDE: { symbol: '~', name: 'Bitwise NOT', summary: 'Flips every bit of an int. For logic, use `not`.', example: 'emit ~1                       # → -2' },
    SHL: { symbol: '<<', name: 'Shift left', summary: 'Shifts the bits of an int to the left.', example: 'emit 1 << 3                   # → 8' },
    SHR: { symbol: '>>', name: 'Shift right', summary: 'Shifts the bits of an int to the right.', example: 'emit 16 >> 2                  # → 4' },
    DOT: {
        symbol: '.',
        name: 'Member access',
        summary: '`d.name` is the same as `d["name"]`: it reads a dict entry, or a proc or variable of an imported module. A missing key raises a KeyError.',
        example: 'alloc person := {name: "Ada"}\nemit person.name              # → Ada'
    },
    COLON: {
        symbol: ':',
        name: 'Key separator, or slice',
        summary: 'Separates a key from its value in a dict literal, and the start from the end in a slice: `xs[a:b]` takes items `a` up to, not including, `b`; `xs[a:]` runs to the end. A slice needs a start, and has no step.',
        example: 'emit "hello"[1:4]             # → ell\nemit [1, 2, 3, 4][2:]         # → [3, 4]'
    }
};

export const HEADER_DOC =
    '**`;;;omg`** file header\n\n' +
    'Marks a file as OMG source. It goes on the first line that is not blank. ' +
    'It is a convention: the compiler accepts a file without it.';

/**
 * Names from other languages that OMG reads as ordinary identifiers.
 * Written as a statement they do nothing, so say what to write instead.
 */
export const FOREIGN_WORDS: Record<string, string> = {
    while: 'OMG spells this `loop condition { ... }`.',
    until: 'OMG spells this `loop not condition { ... }`.',
    foreach: 'OMG spells this `for item in sequence { ... }`.',
    def: 'A function is written `proc name(args) { ... }`.',
    fn: 'A function is written `proc name(args) { ... }`.',
    func: 'A function is written `proc name(args) { ... }`.',
    fun: 'A function is written `proc name(args) { ... }`.',
    function: 'A function is written `proc name(args) { ... }`.',
    var: 'A variable is declared with `alloc name := value`.',
    let: 'A variable is declared with `alloc name := value`.',
    const: 'A variable is declared with `alloc name := value`.',
    elseif: 'OMG spells this `elif`.',
    elsif: 'OMG spells this `elif`.',
    switch: 'OMG has no `switch`. Use `if` / `elif`.',
    match: 'OMG has no `match`. Use `if` / `elif`.',
    case: 'OMG has no `case`. Use `if` / `elif`.',
    throw: 'Raise an error with `panic(message)`.',
    catch: 'Errors are caught with `try { ... } except err { ... }`.',
    finally: 'OMG has no `finally`.',
    assert: 'An assertion is written `facts condition`.',
    echo: 'Print with `emit value`.',
    puts: 'Print with `emit value`.',
    println: 'Print with `emit value`.',
    class: 'OMG has no classes. Return `freeze({name: proc_name})` from a proc to make an object out of closures.',
    struct: 'OMG has no structs. Use a dict: `{x: 1, y: 2}`.',
    pass: 'OMG has no `pass`: an empty block `{ }` is fine.',
    end: 'Blocks end with `}`.',
    then: 'Blocks are wrapped in `{ }`.',
    do: 'Blocks are wrapped in `{ }`.',
    null: 'OMG has no `null`. Use `false`.',
    nil: 'OMG has no `nil`. Use `false`.',
    None: 'OMG has no `None` literal. Use `false`.',
    undefined: 'OMG has no `undefined`. Use `false`.',
    True: 'Booleans are lowercase: `true`.',
    False: 'Booleans are lowercase: `false`.'
};
