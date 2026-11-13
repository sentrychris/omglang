// The OMG parser, ported from `parse_program` and friends in
// src/compiler.omg. Same grammar, same error messages.
//
// The compiler stops at the first error. An editor is looking at a file
// that is wrong most of the time (someone is typing in it), so this one
// notes the error, skips to the next line and carries on, to keep the
// rest of the file understood. Only the first error is the compiler's;
// the ones after it are kept apart, since they are often its echo.

import {
    Block, CallExpr, DictPair, Expr, FuncStmt, IfStmt, Name, Stmt
} from './ast';
import { LexResult, SyntaxIssue, TokKind, Token } from './lexer';

const DESCRIPTIONS: Partial<Record<TokKind, string>> = {
    NUM: 'number', FNUM: 'float', STR: 'string', ID: 'identifier',
    TRUE: 'true', FALSE: 'false', IF: 'if', ELIF: 'elif', ELSE: 'else',
    LOOP: 'loop', BREAK: 'break', EMIT: 'emit', IMPORT: 'import',
    FOR: 'for', IN: 'in', CONTINUE: 'continue', NOT: 'not',
    AS: 'as', FACTS: 'facts', FUNC: 'proc', RETURN: 'return',
    AND: 'and', OR: 'or', ALLOC: 'alloc', TRY: 'try', EXCEPT: 'except',
    ASSIGN: ':=', LBRACE: '{', RBRACE: '}', LPAREN: '(', RPAREN: ')',
    LBRACK: '[', RBRACK: ']', COMMA: ',', DOT: '.', COLON: ':',
    PLUS: '+', MINUS: '-', STAR: '*', PERCENT: '%', SLASH: '/',
    DSLASH: '//', SHL: '<<', SHR: '>>', AMP: '&', PIPE: '|',
    CARET: '^', TILDE: '~', GE: '>=', LE: '<=', EQ: '==', NE: '!=',
    GT: '>', LT: '<', NL: 'newline', EOF: '<eof>'
};

export function describeToken(kind: TokKind): string {
    return DESCRIPTIONS[kind] ?? kind;
}

class ParseFail {
    constructor(public message: string, public index: number, public hint?: string) {}
}

export interface ParseResult {
    stmts: Stmt[];
    /** Every syntax error met, in source order. The first is the compiler's. */
    errors: SyntaxIssue[];
}

const CMP_OPS: Partial<Record<TokKind, string>> = {
    EQ: 'eq', NE: 'ne', LT: 'lt', LE: 'le', GT: 'gt', GE: 'ge'
};
const SHIFT_OPS: Partial<Record<TokKind, string>> = { SHL: 'shl', SHR: 'shr' };
const ADD_OPS: Partial<Record<TokKind, string>> = { PLUS: 'add', MINUS: 'sub' };
const MUL_OPS: Partial<Record<TokKind, string>> = {
    STAR: 'mul', SLASH: 'div', DSLASH: 'floor_div', PERCENT: 'mod'
};

type R<T> = [T, number];

export function parse(lex: LexResult): ParseResult {
    const toks = lex.tokens;
    const errors: SyntaxIssue[] = [];
    // `{` that an error made us step over: their `}` will turn up later
    // and must not close a block they never opened.
    let strayCloses = 0;

    const kind = (i: number): TokKind => toks[Math.min(i, toks.length - 1)].kind;
    const tok = (i: number): Token => toks[Math.min(i, toks.length - 1)];

    function fail(message: string, i: number, hint?: string): never {
        throw new ParseFail(message, i, hint);
    }

    function skipNewlines(i: number): number {
        while (kind(i) === 'NL') i++;
        return i;
    }

    function expect(k: TokKind, i: number): number {
        if (kind(i) !== k) {
            fail("Expected '" + describeToken(k) + "' but got '" + describeToken(kind(i)) + "'", i, expectHint(k, i));
        }
        return i + 1;
    }

    function expectHint(want: TokKind, i: number): string | undefined {
        const got = kind(i);
        if (want === 'LBRACE') {
            if (got === 'IF' && i > 0 && kind(i - 1) === 'ELSE') return 'OMG spells it `elif`.';
            if (got === 'NL') return 'The `{` has to be on the same line.';
            if (got === 'COLON') return 'Blocks are wrapped in `{ }`, not opened with `:`.';
        }
        if (want === 'RBRACK' && got !== 'COLON') return 'Separate list elements with commas.';
        if (want === 'RBRACK' && got === 'COLON') return 'Slices take a start and an optional end: `xs[a:b]`. There is no step.';
        if (want === 'RBRACE') return 'Separate dict entries with commas.';
        if (want === 'RPAREN' && got !== 'NL' && got !== 'EOF') return 'Separate arguments with commas.';
        if (want === 'ASSIGN') return 'Declare a variable with `alloc name := value`.';
        if (want === 'EXCEPT') return 'A `try` block needs an `except` block right after its `}`.';
        if (want === 'AS') return 'Imports are written `import "file.omg" as name`.';
        if (want === 'IN') return 'Write `for item in sequence { ... }`.';
        return undefined;
    }

    function unexpected(i: number): never {
        const k = kind(i);
        let hint: string | undefined;
        const prev = i > 0 ? kind(i - 1) : 'NL';
        if (prev === 'RETURN' && (k === 'NL' || k === 'RBRACE' || k === 'EOF')) {
            hint = '`return` needs a value. Write `return false` to leave a proc early.';
        } else if (k === 'STAR' && prev === 'STAR') {
            hint = 'OMG has no `**`. Use `pow(a, b)`.';
        } else if (k === 'AMP' && prev === 'AMP') {
            hint = 'Use `and`.';
        } else if (k === 'PIPE' && prev === 'PIPE') {
            hint = 'Use `or`.';
        } else if (k === 'COLON' && prev === 'LBRACK') {
            hint = 'A slice needs a start: write `xs[0:n]`.';
        } else if ((k === 'ELSE' || k === 'ELIF') && prevSignificant(i) === 'RBRACE') {
            hint = '`' + describeToken(k) + '` has to follow the closing `}` on the same line: `} ' + describeToken(k) + ' {`.';
        } else if (k === 'ELSE' || k === 'ELIF') {
            hint = '`' + describeToken(k) + '` only follows the `}` of an `if` block.';
        } else if (k === 'EXCEPT') {
            hint = '`except` has to follow the closing `}` of a `try` block on the same line.';
        } else if (k === 'ASSIGN') {
            hint = 'Only a name, `name.key` or `name[index]` can be assigned to.';
        } else if (k === 'RBRACE') {
            hint = 'There is no open `{` for this `}`.';
        } else if (k === 'NUM' || k === 'FNUM' || k === 'STR' || k === 'LPAREN' || k === 'LBRACK' || k === 'MINUS') {
            if (atStatementStart(i)) hint = 'A statement starts with a keyword or a name. To print a value, use `emit`.';
        } else if (k === 'FUNC') {
            hint = 'OMG has no anonymous procs. Define the proc with a name, then pass the name.';
        } else if (k === 'EOF') {
            hint = 'The file ends in the middle of something. Check for a missing `}`.';
        }
        fail("Unexpected token '" + describeToken(k) + "'", i, hint);
    }

    function prevSignificant(i: number): TokKind {
        let j = i - 1;
        while (j >= 0 && toks[j].kind === 'NL') j--;
        return j >= 0 ? toks[j].kind : 'NL';
    }

    function atStatementStart(i: number): boolean {
        if (i === 0) return true;
        const p = kind(i - 1);
        return p === 'NL' || p === 'LBRACE' || p === 'RBRACE';
    }

    // ---- Expressions ----------------------------------------------------

    function parseExpr(i: number): R<Expr> {
        return parseOr(i);
    }

    function binary(
        i: number,
        next: (i: number) => R<Expr>,
        opOf: (k: TokKind) => string | undefined
    ): R<Expr> {
        let [left, j] = next(i);
        for (;;) {
            const op = opOf(kind(j));
            if (!op) return [left, j];
            const [right, k] = next(j + 1);
            left = { type: 'bin', op, left, right, start: left.start, end: right.end };
            j = k;
        }
    }

    function parseOr(i: number): R<Expr> {
        return binary(i, parseAnd, (k) => (k === 'OR' ? 'or' : undefined));
    }

    function parseAnd(i: number): R<Expr> {
        return binary(i, parseNot, (k) => (k === 'AND' ? 'and' : undefined));
    }

    function parseNot(i: number): R<Expr> {
        if (kind(i) === 'NOT') {
            const [operand, j] = parseNot(i + 1);
            return [{ type: 'unary', op: 'not', operand, start: tok(i).start, end: operand.end }, j];
        }
        return parseCmp(i);
    }

    function parseCmp(i: number): R<Expr> {
        return binary(i, parseBor, (k) => CMP_OPS[k]);
    }

    function parseBor(i: number): R<Expr> {
        return binary(i, parseBxor, (k) => (k === 'PIPE' ? 'bor' : undefined));
    }

    function parseBxor(i: number): R<Expr> {
        return binary(i, parseBand, (k) => (k === 'CARET' ? 'bxor' : undefined));
    }

    function parseBand(i: number): R<Expr> {
        return binary(i, parseShift, (k) => (k === 'AMP' ? 'band' : undefined));
    }

    function parseShift(i: number): R<Expr> {
        return binary(i, parseAdd, (k) => SHIFT_OPS[k]);
    }

    function parseAdd(i: number): R<Expr> {
        return binary(i, parseMul, (k) => ADD_OPS[k]);
    }

    function parseMul(i: number): R<Expr> {
        return binary(i, parseUnary, (k) => MUL_OPS[k]);
    }

    function parseUnary(i: number): R<Expr> {
        const k = kind(i);
        if (k === 'TILDE' || k === 'MINUS' || k === 'PLUS') {
            const [operand, j] = parseUnary(i + 1);
            const op = k === 'TILDE' ? 'bnot' : k === 'MINUS' ? 'neg' : 'plus';
            return [{ type: 'unary', op, operand, start: tok(i).start, end: operand.end }, j];
        }
        return parsePostfix(i);
    }

    function parsePostfix(i: number): R<Expr> {
        let [node, j] = parsePrimary(i);
        for (;;) {
            const k = kind(j);
            if (k === 'LPAREN') {
                const lparen = tok(j);
                let p = skipNewlines(j + 1);
                const args: Expr[] = [];
                if (kind(p) !== 'RPAREN') {
                    let a = parseExpr(p);
                    args.push(a[0]);
                    p = skipNewlines(a[1]);
                    while (kind(p) === 'COMMA') {
                        p = skipNewlines(p + 1);
                        a = parseExpr(p);
                        args.push(a[0]);
                        p = skipNewlines(a[1]);
                    }
                }
                p = expect('RPAREN', p);
                const call: CallExpr = {
                    type: 'call', callee: node, args,
                    lparen: lparen.start, rparen: tok(p - 1).end, line: lparen.line,
                    start: node.start, end: tok(p - 1).end
                };
                node = call;
                j = p;
            } else if (k === 'LBRACK') {
                const from = parseExpr(j + 1);
                if (kind(from[1]) === 'COLON') {
                    let e = from[1] + 1;
                    if (kind(e) === 'RBRACK') {
                        node = { type: 'slice', obj: node, from: from[0], to: null, start: node.start, end: tok(e).end };
                        j = e + 1;
                    } else {
                        const to = parseExpr(e);
                        e = expect('RBRACK', to[1]);
                        node = { type: 'slice', obj: node, from: from[0], to: to[0], start: node.start, end: tok(e - 1).end };
                        j = e;
                    }
                } else {
                    const e = expect('RBRACK', from[1]);
                    node = { type: 'index', obj: node, index: from[0], start: node.start, end: tok(e - 1).end };
                    j = e;
                }
            } else if (k === 'DOT') {
                if (kind(j + 1) !== 'ID') {
                    fail("Expected identifier after '.'", j + 1);
                }
                const a = tok(j + 1);
                node = {
                    type: 'dot', obj: node,
                    attr: { name: a.text, start: a.start, end: a.end },
                    start: node.start, end: a.end
                };
                j += 2;
            } else {
                return [node, j];
            }
        }
    }

    function parsePrimary(i: number): R<Expr> {
        const k = kind(i);
        const t = tok(i);
        switch (k) {
            case 'NUM':
                return [{ type: 'num', float: false, text: t.text, value: t.value as bigint, start: t.start, end: t.end }, i + 1];
            case 'FNUM':
                return [{ type: 'num', float: true, text: t.text, start: t.start, end: t.end }, i + 1];
            case 'STR':
                return [{ type: 'str', value: t.value as string, start: t.start, end: t.end }, i + 1];
            case 'TRUE':
            case 'FALSE':
                return [{ type: 'bool', value: k === 'TRUE', start: t.start, end: t.end }, i + 1];
            case 'ID':
                return [{ type: 'id', name: t.text, start: t.start, end: t.end }, i + 1];
            case 'LPAREN': {
                const [inner, j] = parseExpr(i + 1);
                return [inner, expect('RPAREN', j)];
            }
            case 'LBRACK': {
                let j = skipNewlines(i + 1);
                const elems: Expr[] = [];
                while (kind(j) !== 'RBRACK') {
                    const r = parseExpr(j);
                    elems.push(r[0]);
                    j = skipNewlines(r[1]);
                    if (kind(j) === 'COMMA') {
                        j = skipNewlines(j + 1);
                    } else if (kind(j) !== 'RBRACK') {
                        expect('RBRACK', j);
                    }
                }
                return [{ type: 'list', elems, start: t.start, end: tok(j).end }, j + 1];
            }
            case 'LBRACE': {
                let j = skipNewlines(i + 1);
                const pairs: DictPair[] = [];
                while (kind(j) !== 'RBRACE') {
                    const kt = tok(j);
                    let key: Name;
                    let quoted = false;
                    if (kt.kind === 'STR') {
                        key = { name: kt.value as string, start: kt.start, end: kt.end };
                        quoted = true;
                    } else if (kt.kind === 'ID') {
                        key = { name: kt.text, start: kt.start, end: kt.end };
                    } else {
                        fail('Invalid dict key', j, 'A dict key is a name or a string: `{name: 1, "two words": 2}`.');
                    }
                    j = expect('COLON', j + 1);
                    j = skipNewlines(j);
                    const v = parseExpr(j);
                    pairs.push({ key, quoted, value: v[0] });
                    j = skipNewlines(v[1]);
                    if (kind(j) === 'COMMA') {
                        j = skipNewlines(j + 1);
                    } else if (kind(j) !== 'RBRACE') {
                        expect('RBRACE', j);
                    }
                }
                return [{ type: 'dict', pairs, start: t.start, end: tok(j).end }, j + 1];
            }
            default:
                return unexpected(i);
        }
    }

    // ---- Statements -----------------------------------------------------

    function parseBlock(i: number): R<Block> {
        const open = tok(i);
        let j = expect('LBRACE', i);
        j = skipNewlines(j);
        const stmts: Stmt[] = [];
        for (;;) {
            const k = kind(j);
            if (k === 'RBRACE') {
                if (strayCloses > 0) {
                    strayCloses--;
                    j = skipNewlines(j + 1);
                    continue;
                }
                return [{ stmts, start: open.start, end: tok(j).end, closed: true }, j + 1];
            }
            if (k === 'EOF') {
                note(new ParseFail("Unexpected token '<eof>'", j, 'This `{` is never closed.'), open);
                return [{ stmts, start: open.start, end: tok(j).end, closed: false }, j];
            }
            j = statementInto(stmts, j);
        }
    }

    /** Parse one statement into `out`; on an error, note it and resync. */
    function statementInto(out: Stmt[], i: number): number {
        try {
            const [stmt, j] = parseStmt(i);
            out.push(stmt);
            return skipNewlines(j);
        } catch (e) {
            if (!(e instanceof ParseFail)) throw e;
            note(e);
            return skipNewlines(resync(i, e.index));
        }
    }

    function note(e: ParseFail, at?: Token): void {
        let t = at ?? tok(e.index);
        let start = t.start;
        let end = t.end;
        if (!at && (t.kind === 'NL' || t.kind === 'EOF') && e.index > 0) {
            // Nothing to underline at a line end: point at what came before.
            const p = tok(e.index - 1);
            if (p.kind !== 'NL') {
                start = p.start;
                end = p.end;
            }
        }
        errors.push({ message: e.message, hint: e.hint, start, end, line: tok(e.index).line });
    }

    function resync(stmtStart: number, errIndex: number): number {
        let j = Math.max(errIndex, stmtStart);
        if (j === stmtStart && kind(j) !== 'EOF') {
            if (kind(j) === 'LBRACE') strayCloses++;
            j++;
        }
        let depth = 0;
        for (;;) {
            const k = kind(j);
            if (k === 'EOF') break;
            if (kind(j - 1) === 'NL') break;
            if (k === 'LBRACE') {
                depth++;
            } else if (k === 'RBRACE') {
                if (depth === 0) break;
                depth--;
            }
            j++;
        }
        if (depth > 0) strayCloses += depth;
        return j;
    }

    function parseLvalue(i: number): R<Expr> {
        if (kind(i) !== 'ID') fail('Expected identifier', i);
        const t = tok(i);
        let node: Expr = { type: 'id', name: t.text, start: t.start, end: t.end };
        let j = i + 1;
        for (;;) {
            const k = kind(j);
            if (k === 'DOT') {
                if (kind(j + 1) !== 'ID') fail("Expected identifier after '.'", j + 1);
                const a = tok(j + 1);
                node = {
                    type: 'dot', obj: node,
                    attr: { name: a.text, start: a.start, end: a.end },
                    start: node.start, end: a.end
                };
                j += 2;
            } else if (k === 'LBRACK') {
                const r = parseExpr(j + 1);
                const e = expect('RBRACK', r[1]);
                node = { type: 'index', obj: node, index: r[0], start: node.start, end: tok(e - 1).end };
                j = e;
            } else {
                return [node, j];
            }
        }
    }

    function name(i: number): Name {
        const t = tok(i);
        return { name: t.text, start: t.start, end: t.end };
    }

    function parseStmt(i: number): R<Stmt> {
        const k = kind(i);
        const first = tok(i);
        const line = first.line;
        const start = first.start;
        switch (k) {
            case 'ALLOC': {
                if (kind(i + 1) !== 'ID') {
                    fail("Expected identifier after 'alloc'", i + 1, 'Write `alloc name := value`.');
                }
                const nm = name(i + 1);
                const j = expect('ASSIGN', i + 2);
                const [init, e] = parseExpr(j);
                return [{ type: 'decl', name: nm, init, start, end: init.end, line }, e];
            }
            case 'EMIT': {
                const [expr, e] = parseExpr(i + 1);
                return [{ type: 'emit', expr, start, end: expr.end, line }, e];
            }
            case 'FACTS': {
                const [expr, e] = parseExpr(i + 1);
                return [{ type: 'facts', expr, start, end: expr.end, line }, e];
            }
            case 'IMPORT': {
                if (kind(i + 1) !== 'STR') {
                    fail("Expected string literal after 'import'", i + 1, 'Imports are written `import "file.omg" as name`.');
                }
                const p = tok(i + 1);
                const j = expect('AS', i + 2);
                if (kind(j) !== 'ID') fail("Expected identifier after 'as'", j);
                const alias = name(j);
                return [{
                    type: 'import', path: p.value as string,
                    pathSpan: { start: p.start, end: p.end },
                    alias, start, end: alias.end, line
                }, j + 1];
            }
            case 'IF':
                return parseIf(i, false);
            case 'LOOP': {
                const [cond, j] = parseExpr(i + 1);
                const [body, e] = parseBlock(j);
                return [{ type: 'loop', cond, body, start, end: body.end, line }, e];
            }
            case 'FOR': {
                if (kind(i + 1) !== 'ID') fail("Expected identifier after 'for'", i + 1, 'Write `for item in sequence { ... }`.');
                const nm = name(i + 1);
                const j = expect('IN', i + 2);
                const [iter, b] = parseExpr(j);
                const [body, e] = parseBlock(b);
                return [{ type: 'for', name: nm, iter, body, start, end: body.end, line }, e];
            }
            case 'BREAK':
                return [{ type: 'break', start, end: first.end, line }, i + 1];
            case 'CONTINUE':
                return [{ type: 'continue', start, end: first.end, line }, i + 1];
            case 'FUNC': {
                if (kind(i + 1) !== 'ID') {
                    fail(
                        "Expected function name after 'proc'", i + 1,
                        kind(i + 1) === 'LPAREN' ? 'OMG has no anonymous procs. Give it a name, then pass the name.' : undefined
                    );
                }
                const nm = name(i + 1);
                let j = expect('LPAREN', i + 2);
                j = skipNewlines(j);
                const params: Name[] = [];
                if (kind(j) !== 'RPAREN') {
                    if (kind(j) !== 'ID') fail('Expected parameter name', j);
                    params.push(name(j));
                    j = skipNewlines(j + 1);
                    while (kind(j) === 'COMMA') {
                        j = skipNewlines(j + 1);
                        if (kind(j) !== 'ID') fail('Expected parameter name', j);
                        params.push(name(j));
                        j = skipNewlines(j + 1);
                    }
                }
                if (kind(j) === 'ASSIGN' || kind(j) === 'COLON') {
                    fail(
                        "Expected ')' but got '" + describeToken(kind(j)) + "'", j,
                        'Parameters are plain names: no default values and no types.'
                    );
                }
                j = expect('RPAREN', j);
                const headEnd = tok(j - 1).end;
                const [body, e] = parseBlock(j);
                const fn: FuncStmt = { type: 'func', name: nm, params, body, headEnd, start, end: body.end, line };
                return [fn, e];
            }
            case 'RETURN': {
                const [expr, e] = parseExpr(i + 1);
                return [{ type: 'return', expr, start, end: expr.end, line }, e];
            }
            case 'TRY': {
                const [body, b] = parseBlock(i + 1);
                let j = expect('EXCEPT', b);
                let excName: Name | null = null;
                if (kind(j) === 'ID') {
                    excName = name(j);
                    j++;
                }
                const [handler, e] = parseBlock(j);
                return [{ type: 'try', body, excName, handler, start, end: handler.end, line }, e];
            }
            case 'ID': {
                if (kind(i + 1) === 'ASSIGN') {
                    const [value, e] = parseExpr(i + 2);
                    return [{ type: 'assign', name: name(i), value, start, end: value.end, line }, e];
                }
                const [target, j] = parseLvalue(i);
                if (kind(j) === 'ASSIGN') {
                    const [value, e] = parseExpr(j + 1);
                    if (target.type === 'dot') {
                        return [{ type: 'attr_assign', obj: target.obj, attr: target.attr, value, start, end: value.end, line }, e];
                    }
                    if (target.type === 'index') {
                        return [{ type: 'index_assign', obj: target.obj, index: target.index, value, start, end: value.end, line }, e];
                    }
                    return [{ type: 'expr', expr: target, start, end: target.end, line }, j];
                }
                const [expr, e] = parsePostfix(i);
                return [{ type: 'expr', expr, start, end: expr.end, line }, e];
            }
            default:
                return unexpected(i);
        }
    }

    function parseIf(i: number, isElif: boolean): R<IfStmt> {
        const first = tok(i);
        const [cond, c] = parseExpr(i + 1);
        const [then, t] = parseBlock(c);
        const node: IfStmt = {
            type: 'if', cond, then, otherwise: null, isElif,
            start: first.start, end: then.end, line: first.line
        };
        let j = t;
        if (kind(j) === 'ELIF') {
            const [rest, e] = parseIf(j, true);
            node.otherwise = rest;
            node.end = rest.end;
            j = e;
        } else if (kind(j) === 'ELSE') {
            const [blk, e] = parseBlock(j + 1);
            node.otherwise = blk;
            node.end = blk.end;
            j = e;
        }
        return [node, j];
    }

    const stmts: Stmt[] = [];
    let i = skipNewlines(0);
    while (kind(i) !== 'EOF') {
        if (kind(i) === 'RBRACE' && strayCloses > 0) {
            strayCloses--;
            i = skipNewlines(i + 1);
            continue;
        }
        i = statementInto(stmts, i);
    }
    return { stmts, errors };
}
