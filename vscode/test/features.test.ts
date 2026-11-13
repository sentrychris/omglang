// What the editor asks the language server, on a small two-file project.

import { after, test } from 'node:test';
import * as assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { FileAnalysis, registerBuiltinNames } from '../server/src/analysis';
import { BUILTINS } from '../server/src/builtins';
import { DEFAULT_DIAGNOSTICS, codeActions, diagnose } from '../server/src/diagnostics';
import { completion, hover, signatureHelp } from '../server/src/features';
import { formatLines } from '../server/src/format';
import { positionAt } from '../server/src/lexer';
import {
    definition, documentLinks, documentSymbols, foldingRanges, highlights, prepareRename,
    references, rename, workspaceSymbols
} from '../server/src/navigation';
import { TOKEN_MODIFIERS, TOKEN_TYPES, semanticTokens } from '../server/src/semantic';
import { Workspace, pathToUri } from '../server/src/workspace';

registerBuiltinNames(BUILTINS.map((b) => b.name));

const SHAPES = `;;;omg
# Small helpers for shapes.

alloc PI := 3.14159

# Area of a circle.
# @param r the radius
# @return the area, as a float
proc circle_area(r) {
    return PI * r * r
}

proc make_point(x, y) {
    return {x: x, y: y, label: "p"}
}
`;

const MAIN = `;;;omg
import "./shapes.omg" as shapes

alloc LIMIT := 10
alloc total := 0   # running total

# Adds up to n.
proc sum_to(n) {
    alloc acc := 0
    alloc i := 0
    loop i < n {
        acc := acc + i
        i := i + 1
    }
    return acc
}

proc make_counter(start) {
    alloc n := start
    proc tick(step) {
        n := n + step
        return n
    }
    return tick
}

alloc pt := shapes.make_point(1, 2)
emit pt.x + shapes.circle_area(2)
emit length("abc") + sum_to(LIMIT)
alloc next := make_counter(5)
emit next(1)
for ch in "hey" {
    emit ascii(ch)
}
try {
    panic("no")
} except err {
    emit err
}
alloc cfg := {name: "omg", size: 3, nested: {deep: true}}
emit cfg.nested.deep
emit 0b1010 + binary(5, 8) + args[0]
`;

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'omg-lsp-features-'));
after(() => fs.rmSync(dir, { recursive: true, force: true }));
fs.writeFileSync(path.join(dir, 'shapes.omg'), SHAPES);
fs.writeFileSync(path.join(dir, 'main.omg'), MAIN);

const ws = new Workspace();
ws.setRoots([dir]);
const mainUri = pathToUri(path.join(dir, 'main.omg'));
const shapesUri = pathToUri(path.join(dir, 'shapes.omg'));
ws.open(mainUri, MAIN);
const main = ws.get(mainUri) as FileAnalysis;

/** The position of the nth occurrence of `needle` in main.omg, `delta` characters in. */
function at(needle: string, delta = 0, nth = 1) {
    let i = -1;
    for (let k = 0; k < nth; k++) i = MAIN.indexOf(needle, i + 1);
    assert.ok(i >= 0, 'no ' + needle);
    return positionAt(main.lex.lineStarts, i + delta);
}

function hoverText(needle: string, delta = 0, nth = 1): string {
    const h = hover(ws, main, at(needle, delta, nth));
    return h ? (h.contents as { value: string }).value : '';
}

/** A scratch file with the cursor at `|`. */
function cursor(text: string): { file: FileAnalysis; pos: { line: number; character: number }; uri: string } {
    const uri = pathToUri(path.join(dir, 'scratch.omg'));
    const i = text.indexOf('|');
    // Without a marker the cursor is not used: the text goes in as it is.
    ws.open(uri, i < 0 ? text : text.slice(0, i) + text.slice(i + 1));
    const file = ws.get(uri) as FileAnalysis;
    return { file, pos: positionAt(file.lex.lineStarts, i), uri };
}

function labels(text: string): string[] {
    const c = cursor(text);
    return completion(ws, c.file, c.pos).map((i) => i.label);
}

test('hover: a builtin shows its signature, parameters, result and a run example', () => {
    const text = hoverText('length');
    assert.match(text, /length\(x\)/);
    assert.match(text, /\*\*Parameters\*\*/);
    assert.match(text, /\*\*Returns\*\* an int/);
    assert.match(text, /# → 5/);
});

test('hover: a proc from another file shows the comment written above it', () => {
    const text = hoverText('circle_area', 2);
    assert.match(text, /proc circle_area\(r\)/);
    assert.match(text, /shapes\.omg/);
    assert.match(text, /Area of a circle\./);
    assert.match(text, /`r`: the radius/);
    assert.match(text, /\*\*Returns\*\* the area, as a float/);
});

test('hover: a comment keeps its angle brackets', () => {
    const c = cursor(';;;omg\n# Runs in <top-level> when `a < b`, not \\<here>.\nproc f|() {\n    return 1\n}\n');
    const text = (hover(ws, c.file, c.pos)?.contents as { value: string }).value;
    assert.match(text, /Runs in &lt;top-level> when `a < b`, not \\&lt;here>\./);
});

test('hover: modules, variables, closures, dict entries, keywords, operators, numbers', () => {
    assert.match(hoverText('shapes', 1, 2), /\*\*Procs\*\* `circle_area\(r\)`, `make_point\(x, y\)`/);
    assert.match(hoverText('"./shapes.omg"', 3), /import ".\/shapes.omg" as shapes/);
    assert.match(hoverText('LIMIT', 1, 2), /alloc LIMIT := 10[\s\S]*global variable · int/);
    assert.match(hoverText('total'), /running total/);
    assert.match(hoverText('next(1)'), /closure `tick\(step\)`/);
    assert.match(hoverText('acc', 0, 2), /local variable of `sum_to`/);
    assert.match(hoverText('pt.x', 3), /dict entry/);
    assert.match(hoverText('cfg.nested.deep', 12), /deep: true[\s\S]*bool/);
    assert.match(hoverText('make_point', 2), /Keys: `x`, `y`, `label`/);
    assert.match(hoverText('err', 1, 2), /caught error/);
    assert.match(hoverText('ch', 0, 2), /loop variable · string/);
    assert.match(hoverText('loop'), /Repeats a block while a condition is truthy/);
    assert.match(hoverText(':='), /Assignment/);
    assert.match(hoverText('0b1010'), /decimal \| `10`/);
    assert.match(hoverText('args'), /The command line/);
    assert.match(hoverText('panic'), /RuntimeError/);
    assert.match(hoverText(';;;omg', 2), /file header/);
    assert.equal(hoverText('running'), '', 'nothing inside a comment');
});

test('completion: members after a dot', () => {
    assert.deepEqual(labels(';;;omg\nimport "./shapes.omg" as shapes\nemit shapes.|\n').sort(), ['PI', 'circle_area', 'make_point']);
    assert.deepEqual(labels(';;;omg\nalloc cfg := {name: "omg", size: 3}\nemit cfg.|\n').sort(), ['name', 'size']);
    assert.deepEqual(
        labels(';;;omg\nimport "./shapes.omg" as shapes\nalloc p := shapes.make_point(1, 2)\nemit p.|\n').sort(),
        ['label', 'x', 'y']
    );
});

test('completion: fits where the cursor is', () => {
    const inProc = labels(';;;omg\nalloc top := 1\nproc f(aa, bb) {\n    alloc local := 1\n    |\n}\n');
    for (const want of ['alloc', 'return', 'aa', 'bb', 'local', 'top', 'f', 'length']) assert.ok(inProc.includes(want), want);
    const expr = labels(';;;omg\nalloc LIMIT := 1\nemit le|\n');
    assert.ok(expr.includes('length') && expr.includes('LIMIT') && expr.includes('true'));
    assert.ok(!expr.includes('alloc'), 'no statement keywords inside an expression');
    assert.deepEqual(labels(';;;omg\nalloc |\n'), [], 'a new name is yours to choose');
    assert.deepEqual(labels(';;;omg\nemit "le|"\n'), [], 'nothing inside a string');
    assert.deepEqual(labels(';;;omg\n# le|\n'), [], 'nothing inside a comment');
    assert.deepEqual(labels(';;;omg\nimport "./shapes.omg" |\n'), ['as']);
    assert.deepEqual(labels(';;;omg\nfor x |\n'), ['in']);
    assert.deepEqual(labels(';;;omg\nimport "./|"\n').sort(), ['main.omg', 'shapes.omg']);
});

test('signature help: builtins with two forms, documented procs, procs of a module', () => {
    let c = cursor(';;;omg\nemit binary(5, |)\n');
    let s = signatureHelp(ws, c.file, c.pos);
    assert.ok(s);
    assert.deepEqual(s.signatures.map((x) => x.label), ['binary(n)', 'binary(n, width)']);
    assert.equal(s.activeSignature, 1);
    assert.equal(s.activeParameter, 1);

    c = cursor(';;;omg\n# Adds.\n# @param a first\n# @param b second\nproc add(a, b) {\n    return a + b\n}\nemit add(1, |\n');
    s = signatureHelp(ws, c.file, c.pos);
    assert.ok(s);
    assert.equal(s.signatures[0].label, 'add(a, b)');
    assert.equal(s.activeParameter, 1);
    assert.deepEqual(s.signatures[0].parameters?.map((p) => (p.documentation as { value: string }).value), ['first', 'second']);

    c = cursor(';;;omg\nimport "./shapes.omg" as shapes\nemit shapes.make_point(1|)\n');
    s = signatureHelp(ws, c.file, c.pos);
    assert.equal(s?.signatures[0].label, 'make_point(x, y)');

    c = cursor(';;;omg\nemit length(hex(|))\n');
    assert.equal(signatureHelp(ws, c.file, c.pos)?.signatures[0].label, 'hex(n)', 'the innermost call');
});

test('go to definition: across files, into dicts, through import paths', () => {
    const d1 = definition(ws, main, at('circle_area', 2));
    assert.equal(d1?.uri, shapesUri);
    assert.equal(d1?.range.start.line, 8);
    const d2 = definition(ws, main, at('sum_to', 1, 2));
    assert.equal(d2?.uri, mainUri);
    assert.equal(d2?.range.start.line, 7);
    assert.equal(definition(ws, main, at('"./shapes.omg"', 3))?.uri, shapesUri);
    const d3 = definition(ws, main, at('pt.x', 3));
    assert.equal(d3?.uri, shapesUri);
    assert.equal(d3?.range.start.line, 13);
    assert.equal(definition(ws, main, at('length')), null, 'builtins have no source');
});

test('references and highlights', () => {
    assert.deepEqual(references(ws, main, at('LIMIT', 1), true).map((r) => r.range.start.line), [3, 28]);
    assert.deepEqual(references(ws, main, at('LIMIT', 1), false).map((r) => r.range.start.line), [28]);
    const across = references(ws, main, at('circle_area', 2), true).map((r) => path.basename(r.uri) + ':' + r.range.start.line);
    assert.deepEqual(across.sort(), ['main.omg:27', 'shapes.omg:8']);
    // `n` of make_counter, as the closure reads and writes it.
    assert.equal(highlights(ws, main, at('alloc n', 6)).length, 4);
});

test('rename: every use, in every file, and no bad names', () => {
    assert.deepEqual(prepareRename(ws, main, at('sum_to', 1))?.placeholder, 'sum_to');
    assert.equal(prepareRename(ws, main, at('length', 1)), null, 'builtins cannot be renamed');
    const edit = rename(ws, main, at('sum_to', 1), 'add_up');
    assert.ok(typeof edit !== 'string');
    assert.deepEqual(edit.changes?.[mainUri].map((e) => e.range.start.line), [7, 28]);
    const across = rename(ws, main, at('circle_area', 2), 'area');
    assert.ok(typeof across !== 'string');
    assert.deepEqual(Object.keys(across.changes ?? {}).map((u) => path.basename(u)).sort(), ['main.omg', 'shapes.omg']);
    assert.match(String(rename(ws, main, at('sum_to', 1), 'loop')), /keyword/);
    assert.equal(typeof rename(ws, main, at('sum_to', 1), '9lives'), 'string');
    assert.match(String(rename(ws, main, at('sum_to', 1), 'total')), /already in use here \(declared on line 5\)/);
    assert.match(String(rename(ws, main, at('sum_to', 1), 'length')), /is a builtin/);
    assert.match(String(rename(ws, main, at('acc', 0, 2), 'LIMIT')), /already in use/, 'a local must not hide a global the proc may read');
});

test('outline, workspace symbols, folding, links', () => {
    const outline = documentSymbols(main).map((s) => s.name + (s.children?.length ? '>' + s.children.map((c) => c.name).join(',') : ''));
    assert.deepEqual(outline, ['shapes', 'LIMIT', 'total', 'sum_to', 'make_counter>tick', 'pt', 'next', 'cfg']);
    assert.deepEqual(workspaceSymbols(ws, 'po').map((s) => s.name), ['make_point']);
    assert.ok(foldingRanges(main).some((r) => r.startLine === 7 && r.endLine === 14));
    assert.equal(documentLinks(ws, main)[0].target, shapesUri);
});

test('semantic tokens say what each name is', () => {
    const data = semanticTokens(main);
    const lines = MAIN.split('\n');
    const seen = new Map<string, string>();
    let line = 0;
    let ch = 0;
    for (let i = 0; i < data.length; i += 5) {
        line += data[i];
        ch = data[i] === 0 ? ch + data[i + 1] : data[i + 1];
        const mods = TOKEN_MODIFIERS.filter((_, b) => data[i + 4] & (1 << b));
        const key = (line + 1) + ':' + lines[line].substr(ch, data[i + 2]);
        seen.set(key, [TOKEN_TYPES[data[i + 3]], ...mods].join(' '));
    }
    assert.equal(seen.get('2:shapes'), 'namespace declaration');
    assert.equal(seen.get('4:LIMIT'), 'variable declaration readonly global');
    assert.equal(seen.get('5:total'), 'variable declaration global');
    assert.equal(seen.get('8:sum_to'), 'function declaration global');
    assert.equal(seen.get('8:n'), 'parameter declaration');
    assert.equal(seen.get('9:acc'), 'variable declaration');
    assert.equal(seen.get('20:tick'), 'function declaration');
    assert.equal(seen.get('27:make_point'), 'method');
    assert.equal(seen.get('28:x'), 'property');
    assert.equal(seen.get('29:length'), 'function defaultLibrary');
    assert.equal(seen.get('31:next'), 'function global', 'a variable holding a closure, where it is called');
    assert.equal(seen.get('42:args'), 'variable defaultLibrary readonly');
    assert.equal(seen.get('40:deep'), 'property declaration');
});

function problems(text: string): string[] {
    const c = cursor(text);
    return diagnose(ws, c.file, DEFAULT_DIAGNOSTICS).map((d) => (d.range.start.line + 1) + ' ' + d.code + ' ' + d.severity);
}

test('diagnostics: the project itself is clean', () => {
    assert.deepEqual(diagnose(ws, main, DEFAULT_DIAGNOSTICS), []);
    assert.deepEqual(diagnose(ws, ws.get(shapesUri) as FileAnalysis, DEFAULT_DIAGNOSTICS), []);
});

test('diagnostics: syntax errors read as the compiler writes them', () => {
    const c = cursor(';;;omg\nemit 1 +\nalloc x := 2\n');
    const ds = diagnose(ws, c.file, DEFAULT_DIAGNOSTICS);
    assert.equal(ds.length, 1);
    assert.equal(ds[0].message.split('\n')[0], "SyntaxError: Unexpected token 'newline'");
    assert.equal(ds[0].range.start.line, 1);
});

test('diagnostics: mistakes the compiler leaves for run time', () => {
    assert.deepEqual(problems(';;;omg\nproc f(a) {\n    return a\n}\nemit f(1, 2)\n'), ['5 arity 1']);
    assert.deepEqual(problems(';;;omg\nalloc x := 1\nalloc x := 2\nemit x\n'), ['3 duplicate-alloc 1']);
    assert.deepEqual(problems(';;;omg\nreturn 1\n'), ['2 return-outside-proc 1']);
    assert.deepEqual(problems(';;;omg\nbreak\n'), ['2 break-outside-loop 1']);
    assert.deepEqual(problems(';;;omg\ncount := 1\n'), ['2 undeclared-assignment 2']);
    assert.deepEqual(problems(';;;omg\nalloc total := 1\nemit totl\n'), ['3 undefined-name 2']);
    assert.deepEqual(problems(';;;omg\nemit length(1, 2)\n'), ['2 builtin-arity 2']);
    assert.deepEqual(problems(';;;omg\nimport "./nowhere.omg" as gone\nemit gone.x\n'), ['2 import-not-found 1']);
    assert.deepEqual(problems(';;;omg\nimport "./shapes.omg" as shapes\nemit shapes.circle(1)\n'), ['3 unknown-member 2']);
    assert.deepEqual(problems(';;;omg\nimport "./shapes.omg" as shapes\nshapes.PI := 3\n'), ['3 frozen-write 2']);
    assert.deepEqual(problems(';;;omg\nproc f() {\n    return 1\n    emit 2\n}\nemit f()\n'), ['4 unreachable 4']);
    assert.deepEqual(problems(';;;omg\nproc f(a) {\n    alloc spare := 1\n    return a\n}\nemit f(1)\n'), ['3 unused 4']);
    assert.deepEqual(problems(';;;omg\nproc length(x) {\n    return 0\n}\n'), ['2 shadows-builtin 2']);
});

test('diagnostics: settings switch the optional ones off', () => {
    const c = cursor(';;;omg\nproc f(a) {\n    alloc spare := 1\n    return a + nope\n}\nemit f(1)\n');
    const codes = (s: typeof DEFAULT_DIAGNOSTICS): string[] => diagnose(ws, c.file, s).map((d) => String(d.code)).sort();
    assert.deepEqual(codes(DEFAULT_DIAGNOSTICS), ['undefined-name', 'unused']);
    assert.deepEqual(codes({ ...DEFAULT_DIAGNOSTICS, unused: false }), ['undefined-name']);
    assert.deepEqual(codes({ ...DEFAULT_DIAGNOSTICS, undefinedNames: false }), ['unused']);
    assert.deepEqual(codes({ ...DEFAULT_DIAGNOSTICS, enabled: false }), []);
});

function fixed(text: string): string[] {
    const c = cursor(text);
    const src = c.file.text;
    const ds = diagnose(ws, c.file, DEFAULT_DIAGNOSTICS);
    const whole = { start: { line: 0, character: 0 }, end: { line: 999, character: 0 } };
    return codeActions(c.file, c.uri, whole, ds).map((a) => {
        const e = a.edit?.changes?.[c.uri][0];
        assert.ok(e);
        const from = c.file.lex.lineStarts[e.range.start.line] + e.range.start.character;
        const to = c.file.lex.lineStarts[e.range.end.line] + e.range.end.character;
        return a.title + ' => ' + JSON.stringify(src.slice(0, from) + e.newText + src.slice(to));
    });
}

test('quick fixes rewrite the code correctly', () => {
    assert.deepEqual(fixed(';;;omg\ncount := 1\n'), ['Declare with alloc => ";;;omg\\nalloc count := 1\\n"']);
    assert.deepEqual(
        fixed(';;;omg\nalloc x := 1\nalloc x := 2\nemit x\n'),
        ['Assign instead of declaring again => ";;;omg\\nalloc x := 1\\nx := 2\\nemit x\\n"']
    );
    assert.deepEqual(
        fixed(';;;omg\nalloc total := 1\nemit totl\n'),
        ['Change to \'total\' => ";;;omg\\nalloc total := 1\\nemit total\\n"']
    );
    assert.deepEqual(
        fixed(';;;omg\nimport "./shapes.omg" as shapes\nemit 1\n'),
        ['Remove unused import => ";;;omg\\nemit 1\\n"']
    );
});

function formatted(src: string, tabSize = 4, spaces = true): string {
    const uri = pathToUri(path.join(dir, 'fmt.omg'));
    ws.open(uri, src);
    const lines = src.split('\n');
    const edits = formatLines(ws.get(uri) as FileAnalysis, tabSize, spaces);
    // Right to left, so earlier columns stay valid.
    edits.sort((a, b) => b.line - a.line || b.from - a.from);
    for (const e of edits) lines[e.line] = lines[e.line].slice(0, e.from) + e.text + lines[e.line].slice(e.to);
    return lines.join('\n');
}

test('formatting: indentation and trailing spaces, nothing else', () => {
    const messy = ';;;omg\nproc f(a) {\nif a {   \n        emit [\n1,\n2\n]\n} else {\nemit 0\n}\n}\n';
    const tidy = ';;;omg\nproc f(a) {\n    if a {\n        emit [\n            1,\n            2\n        ]\n    } else {\n        emit 0\n    }\n}\n';
    assert.equal(formatted(messy), tidy);
    assert.equal(formatted(tidy), tidy, 'formatting twice changes nothing');
    assert.equal(formatted(messy, 4, false), tidy.replace(/ {4}/g, '\t'));
});

test('formatting: leaves strings, block comments and hand-aligned lines alone', () => {
    const keep = ';;;omg\nalloc s := "two\n   lines  "\n/* a\n      b */\nalloc r := call(1,\n               2)\nalloc d := {a: 1,   # first\n                    # still about a\n            b: 2}\n';
    assert.equal(formatted(keep), keep);
    assert.equal(formatted(';;;omg\nemit "never closed\n  x\n'), ';;;omg\nemit "never closed\n  x\n', 'no guessing in a broken file');
});
