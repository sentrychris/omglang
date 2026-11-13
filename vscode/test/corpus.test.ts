// The language server's front end against the real thing.
//
// 1. Every .omg file in the repository compiles, so none may get an
//    error here (apart from the conformance cases that are meant to
//    fail). A warning on working code is a false alarm, so those are
//    held to a short list of known, deliberate cases.
// 2. Break the files in small ways and compare the first syntax error
//    with what `bin/omg --compile` reports: same message, same line.

import { test } from 'node:test';
import * as assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { FileAnalysis, registerBuiltinNames } from '../server/src/analysis';
import { BUILTINS } from '../server/src/builtins';
import { DEFAULT_DIAGNOSTICS, diagnose } from '../server/src/diagnostics';
import { Workspace, pathToUri } from '../server/src/workspace';

registerBuiltinNames(BUILTINS.map((b) => b.name));

const REPO = path.resolve(__dirname, '..', '..', '..');
const OMG = path.join(REPO, 'bin', process.platform === 'win32' ? 'omg.exe' : 'omg');
const haveOmg = fs.existsSync(OMG) && process.platform !== 'darwin';

function omgFiles(dir: string, out: string[] = []): string[] {
    for (const d of fs.readdirSync(dir, { withFileTypes: true })) {
        if (d.name === 'node_modules' || d.name === '.git' || d.name === 'target') continue;
        const p = path.join(dir, d.name);
        if (d.isDirectory()) omgFiles(p, out);
        else if (d.name.endsWith('.omg')) out.push(p);
    }
    return out;
}

const FILES = omgFiles(REPO).sort();

/** What `omg --compile` says about a source text: undefined when it compiles. */
function compilerVerdict(file: string): { message: string; line: number } | undefined {
    const out = path.join(os.tmpdir(), 'omg-lsp-test-' + process.pid + '.omgb');
    try {
        execFileSync(OMG, ['--compile', file, out], { stdio: ['ignore', 'pipe', 'pipe'], timeout: 20000 });
        return undefined;
    } catch (e) {
        const err = e as { stderr?: Buffer; stdout?: Buffer };
        const text = String(err.stderr ?? '') + String(err.stdout ?? '');
        const lines = text.trim().split('\n');
        const last = lines[lines.length - 1] ?? '';
        const m = /^SyntaxError: (.*?) on line (\d+) in /.exec(last) ?? /^SyntaxError: (.*?) on line (\d+)/.exec(last);
        if (m) return { message: m[1], line: Number(m[2]) };
        const m2 = /^SyntaxError: (.*?) in /.exec(last);
        if (m2) return { message: m2[1], line: -1 };
        return { message: last, line: -1 };
    } finally {
        fs.rmSync(out, { force: true });
    }
}

test('the repository has OMG sources to check', () => {
    assert.ok(FILES.length > 100, 'found only ' + FILES.length + ' .omg files');
});

test('no errors in any .omg file of the repository', () => {
    const ws = new Workspace();
    const problems: string[] = [];
    for (const p of FILES) {
        const rel = path.relative(REPO, p);
        const a = ws.get(pathToUri(p));
        assert.ok(a, rel);
        const ds = diagnose(ws, a, DEFAULT_DIAGNOSTICS).filter((d) => d.severity === 1);
        const meantToFail = rel.includes(path.join('conformance', 'fail')) || rel.includes(path.join('tests', 'syntax'));
        if (ds.length > 0 && !meantToFail) {
            for (const d of ds) problems.push(rel + ':' + (d.range.start.line + 1) + ' ' + d.message.split('\n')[0]);
        }
    }
    assert.deepEqual(problems, []);
});

// Warnings on code that works. Each entry is a deliberate construct.
const KNOWN_WARNINGS: Record<string, RegExp[]> = {};

test('no false alarms on working code', () => {
    const ws = new Workspace();
    const problems: string[] = [];
    for (const p of FILES) {
        const rel = path.relative(REPO, p).split(path.sep).join('/');
        if (rel.includes('conformance/fail/') || rel.startsWith('tests/')) continue;
        const a = ws.get(pathToUri(p));
        assert.ok(a, rel);
        const allowed = KNOWN_WARNINGS[rel] ?? [];
        for (const d of diagnose(ws, a, DEFAULT_DIAGNOSTICS)) {
            if (d.severity !== 2) continue;
            const line = d.message.split('\n')[0];
            if (allowed.some((r) => r.test(line))) continue;
            problems.push(rel + ':' + (d.range.start.line + 1) + ' [' + d.code + '] ' + line);
        }
    }
    assert.deepEqual(problems, []);
});

test('syntax errors match the compiler, message and line', { skip: !haveOmg }, () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'omg-lsp-mut-'));
    const sample = FILES.filter((p) => {
        const rel = path.relative(REPO, p).split(path.sep).join('/');
        return rel.startsWith('examples/') || rel.startsWith('tests/conformance/') || rel.startsWith('tools/unix/');
    }).filter((p) => fs.statSync(p).size < 12000);
    let seed = 20261113;
    const rand = (n: number): number => {
        seed = (seed * 1103515245 + 12345) & 0x7fffffff;
        return seed % n;
    };
    const mutations: ((s: string) => string)[] = [
        (s) => { const i = rand(s.length); return s.slice(0, i) + s.slice(i + 1); },
        (s) => { const i = rand(s.length); return s.slice(0, i) + s.slice(i + 1 + rand(4)); },
        (s) => { const i = rand(s.length); const ins = ['(', ')', '{', '}', '[', ']', ',', ':', '"', '=', '!', ' else ', ' := ', '\n', '0b', '1e', '/*', '.', ' proc ', ' return\n', '**', '@'][rand(22)]; return s.slice(0, i) + ins + s.slice(i); },
        (s) => { const lines = s.split('\n'); lines.splice(rand(lines.length), 1); return lines.join('\n'); },
        (s) => { const i = rand(s.length); const j = Math.min(s.length, i + 1 + rand(30)); return s.slice(0, i) + s.slice(j); }
    ];
    let checked = 0;
    let rejected = 0;
    const mismatches: string[] = [];
    try {
        for (let round = 0; round < 600; round++) {
            const src = fs.readFileSync(sample[rand(sample.length)], 'utf8');
            const mutated = mutations[rand(mutations.length)](src);
            const file = path.join(dir, 'm.omg');
            fs.writeFileSync(file, mutated);
            const a = new FileAnalysis(pathToUri(file), mutated);
            const verdict = compilerVerdict(file);
            checked++;
            const ours = a.syntaxErrors[0];
            if (!verdict) {
                if (ours) mismatches.push('compiles, but we say: ' + ours.message + ' (line ' + ours.line + ')\n' + mutated);
                continue;
            }
            if (!verdict.message || /^(ModuleImportError|Traceback)/.test(verdict.message)) continue;
            const semantic = /is already declared|outside of|expects \d+ argument|used in a nested proc/.test(verdict.message);
            if (semantic) {
                // Compile errors past the parser: ours come from the analysis.
                if (ours) mismatches.push('compiler: ' + verdict.message + '\nwe say a syntax error: ' + ours.message + '\n' + mutated);
                else {
                    const errs = a.diagnostics.filter((d) => d.severity === 'error');
                    const stem = verdict.message.replace(/ on line.*$/, '');
                    if (!errs.some((d) => d.message.includes(stem))) {
                        mismatches.push('compiler: ' + verdict.message + '\nwe report: ' + JSON.stringify(errs.map((d) => d.message)) + '\n' + mutated);
                    }
                }
                continue;
            }
            rejected++;
            if (!ours) {
                mismatches.push('compiler: ' + verdict.message + ' (line ' + verdict.line + ')\nwe accept it\n' + mutated);
            } else if (ours.message !== verdict.message || (verdict.line !== -1 && ours.line !== verdict.line)) {
                mismatches.push('compiler: ' + verdict.message + ' (line ' + verdict.line + ')\nwe say:   ' + ours.message + ' (line ' + ours.line + ')\n' + mutated);
            }
        }
    } finally {
        fs.rmSync(dir, { recursive: true, force: true });
    }
    assert.ok(checked > 500 && rejected > 100, 'checked ' + checked + ', rejected ' + rejected);
    if (mismatches.length > 0) {
        assert.fail(mismatches.length + ' of ' + checked + ' differ. First:\n' + mismatches.slice(0, 3).join('\n-----\n').slice(0, 6000));
    }
});
