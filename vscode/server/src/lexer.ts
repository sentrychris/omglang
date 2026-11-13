// The OMG lexer, ported from `tokenize` in src/compiler.omg.
//
// It makes the same tokens the compiler makes, and reports the same
// errors in the same words. The differences are the ones an editor
// needs: every token knows where it starts and ends, comments are kept,
// and an error does not stop the scan.

export type TokKind =
    | 'NUM' | 'FNUM' | 'STR' | 'ID'
    | 'TRUE' | 'FALSE' | 'IF' | 'ELIF' | 'ELSE' | 'LOOP' | 'BREAK'
    | 'FOR' | 'IN' | 'CONTINUE' | 'NOT' | 'EMIT' | 'IMPORT' | 'AS'
    | 'FACTS' | 'FUNC' | 'RETURN' | 'AND' | 'OR' | 'ALLOC' | 'TRY' | 'EXCEPT'
    | 'ASSIGN' | 'LBRACE' | 'RBRACE' | 'LPAREN' | 'RPAREN' | 'LBRACK' | 'RBRACK'
    | 'COMMA' | 'DOT' | 'COLON' | 'PLUS' | 'MINUS' | 'STAR' | 'PERCENT'
    | 'SLASH' | 'DSLASH' | 'SHL' | 'SHR' | 'AMP' | 'PIPE' | 'CARET' | 'TILDE'
    | 'GE' | 'LE' | 'EQ' | 'NE' | 'GT' | 'LT' | 'NL' | 'EOF';

export interface Token {
    kind: TokKind;
    /** Source text of the token. */
    text: string;
    /** Decoded value of a string literal; the integer of a NUM. */
    value?: string | bigint;
    start: number;
    end: number;
    /** 1-based line, counted the way the compiler counts it. */
    line: number;
}

export interface Comment {
    start: number;
    end: number;
    block: boolean;
    /** 0-based line of the first and last character. */
    startLine: number;
    endLine: number;
}

export interface SyntaxIssue {
    /** The compiler's message, without the "on line N in file" tail. */
    message: string;
    /** A line of advice for the editor; the compiler does not print it. */
    hint?: string;
    start: number;
    end: number;
    /** The line the compiler would name. */
    line: number;
    severity?: 'error' | 'warning';
}

export interface LexResult {
    tokens: Token[];
    comments: Comment[];
    errors: SyntaxIssue[];
    warnings: SyntaxIssue[];
    /** Offset of the first character of each line. */
    lineStarts: number[];
    /** Range of the `;;;omg` header line, when there is one. */
    header?: { start: number; end: number };
}

const KEYWORD_KINDS: Record<string, TokKind> = {
    if: 'IF', elif: 'ELIF', else: 'ELSE', loop: 'LOOP', break: 'BREAK',
    for: 'FOR', in: 'IN', continue: 'CONTINUE', not: 'NOT', emit: 'EMIT',
    import: 'IMPORT', as: 'AS', facts: 'FACTS', proc: 'FUNC',
    return: 'RETURN', and: 'AND', or: 'OR', alloc: 'ALLOC', try: 'TRY',
    except: 'EXCEPT', true: 'TRUE', false: 'FALSE'
};

/** Every reserved word, in the order the compiler lists them. */
export const KEYWORD_LIST: readonly string[] = Object.keys(KEYWORD_KINDS);

export function isKeyword(word: string): boolean {
    return Object.prototype.hasOwnProperty.call(KEYWORD_KINDS, word);
}

const SINGLE: Record<string, TokKind> = {
    '{': 'LBRACE', '}': 'RBRACE', '(': 'LPAREN', ')': 'RPAREN',
    '[': 'LBRACK', ']': 'RBRACK', ',': 'COMMA', '.': 'DOT', ':': 'COLON',
    '+': 'PLUS', '-': 'MINUS', '*': 'STAR', '/': 'SLASH', '%': 'PERCENT',
    '&': 'AMP', '|': 'PIPE', '^': 'CARET', '~': 'TILDE', '<': 'LT', '>': 'GT'
};

const DOUBLE: Record<string, TokKind> = {
    ':=': 'ASSIGN', '==': 'EQ', '!=': 'NE', '<=': 'LE', '>=': 'GE',
    '<<': 'SHL', '>>': 'SHR', '//': 'DSLASH'
};

const I64_MAX = 9223372036854775807n;

function isDigit(c: number): boolean {
    return c >= 48 && c <= 57;
}

function isAlpha(c: number): boolean {
    return (c >= 97 && c <= 122) || (c >= 65 && c <= 90) || c === 95;
}

function isAlnum(c: number): boolean {
    return isAlpha(c) || isDigit(c);
}

/** What a backslash escape stands for. Unknown ones keep the backslash. */
function decodeEscape(c: string): string {
    switch (c) {
        case 'n': return '\n';
        case 't': return '\t';
        case 'r': return '\r';
        case '\\': return '\\';
        case '"': return '"';
        case '0': return '\0';
        case "'": return "'";
        default: return '\\' + c;
    }
}

export const KNOWN_ESCAPES = 'ntr\\"0\'';

function unexpectedCharHint(src: string, i: number): string | undefined {
    const c = src[i];
    const prev = i > 0 ? src[i - 1] : '';
    switch (c) {
        case '!':
            return 'OMG has no `!`. Write `not x`, or `!=` to compare.';
        case "'":
            return 'Strings use double quotes: "like this".';
        case ';':
            return 'Statements are not ended with `;`. Just start the next one.';
        case '=':
            if ('+-*/%&|^'.includes(prev) && prev !== '') {
                return 'OMG has no compound assignment. Write `x := x ' + prev + ' 1`.';
            }
            return 'Use `:=` to assign and `==` to compare.';
        case '?':
            return 'OMG has no `?:`. Use `if` / `else`.';
        case '@':
        case '$':
        case '`':
        case '\\':
            return undefined;
        default:
            return undefined;
    }
}

export function tokenize(src: string): LexResult {
    const n = src.length;
    const tokens: Token[] = [];
    const comments: Comment[] = [];
    const errors: SyntaxIssue[] = [];
    const warnings: SyntaxIssue[] = [];

    const lineStarts: number[] = [0];
    for (let k = 0; k < n; k++) {
        if (src.charCodeAt(k) === 10) lineStarts.push(k + 1);
    }

    let i = 0;
    let line = 1;
    let header: LexResult['header'];

    // The `;;;omg` header: optional, and only on the first non-blank line.
    {
        let j = 0;
        let l = 1;
        while (j < n) {
            const c = src.charCodeAt(j);
            if (c === 10) { l++; j++; } else if (c === 32 || c === 9 || c === 13) { j++; } else { break; }
        }
        if (src.startsWith(';;;omg', j)) {
            const hs = j;
            j += 6;
            while (j < n && src.charCodeAt(j) !== 10) j++;
            header = { start: hs, end: j };
            if (j < n) { j++; l++; }
            i = j;
            line = l;
        }
    }

    const push = (kind: TokKind, start: number, end: number, value?: string | bigint): void => {
        tokens.push({ kind, text: src.slice(start, end), value, start, end, line });
    };

    while (i < n) {
        const c = src.charCodeAt(i);
        const d = i + 1 < n ? src.charCodeAt(i + 1) : -1;

        if (c === 32 || c === 9 || c === 13) {
            i++;
        } else if (c === 10) {
            push('NL', i, i + 1);
            line++;
            i++;
        } else if (c === 35) {
            const start = i;
            while (i < n && src.charCodeAt(i) !== 10) i++;
            comments.push({ start, end: i, block: false, startLine: line - 1, endLine: line - 1 });
        } else if (c === 47 && d === 42) {
            const start = i;
            const startLine = line;
            i += 2;
            while (i + 1 < n && (src.charCodeAt(i) !== 42 || src.charCodeAt(i + 1) !== 47)) {
                if (src.charCodeAt(i) === 10) line++;
                i++;
            }
            if (i + 1 < n) {
                i += 2;
            } else {
                if (i < n && src.charCodeAt(i) === 10) line++;
                i = n;
                errors.push({
                    message: 'Unterminated block comment starting',
                    hint: 'Close it with `*/`.',
                    start,
                    end: start + 2,
                    line: startLine
                });
            }
            comments.push({ start, end: i, block: true, startLine: startLine - 1, endLine: line - 1 });
        } else if (c === 48 && (d === 98 || d === 66)) {
            const start = i;
            i += 2;
            const binStart = i;
            let sig = 0;
            let tooLarge = false;
            while (i < n && (src.charCodeAt(i) === 48 || src.charCodeAt(i) === 49)) {
                if (sig > 0 || src.charCodeAt(i) === 49) sig++;
                if (sig > 63) tooLarge = true;
                i++;
            }
            if (tooLarge) {
                errors.push({
                    message: 'Invalid binary literal',
                    hint: 'An integer holds 64 bits with a sign, so at most 63 binary digits.',
                    start, end: i, line
                });
                push('NUM', start, i, 0n);
            } else if (i === binStart) {
                errors.push({ message: 'Empty binary literal', hint: 'Write the bits after `0b`, as in `0b1010`.', start, end: i, line });
                push('NUM', start, i, 0n);
            } else {
                push('NUM', start, i, BigInt('0b' + src.slice(binStart, i)));
            }
            noteGluedName(start);
        } else if (isDigit(c)) {
            const start = i;
            while (i < n && isDigit(src.charCodeAt(i))) i++;
            const intEnd = i;
            const floatDot = i + 1 < n && src.charCodeAt(i) === 46 && isDigit(src.charCodeAt(i + 1));
            const floatExp = i < n && (src.charCodeAt(i) === 101 || src.charCodeAt(i) === 69);
            if (floatDot || floatExp) {
                let bad = false;
                if (floatDot) {
                    i++;
                    while (i < n && isDigit(src.charCodeAt(i))) i++;
                }
                if (i < n && (src.charCodeAt(i) === 101 || src.charCodeAt(i) === 69)) {
                    i++;
                    if (i < n && (src.charCodeAt(i) === 43 || src.charCodeAt(i) === 45)) i++;
                    const expStart = i;
                    while (i < n && isDigit(src.charCodeAt(i))) i++;
                    if (expStart === i) {
                        bad = true;
                        errors.push({
                            message: "Float exponent has no digits in '" + src.slice(start, i) + "'",
                            hint: 'A number directly followed by `e` is read as a float with an exponent, as in `1e3`.',
                            start, end: i, line
                        });
                    }
                }
                push('FNUM', start, i, src.slice(start, i));
                if (!bad) noteGluedName(start);
            } else {
                const text = src.slice(start, intEnd);
                const value = BigInt(text);
                if (value > I64_MAX) {
                    errors.push({
                        message: "Invalid integer literal '" + text + "'",
                        hint: 'The largest integer is 9223372036854775807.',
                        start, end: intEnd, line
                    });
                    push('NUM', start, intEnd, 0n);
                } else {
                    push('NUM', start, intEnd, value);
                }
                noteGluedName(start);
            }
        } else if (c === 34) {
            const start = i;
            const startLine = line;
            i++;
            let out = '';
            while (i < n && src.charCodeAt(i) !== 34) {
                if (src.charCodeAt(i) === 92 && i + 1 < n) {
                    out += decodeEscape(src[i + 1]);
                    i += 2;
                } else {
                    if (src.charCodeAt(i) === 10) line++;
                    out += src[i];
                    i++;
                }
            }
            if (i >= n) {
                // The compiler names the line it stopped on, which is the
                // last one; the squiggle goes where the string opens.
                errors.push({
                    message: 'Unterminated string starting',
                    hint: 'This string is opened on line ' + startLine + ' and never closed.',
                    start,
                    end: Math.min(n, start + 1),
                    line
                });
                tokens.push({ kind: 'STR', text: src.slice(start, n), value: out, start, end: n, line });
            } else {
                i++;
                tokens.push({ kind: 'STR', text: src.slice(start, i), value: out, start, end: i, line });
            }
        } else if (isAlpha(c)) {
            const start = i;
            while (i < n && isAlnum(src.charCodeAt(i))) i++;
            const word = src.slice(start, i);
            const kind = KEYWORD_KINDS[word];
            push(Object.prototype.hasOwnProperty.call(KEYWORD_KINDS, word) ? kind : 'ID', start, i, word);
        } else {
            const two = src.substr(i, 2);
            if (Object.prototype.hasOwnProperty.call(DOUBLE, two)) {
                push(DOUBLE[two], i, i + 2);
                i += 2;
            } else {
                const one = src[i];
                if (Object.prototype.hasOwnProperty.call(SINGLE, one)) {
                    push(SINGLE[one], i, i + 1);
                    i++;
                } else {
                    const cp = src.codePointAt(i) ?? c;
                    const ch = String.fromCodePoint(cp);
                    errors.push({
                        message: "Unexpected character '" + ch + "'",
                        hint: unexpectedCharHint(src, i),
                        start: i,
                        end: i + ch.length,
                        line
                    });
                    i += ch.length;
                }
            }
        }
    }
    tokens.push({ kind: 'EOF', text: '', start: n, end: n, line });

    return { tokens, comments, errors, warnings, lineStarts, header };

    // `0xff` is not a hex literal: it is the number 0 and then the name
    // `xff`, which compiles and then fails at run time. Say so early.
    function noteGluedName(start: number): void {
        if (i >= n || !isAlpha(src.charCodeAt(i))) return;
        let j = i;
        while (j < n && isAlnum(src.charCodeAt(j))) j++;
        const num = src.slice(start, i);
        const word = src.slice(i, j);
        if (isKeyword(word)) return;
        let hint = 'Put a space or an operator between them.';
        if (num === '0' && /^[xX][0-9a-fA-F]+$/.test(word)) {
            hint = 'OMG has no hex literals. Write ' + parseInt(word.slice(1), 16) + ', or use a binary literal such as 0b1010.';
        } else if (num === '0' && /^[oO][0-7]+$/.test(word)) {
            hint = 'OMG has no octal literals. Write ' + parseInt(word.slice(1), 8) + '.';
        }
        warnings.push({
            message: "'" + num + word + "' is read as the number " + num + " followed by the name '" + word + "'",
            hint,
            start,
            end: j,
            line,
            severity: 'warning'
        });
    }
}

/** 0-based line and column of an offset. */
export function positionAt(lineStarts: number[], offset: number): { line: number; character: number } {
    let lo = 0;
    let hi = lineStarts.length - 1;
    while (lo < hi) {
        const mid = (lo + hi + 1) >> 1;
        if (lineStarts[mid] <= offset) lo = mid; else hi = mid - 1;
    }
    return { line: lo, character: offset - lineStarts[lo] };
}

export function offsetAt(lineStarts: number[], srcLength: number, line: number, character: number): number {
    if (line < 0) return 0;
    if (line >= lineStarts.length) return srcLength;
    const start = lineStarts[line];
    const end = line + 1 < lineStarts.length ? lineStarts[line + 1] : srcLength;
    return Math.max(start, Math.min(start + character, end));
}
