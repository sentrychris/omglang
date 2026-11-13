// The TextMate grammar, run by the same engine VS Code uses.

import { before, test } from 'node:test';
import * as assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';

import * as oniguruma from 'vscode-oniguruma';
import * as textmate from 'vscode-textmate';

const VSCODE = path.resolve(__dirname, '..', '..');
const REPO = path.resolve(VSCODE, '..');

let grammar: textmate.IGrammar;
let markdown: textmate.IGrammar;

before(async () => {
    const wasm = fs.readFileSync(require.resolve('vscode-oniguruma/release/onig.wasm'));
    await oniguruma.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength) as ArrayBuffer);
    const files: Record<string, string> = {
        'source.omg': 'omg.tmLanguage.json',
        'markdown.omg.codeblock': 'omg.markdown-injection.tmLanguage.json'
    };
    const registry = new textmate.Registry({
        onigLib: Promise.resolve({
            createOnigScanner: (patterns: string[]) => new oniguruma.OnigScanner(patterns),
            createOnigString: (s: string) => new oniguruma.OnigString(s)
        }),
        loadGrammar: async (scope: string) => {
            if (scope === 'text.html.markdown') {
                // A stand-in for VS Code's Markdown grammar: plain text for the injection to land in.
                return textmate.parseRawGrammar(JSON.stringify({ scopeName: scope, patterns: [] }), 'markdown.json');
            }
            const file = files[scope];
            if (!file) return null;
            const p = path.join(VSCODE, 'syntaxes', file);
            return textmate.parseRawGrammar(fs.readFileSync(p, 'utf8'), p);
        },
        getInjections: (scope: string) => (scope === 'text.html.markdown' ? ['markdown.omg.codeblock'] : undefined)
    });
    grammar = (await registry.loadGrammar('source.omg')) as textmate.IGrammar;
    markdown = (await registry.loadGrammar('text.html.markdown')) as textmate.IGrammar;
});

interface Tok {
    line: number;
    text: string;
    scopes: string;
}

function tokenize(source: string, g: textmate.IGrammar = grammar): Tok[] {
    const out: Tok[] = [];
    let stack = textmate.INITIAL;
    source.split('\n').forEach((line, i) => {
        const r = g.tokenizeLine(line, stack);
        for (const t of r.tokens) {
            const text = line.slice(t.startIndex, t.endIndex);
            if (text.trim() !== '') out.push({ line: i + 1, text, scopes: t.scopes.slice(1).join(' ') });
        }
        stack = r.ruleStack;
    });
    return out;
}

/** The scopes of the nth token whose text is exactly `text`. */
function scopeOf(source: string, text: string, nth = 1): string {
    const hits = tokenize(source).filter((t) => t.text === text);
    assert.ok(hits.length >= nth, 'no token ' + JSON.stringify(text) + ' in ' + JSON.stringify(source));
    return hits[nth - 1].scopes;
}

function has(source: string, text: string, scope: string, nth = 1): void {
    const got = scopeOf(source, text, nth);
    assert.ok(got.split(' ').some((s) => s === scope || s.startsWith(scope + '.')), JSON.stringify(text) + ' is ' + (got || 'unscoped') + ', expected ' + scope);
}

function hasNot(source: string, text: string, scope: string, nth = 1): void {
    const got = scopeOf(source, text, nth);
    assert.ok(!got.split(' ').some((s) => s === scope || s.startsWith(scope + '.')), JSON.stringify(text) + ' should not be ' + scope + ' (is ' + got + ')');
}

test('header, comments and doc tags', () => {
    has(';;;omg', 'omg', 'keyword.control.directive');
    has('# plain', ' plain', 'comment.line.number-sign');
    has('emit 1 # after code', ' after code', 'comment.line');
    has('/* one\n   two */', '   two ', 'comment.block');
    has('/** doc */', ' doc ', 'comment.block.documentation');
    has('# @param name the thing', '@param', 'keyword.other.documentation.tag');
    has('# @param name the thing', 'name', 'variable.parameter.documentation');
    has('# TODO later', 'TODO', 'keyword.other.todo');
    hasNot('# if x and proc', ' if x and proc', 'keyword');
});

test('strings and escapes', () => {
    has('emit "hi"', 'hi', 'string.quoted.double');
    for (const esc of ['\\n', '\\t', '\\r', '\\\\', '\\"', '\\0', "\\'"]) has('emit "a' + esc + 'b"', esc, 'constant.character.escape');
    hasNot('emit "a\\qb"', '\\q', 'constant.character.escape');
    hasNot('emit "a\\qb"', '\\q', 'invalid');
    has('emit "a \\" still"', ' still', 'string');
    has('alloc s := "first\nsecond"', 'second', 'string', 1);
    hasNot('emit "if # not a comment"', 'if # not a comment', 'comment');
    has('emit "x" + y', 'y', 'variable.other.readwrite');
});

test('numbers', () => {
    has('emit 42', '42', 'constant.numeric.integer');
    has('emit 3.14', '3.14', 'constant.numeric.float');
    has('emit 1.5e-3', '1.5e-3', 'constant.numeric.float');
    has('emit 2e9', '2e9', 'constant.numeric.float');
    has('emit 0b1010', '0b1010', 'constant.numeric.binary');
    has('emit 0xff', '0xff', 'invalid.illegal');
    has('emit 0b102', '0b102', 'invalid.illegal');
    has('emit xs[1].y', '1', 'constant.numeric.integer');
    has('emit x1', 'x1', 'variable.other.readwrite');
});

test('keywords', () => {
    for (const k of ['if', 'elif', 'else', 'loop', 'for', 'in', 'break', 'continue', 'return', 'try', 'except']) {
        has(k + ' x', k, 'keyword.control');
    }
    has('alloc x := 1', 'alloc', 'storage.type');
    has('proc f() {', 'proc', 'storage.type.function');
    has('emit x', 'emit', 'keyword.other');
    has('facts x', 'facts', 'keyword.other');
    for (const k of ['and', 'or', 'not']) has('emit a ' + k + ' b', k, 'keyword.operator.expression');
    has('emit true', 'true', 'constant.language.boolean');
    has('emit false', 'false', 'constant.language.boolean');
    hasNot('alloc format := 1', 'format', 'keyword');
    hasNot('alloc info := 1', 'info', 'keyword');
    hasNot('emit loops', 'loops', 'keyword');
});

test('declarations', () => {
    const src = 'proc greet(name, times) {';
    has(src, 'greet', 'entity.name.function');
    has(src, 'name', 'variable.parameter');
    has(src, 'times', 'variable.parameter');
    has('import "./lib/util.omg" as util', 'util', 'entity.name.namespace');
    has('import "./lib/util.omg" as util', './lib/util.omg', 'string.quoted.double');
    has('import "./lib/util.omg" as util', 'as', 'keyword.control.import');
    has('alloc MAX_SIZE := 1', 'MAX_SIZE', 'variable.other.constant');
    has('alloc count := 1', 'count', 'variable.other.readwrite');
    has('emit MAX_SIZE', 'MAX_SIZE', 'variable.other.constant');
});

test('calls, builtins and members', () => {
    has('emit length(xs)', 'length', 'support.function.builtin');
    has('emit tcp_nonblocking(h, true)', 'tcp_nonblocking', 'support.function.builtin');
    has('emit stdin_read_bytes()', 'stdin_read_bytes', 'support.function.builtin');
    has('emit helper(xs)', 'helper', 'entity.name.function');
    hasNot('emit helper(xs)', 'helper', 'support.function');
    // A builtin's name is only the builtin when it is called on its own.
    hasNot('alloc length := 3', 'length', 'support.function');
    hasNot('emit obj.length(1)', 'length', 'support.function');
    has('emit obj.length(1)', 'length', 'entity.name.function.member');
    hasNot('emit lengthen(1)', 'lengthen', 'support.function');
    has('panic("no")', 'panic', 'keyword.control.exception');
    has('raise("no")', 'raise', 'keyword.control.exception');
    has('emit __peek64(p)', '__peek64', 'support.function.intrinsic');
    has('emit args[1]', 'args', 'variable.language');
    hasNot('emit cfg.args', 'args', 'variable.language');
    // A dotted name is a property, not a module: `ed` here is a dict.
    hasNot('emit ed.cols', 'ed', 'entity.name.namespace');
    has('emit ed.cols', 'cols', 'variable.other.property');
    has('ed.buf.insert(0, x)', 'buf', 'variable.other.property');
    has('ed.buf.insert(0, x)', 'insert', 'entity.name.function.member');
    hasNot('if (x) {', 'if', 'entity.name.function');
});

test('dict keys and slices', () => {
    has('alloc d := {name: 1}', 'name', 'variable.other.property');
    has('alloc d := {\n    name: 1,\n    size: 2\n}', 'size', 'meta.object-literal.key');
    // The part before the colon of a slice is a value.
    hasNot('emit xs[a:b]', 'a', 'meta.object-literal.key');
    has('emit xs[a:b]', ':', 'punctuation.separator.slice');
    hasNot('emit s[start:start + n]', 'start', 'meta.object-literal.key');
    // A dict inside a list has keys again; a slice inside that dict does not.
    has('alloc rows := [{id: xs[i:j]}]', 'id', 'meta.object-literal.key');
    hasNot('alloc rows := [{id: xs[i:j]}]', 'i', 'meta.object-literal.key');
    has('x := y', ':=', 'keyword.operator.assignment');
    hasNot('x := y', 'x', 'meta.object-literal.key');
    hasNot('xs[i] := y', 'i', 'meta.object-literal.key');
});

test('operators', () => {
    has('emit 7 // 2', '//', 'keyword.operator.arithmetic');
    hasNot('emit 7 // 2', '//', 'comment');
    has('emit 7 // 2', '2', 'constant.numeric');
    for (const op of ['+', '-', '*', '/', '%']) has('emit a ' + op + ' b', op, 'keyword.operator.arithmetic');
    for (const op of ['==', '!=', '<', '>', '<=', '>=']) has('emit a ' + op + ' b', op, 'keyword.operator.comparison');
    for (const op of ['&', '|', '^', '<<', '>>']) has('emit a ' + op + ' b', op, 'keyword.operator.bitwise');
    has('emit ~a', '~', 'keyword.operator.bitwise');
});

test('what OMG does not have is marked', () => {
    has('x = 1', '=', 'invalid.illegal');
    has("emit 'single'", "'single'", 'invalid.illegal');
    has('emit a && b', '&&', 'invalid.illegal');
    has('emit a || b', '||', 'invalid.illegal');
    has('emit a ** b', '**', 'invalid.illegal');
    has('emit !a', '!', 'invalid.illegal');
    has('emit 1;', ';', 'invalid.illegal');
});

test('nothing in the repository is marked invalid', () => {
    const files: string[] = [];
    const walk = (dir: string): void => {
        for (const d of fs.readdirSync(dir, { withFileTypes: true })) {
            if (d.name === 'node_modules' || d.name === '.git' || d.name === 'target') continue;
            const p = path.join(dir, d.name);
            if (d.isDirectory()) walk(p);
            else if (d.name.endsWith('.omg')) files.push(p);
        }
    };
    walk(REPO);
    assert.ok(files.length > 100);
    const bad: string[] = [];
    for (const p of files) {
        const rel = path.relative(REPO, p).split(path.sep).join('/');
        if (rel.includes('conformance/fail/') || rel.startsWith('tests/syntax/')) continue;
        for (const t of tokenize(fs.readFileSync(p, 'utf8'))) {
            if (t.scopes.includes('invalid.')) bad.push(rel + ':' + t.line + ' ' + JSON.stringify(t.text));
        }
    }
    assert.deepEqual(bad.slice(0, 20), []);
});

test('OMG inside a Markdown code fence', () => {
    const doc = 'Some text with emit in it.\n\n```omg\nemit length("abc")\n```\n\nafter\n';
    const toks = tokenize(doc, markdown);
    const find = (text: string, nth = 1): string => toks.filter((t) => t.text === text)[nth - 1]?.scopes ?? '(none)';
    assert.match(find('length'), /meta\.embedded\.block\.omg.*support\.function\.builtin\.omg/);
    assert.match(find('emit'), /keyword\.other\.emit\.omg/);
    assert.doesNotMatch(find('Some text with emit in it.'), /omg/);
    assert.doesNotMatch(find('after'), /omg/);
});
