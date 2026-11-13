// The reference data against the real thing: the builtin list matches
// the compiler's, the grammar colours the same names, and every example
// shown in a hover prints what its `# →` notes say when run by bin/omg.

import { test } from 'node:test';
import * as assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { FileAnalysis } from '../server/src/analysis';
import { BUILTINS, RAISERS, RESERVED_GLOBALS } from '../server/src/builtins';
import { KEYWORD_DOCS, OPERATOR_DOCS } from '../server/src/language';
import { KEYWORD_LIST } from '../server/src/lexer';

const REPO = path.resolve(__dirname, '..', '..', '..');
const OMG = path.join(REPO, 'bin', process.platform === 'win32' ? 'omg.exe' : 'omg');
const haveOmg = fs.existsSync(OMG) && process.platform !== 'darwin';

function compilerList(name: string): string[] {
    const src = fs.readFileSync(path.join(REPO, 'src', 'compiler.omg'), 'utf8');
    const m = new RegExp('alloc ' + name + ' := \\[([^\\]]*)\\]').exec(src);
    assert.ok(m, name + ' not found in src/compiler.omg');
    return [...m[1].matchAll(/"([^"]+)"/g)].map((x) => x[1]);
}

test('the builtin list is the compiler\'s', () => {
    const theirs = compilerList('cc_builtins').sort();
    const ours = BUILTINS.map((b) => b.name).sort();
    assert.deepEqual(ours, theirs);
});

test('every builtin is documented in full', () => {
    for (const b of [...BUILTINS, ...RAISERS]) {
        assert.ok(b.summary.length > 10, b.name + ': summary');
        assert.ok(b.signatures.length > 0, b.name + ': signature');
        assert.equal(b.params.length, b.maxArgs, b.name + ': one description per parameter');
        assert.ok(b.minArgs <= b.maxArgs, b.name + ': argument counts');
        assert.ok(b.example, b.name + ': example');
        for (const s of b.see ?? []) {
            assert.ok(BUILTINS.some((x) => x.name === s) || RAISERS.some((x) => x.name === s), b.name + ': see ' + s);
        }
    }
});

test('every keyword has a hover', () => {
    const documented = KEYWORD_DOCS.map((k) => k.word).sort();
    assert.deepEqual(documented, [...KEYWORD_LIST].sort());
});

test('the grammar colours exactly the builtins', () => {
    const grammar = JSON.parse(fs.readFileSync(path.join(REPO, 'vscode', 'syntaxes', 'omg.tmLanguage.json'), 'utf8'));
    const rules = grammar.repository['language-names'].patterns as { name: string; match: string }[];
    const names = (scope: string): string[] => {
        const rule = rules.find((r) => r.name === scope);
        assert.ok(rule, scope);
        const m = /\(\?:([a-z0-9_|]+)\)/.exec(rule.match);
        assert.ok(m, scope + ': list of names');
        return m[1].split('|');
    };
    const coloured = names('support.function.builtin.omg');
    assert.deepEqual([...coloured].sort(), BUILTINS.map((b) => b.name).sort());
    // A name that begins another must come after it, or the shorter one wins.
    coloured.forEach((n, i) => {
        for (const later of coloured.slice(i + 1)) assert.ok(!later.startsWith(n), n + ' is listed before ' + later);
    });
    assert.deepEqual(names('keyword.control.exception.throw.omg').sort(), RAISERS.map((r) => r.name).sort());
    assert.deepEqual(names('variable.language.omg').sort(), RESERVED_GLOBALS.map((g) => g.name).sort());
});

interface Example {
    where: string;
    source: string;
}

function examples(): Example[] {
    const out: Example[] = [];
    for (const b of [...BUILTINS, ...RAISERS]) {
        if (b.example && b.runnable !== false) out.push({ where: b.name + '()', source: b.example });
    }
    for (const k of KEYWORD_DOCS) {
        if (k.example && k.runnable !== false) out.push({ where: 'keyword ' + k.word, source: k.example });
    }
    for (const [kind, o] of Object.entries(OPERATOR_DOCS)) {
        const ex = (o as { example?: string }).example;
        if (ex) out.push({ where: 'operator ' + kind, source: ex });
    }
    for (const g of RESERVED_GLOBALS) {
        const ex = (g as { example?: string; runnable?: boolean });
        if (ex.example && ex.runnable !== false) out.push({ where: 'global ' + g.name, source: ex.example });
    }
    return out;
}

test('every example prints what it says it prints', { skip: !haveOmg }, () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'omg-lsp-ex-'));
    const wrong: string[] = [];
    let ran = 0;
    try {
        for (const ex of examples()) {
            const expected = [...ex.source.matchAll(/#\s*→ ?(.*)$/gm)].map((m) => m[1].trimEnd());
            const file = path.join(dir, 'example.omg');
            fs.writeFileSync(file, ';;;omg\n' + ex.source + '\n');
            let stdout = '';
            try {
                stdout = execFileSync(OMG, [file], { cwd: dir, encoding: 'utf8', timeout: 20000, stdio: ['ignore', 'pipe', 'pipe'] });
            } catch (e) {
                const err = e as { stdout?: string; stderr?: string };
                wrong.push(ex.where + ' failed:\n' + String(err.stderr ?? '') + '\n' + ex.source);
                continue;
            }
            ran++;
            const got = stdout.replace(/\n$/, '').split('\n').map((l) => l.trimEnd());
            if (expected.length === 0) continue;
            if (JSON.stringify(got) !== JSON.stringify(expected)) {
                wrong.push(ex.where + '\n  says:   ' + JSON.stringify(expected) + '\n  prints: ' + JSON.stringify(got));
            }
        }
    } finally {
        fs.rmSync(dir, { recursive: true, force: true });
    }
    assert.deepEqual(wrong, []);
    assert.ok(ran > 80, 'only ' + ran + ' examples ran');
});

test('the examples that cannot be run here at least parse', () => {
    const all = [
        ...[...BUILTINS, ...RAISERS].map((b) => ({ where: b.name + '()', source: b.example })),
        ...KEYWORD_DOCS.map((k) => ({ where: 'keyword ' + k.word, source: k.example }))
    ];
    for (const ex of all) {
        if (!ex.source) continue;
        const a = new FileAnalysis('file:///example.omg', ';;;omg\n' + ex.source + '\n');
        assert.deepEqual(a.syntaxErrors.map((e) => e.message), [], ex.where);
    }
});
