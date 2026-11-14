// Test for the browser bundles that scripts/build-web.sh writes:
// omg-web.js (runs a program), omg-explorer.js (reports every stage of
// the compiler, then runs it) and omg-rt.js (the JS runtime, to run
// JavaScript the explorer generated).
//
//     scripts/build-web.sh
//     node tests/web-bundles.mjs [bundle-dir] [--worker path/to/omg-worker.js]
//
// The bundle directory defaults to build/web. tests/web.sh runs this as
// part of the suite.
//
// The page that loads these files is not in this repository: it is the
// playground on the site. It loads a bundle the way this test does. A
// bundle is a script that reads its input from a few `let` lines at
// the top of the runtime (args, the two output functions, exit, the
// file system); the page rewrites those lines to read from an object
// of its own and runs the result as a function body, with `process`
// and `require` undefined so that the runtime takes its browser path.
// So those five lines of src/runtime/omg_rt.js are an interface: change
// one and the site's playground stops loading. PATCHES below is a copy
// of the site's list, and every line must be found in every file.
//
// With --worker, the site's own loader (assets/omg-worker.js) does the
// loading and running instead of the copy here. scripts/update-site.sh
// uses that before it copies anything, so that what is checked is the
// site as it is, not as this file remembers it.

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const argv = process.argv.slice(2);
let bundleDir = 'build/web';
let workerPath = null;
for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--worker') workerPath = argv[++i];
    else bundleDir = argv[i];
}

function read(name) {
    try {
        return readFileSync(resolve(bundleDir, name), 'utf8');
    } catch (e) {
        console.log(`Cannot read ${bundleDir}/${name}: run scripts/build-web.sh first.`);
        process.exit(1);
    }
}
const TEXT = {
    'omg-web.js': read('omg-web.js'),
    'omg-explorer.js': read('omg-explorer.js'),
    'omg-rt.js': read('omg-rt.js'),
};

// ---------------------------------------------------------------------------
// Loading and running a bundle, as the site does it.
// ---------------------------------------------------------------------------

const PATCHES = [
    [
        "let _omg_write = (s) => { console.log(s); };",
        "let _omg_write = (s) => { __host.write(String(s) + '\\n'); };",
    ],
    [
        "let _omg_print_raw = (s) => { process.stdout.write(s); };",
        "let _omg_print_raw = (s) => { __host.write(String(s)); };",
    ],
    [
        "let _omg_exit = (code) => { if (typeof process !== 'undefined') process.exit(Number(code)); };",
        "let _omg_exit = (code) => { throw new OmgError('Exit', '' + code); };",
    ],
    [
        "let _omg_fs = null;",
        "let _omg_fs = __host.fs;",
    ],
    [
        "let args = (typeof process !== 'undefined') ? process.argv.slice(1) : [];",
        "let args = __host.args;",
    ],
];

function ownCompileBundle(text, name) {
    let out = text;
    for (const [from, to] of PATCHES) {
        if (!out.includes(from)) {
            throw new Error(name + ': the runtime header has changed (missing: ' + from + ')');
        }
        out = out.replace(from, () => to);
    }
    return new Function('__host', 'process', 'require', 'Buffer', out);
}

// Just enough of a file system for `read_file` and `import`.
function memoryFs(seed) {
    const files = new Map(Object.entries(seed || {}).map(([k, v]) => [k.replace(/^\.\//, ''), v]));
    const norm = (p) => String(p).replace(/\\/g, '/').split('/').filter((s) => s !== '' && s !== '.').join('/');
    const isDir = (p) => p === '' || [...files.keys()].some((k) => k.startsWith(p + '/'));
    const missing = (p) => new Error("ENOENT: no such file or directory, '" + p + "'");
    return {
        readFileSync(path, encoding) {
            if (typeof path !== 'string' || !files.has(norm(path))) throw missing(path);
            return encoding ? files.get(norm(path)) : Buffer.from(files.get(norm(path)));
        },
        writeFileSync(path, data) { files.set(norm(path), String(data)); },
        existsSync(path) { return files.has(norm(path)) || isDir(norm(path)); },
        statSync(path) {
            if (!this.existsSync(path)) throw missing(path);
            return { isDirectory: () => isDir(norm(path)) };
        },
        readdirSync() { return []; },
        mkdirSync() {},
    };
}

class StopEarly extends Error {}

function ownRunBundle(program, args, options) {
    const host = {
        args,
        fs: memoryFs(options.files),
        write(text) {
            if (options.stopAt && text.startsWith(options.stopAt)) throw new StopEarly();
            options.onOutput(text);
        },
    };
    let ok = true;
    let error = '';
    try {
        program(host, undefined, undefined, Buffer);
    } catch (e) {
        if (e instanceof StopEarly) {
            // Asked for.
        } else if (e && e.kind === 'Exit') {
            const message = String(e.omgMessage);
            const status = /^(-?\d+)$/.exec(message) || /(?:^|\n)Exit: (-?\d+)\s*$/.exec(message);
            ok = status ? Number(status[1]) === 0 : false;
            if (!ok) error = message;
        } else if (e && e.kind !== undefined && e.omgMessage !== undefined) {
            ok = false;
            error = e.kind + ': ' + e.omgMessage;
        } else {
            ok = false;
            error = 'Internal error: ' + (e && e.message ? e.message : String(e));
        }
    }
    return { ok, error };
}

let compileBundle = ownCompileBundle;
let runBundle = ownRunBundle;
if (workerPath) {
    // The site's file is a classic script that exports under Node when
    // it finds `module`. Hand it one, whatever the package around it
    // says about module types.
    const mod = { exports: {} };
    new Function('module', readFileSync(workerPath, 'utf8'))(mod);
    if (typeof mod.exports.compileBundle !== 'function' || typeof mod.exports.runBundle !== 'function') {
        console.log(`${workerPath} does not export compileBundle and runBundle.`);
        process.exit(1);
    }
    compileBundle = mod.exports.compileBundle;
    runBundle = mod.exports.runBundle;
    console.log('Loading and running through ' + workerPath);
}

function run(program, args, files, stopAt) {
    let out = '';
    const result = runBundle(program, args, { files, stopAt, onOutput: (text) => { out += text; } });
    return { out, ok: result.ok, error: result.error };
}

// ---------------------------------------------------------------------------
// The checks.
// ---------------------------------------------------------------------------

let failed = 0;
let count = 0;
function check(name, ok, detail) {
    count++;
    if (ok) {
        console.log('[OK]   ' + name);
    } else {
        failed++;
        console.log('[FAIL] ' + name);
        if (detail) console.log('       ' + String(detail).split('\n').join('\n       '));
    }
}

// 1. The five lines the site rewrites are in all three files.
const programs = {};
for (const name of Object.keys(TEXT)) {
    let problem = null;
    try {
        for (const [from] of PATCHES) {
            if (!TEXT[name].includes(from)) throw new Error('missing: ' + from);
        }
        if (name !== 'omg-rt.js') programs[name] = compileBundle(TEXT[name], name);
    } catch (e) {
        problem = e.message;
    }
    check(`${name}: has the runtime header the site rewrites`, problem === null, problem);
}
if (failed > 0) {
    console.log(`FAILED: ${failed} of ${count} checks`);
    process.exit(1);
}

const MODULE = ';;;omg\n\nproc twice(x) { return x * 2 }\n';
const SAMPLES = [
    { name: 'hello', src: ';;;omg\n\nemit "hi"\n', expect: 'hi\n' },
    { name: 'arith', src: ';;;omg\n\nemit 1 + 2 * 3\n', expect: '7\n' },
    { name: 'closure', expect: '15\n', src:
`;;;omg

proc make_adder(n) {
    proc add(x) { return x + n }
    return add
}
alloc add5 := make_adder(5)
emit add5(10)
` },
    // Float literals reach the back ends in a different form from the
    // one the VM runs; the C and JS stages used to fail on any of them.
    { name: 'floats', src: ';;;omg\n\nemit 1.5 + 2\nemit 0.1 + 0.2\n', expect: '3.5\n0.30000000000000004\n' },
    { name: 'dicts', src: ';;;omg\n\nalloc d := {b: 1, a: 2}\nemit d\nemit length(d)\n', expect: '{b: 1, a: 2}\n2\n' },
    { name: 'print', src: ';;;omg\n\nprint("a")\nprint("b\\n")\n', expect: 'ab\n' },
    // The page gives a run a file system of its own.
    { name: 'read_file', src: ';;;omg\n\nemit read_file("note.txt")\n', expect: 'from a file\n',
      files: { 'note.txt': 'from a file' } },
    // Not on the generated JavaScript: the explorer shows the JS for
    // the program alone, and an import is resolved when it is compiled.
    { name: 'import', src: ';;;omg\n\nimport "modules/m.omg" as m\nemit m.twice(21)\n', expect: '42\n',
      files: { 'modules/m.omg': MODULE }, generated: false },
    // The explorer's own run prints exit(0) as a traceback ending
    // "Exit: 0"; the site stops the explorer before its run and uses
    // the playground bundle to run, so only that is held to it here.
    { name: 'exit', src: ';;;omg\n\nemit "before"\nexit(0)\nemit "after"\n', expect: 'before\n',
      generated: false, explorerRun: false },
    { name: 'syntax_err', src: ';;;omg\n\nemit (((\n', error: /SyntaxError/ },
    { name: 'compile_err', src: ';;;omg\n\nimport "no_such.omg" as x\nemit 1\n', error: /no_such\.omg/ },
    { name: 'runtime_err', src: ';;;omg\n\nemit "start"\nemit 1 / 0\n', error: /ZeroDivision|division/i, generated: false },
];

const MARKER_RE = /___OMG_EXPLORER_STAGE___([a-z_]+)___OMG_EXPLORER_STAGE___\n?/g;
const STAGES = ['tokens', 'ast', 'bytecode', 'c', 'js', 'run', 'end'];

function stages(buf) {
    const out = {};
    const matches = [];
    let m;
    MARKER_RE.lastIndex = 0;
    while ((m = MARKER_RE.exec(buf)) !== null) {
        matches.push({ name: m[1], end: MARKER_RE.lastIndex, start: m.index });
    }
    for (let i = 0; i < matches.length; i++) {
        const next = matches[i + 1];
        out[matches[i].name] = buf.slice(matches[i].end, next ? next.start : buf.length);
    }
    return out;
}

for (const s of SAMPLES) {
    // 2. The playground: compile and run on the VM.
    const play = run(programs['omg-web.js'], ['<playground>', s.src], s.files);
    if (s.error) {
        check(`playground: ${s.name} reports its error`, !play.ok && s.error.test(play.error + play.out),
            `ok=${play.ok} error=${JSON.stringify(play.error)} output=${JSON.stringify(play.out)}`);
    } else {
        check(`playground: ${s.name}`, play.ok && play.out === s.expect,
            `ok=${play.ok} error=${JSON.stringify(play.error)}\nprinted  ${JSON.stringify(play.out)}\nexpected ${JSON.stringify(s.expect)}`);
    }

    // 3. The explorer: every stage framed by its marker, no stage
    // reporting an internal error, and the run at the end printing the
    // same as the playground.
    const ex = run(programs['omg-explorer.js'], ['<omg-explorer>', '<source>', s.src], s.files);
    const got = stages(ex.out);
    const brokenStage = ['c', 'js'].find((st) => /^(C-gen|JS-gen) error:/.test(got[st] || ''));
    if (s.error) {
        // A program that does not compile has no later stages; the
        // explorer still has to say why and come to its end.
        check(`explorer: ${s.name} reports its error`, s.error.test(ex.out + ex.error),
            `error=${JSON.stringify(ex.error)}\noutput=${JSON.stringify(ex.out.slice(-400))}`);
    } else {
        const missing = STAGES.filter((st) => got[st] === undefined);
        check(`explorer: ${s.name} has every stage`, ex.ok && missing.length === 0 && !brokenStage,
            `ok=${ex.ok} error=${JSON.stringify(ex.error)} missing=${missing.join(',')} ${brokenStage ? got[brokenStage].split('\n')[0] : ''}`);
        if (s.explorerRun !== false) {
            check(`explorer: ${s.name} runs`, got.run === s.expect,
                `printed  ${JSON.stringify(got.run)}\nexpected ${JSON.stringify(s.expect)}`);
        }
    }

    // 4. Stopping before the run, which is what the page asks for
    // when it only wants the stages.
    if (!s.error) {
        const early = run(programs['omg-explorer.js'], ['<omg-explorer>', '<source>', s.src], s.files,
            '___OMG_EXPLORER_STAGE___run___OMG_EXPLORER_STAGE___');
        const seen = stages(early.out);
        check(`explorer: ${s.name} stops before the run`, early.ok && seen.js !== undefined && seen.run === undefined,
            `ok=${early.ok} error=${JSON.stringify(early.error)}`);
    }

    // 5. The JavaScript the explorer generated, with the runtime put in
    // front of it, is a program that prints the same.
    if (!s.error && s.generated !== false && got.js !== undefined) {
        let problem = null;
        let js = { out: '', ok: false, error: '' };
        try {
            js = run(compileBundle(TEXT['omg-rt.js'] + '\n' + got.js, 'generated JavaScript'), ['<playground>'], s.files);
        } catch (e) {
            problem = e.message;
        }
        check(`generated JavaScript: ${s.name}`, problem === null && js.ok && js.out === s.expect,
            problem || `ok=${js.ok} error=${JSON.stringify(js.error)}\nprinted  ${JSON.stringify(js.out)}\nexpected ${JSON.stringify(s.expect)}`);
    }
}

console.log('---');
if (failed > 0) {
    console.log(`FAILED: ${failed} of ${count} checks`);
    process.exit(1);
}
console.log(`PASSED: all ${count} checks`);
