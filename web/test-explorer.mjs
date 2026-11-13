// Smoke test for the Compiler Explorer pipeline. Mirrors the browser
// frontend's bundle-eval path (args injection + _omg_write rewire),
// then splits the output buffer on stage markers and prints per-stage
// summaries. Run from the omglang root after
// `bash scripts/build-web.sh`:
//
//     node web/test-explorer.mjs
//
// Failures here mean either (a) the OMG-side driver isn't producing
// the marker-framed output the frontend expects, or (b) the
// find-replace strings in explorer.app.js no longer match the JS
// runtime's source. Either way: fix before reaching the browser.

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const bundlePath = resolve('web/omg-explorer.js');
const bundle = readFileSync(bundlePath, 'utf8');

const SAMPLES = [
    { name: 'hello',     src: ';;;omg\n\nemit "hi"\n' },
    { name: 'arith',     src: ';;;omg\n\nemit 1 + 2 * 3\n' },
    { name: 'closure',   src:
`;;;omg

proc make_adder(n) {
    proc add(x) { return x + n }
    return add
}
alloc add5 := make_adder(5)
emit add5(10)
` },
    // Float literals reach the transpilers in a different form from the
    // one the VM runs; the C and JS stages used to fail on any of them.
    { name: 'floats',    src: ';;;omg\n\nemit 1.5 + 2\nemit 0.1 + 0.2\n', expectRun: '3.5\n0.30000000000000004' },
    { name: 'dicts',     src: ';;;omg\n\nalloc d := {b: 1, a: 2}\nemit d\nemit length(d)\n', expectRun: '{b: 1, a: 2}\n2' },
    { name: 'syntax_err', src: ';;;omg\n\nemit (((\n' },
    { name: 'compile_err', src: ';;;omg\n\nimport "no_such.omg" as x\nemit 1\n' },
];

const MARKER_RE = /___OMG_EXPLORER_STAGE___([a-z_]+)___OMG_EXPLORER_STAGE___\n?/g;
const STAGES = ['tokens', 'ast', 'bytecode', 'c', 'js', 'run', 'end'];

function parse(buf) {
    const out = {};
    MARKER_RE.lastIndex = 0;
    const matches = [];
    let m;
    while ((m = MARKER_RE.exec(buf)) !== null) {
        matches.push({ name: m[1], end: MARKER_RE.lastIndex, start: m.index });
    }
    for (let i = 0; i < matches.length; i++) {
        const cur = matches[i];
        const next = matches[i + 1];
        const sliceEnd = next ? next.start : buf.length;
        out[cur.name] = buf.slice(cur.end, sliceEnd).replace(/\n$/, '');
    }
    return out;
}

function runSample(s) {
    const wrapped =
        bundle
            .replace(
                /let args = \(typeof process !== 'undefined'\) \? process\.argv\.slice\(1\) : \[\];/,
                'let args = ["<omg-explorer>", "<source>", ' + JSON.stringify(s.src) + '];'
            )
            .replace(
                "let _omg_write = (s) => { console.log(s); };",
                "let _omg_write = (s) => { globalThis.__omg_buf += String(s) + '\\n'; };"
            )
            .replace(
                "let _omg_print_raw = (s) => { process.stdout.write(s); };",
                "let _omg_print_raw = (s) => { globalThis.__omg_buf += String(s); };"
            )
            .replace(
                "let _omg_exit = (code) => { if (typeof process !== 'undefined') process.exit(Number(code)); };",
                "let _omg_exit = (code) => { globalThis.__omg_exit_code = Number(code); throw new OmgError('Exit', 'exit ' + code); };"
            );

    globalThis.__omg_buf = '';
    globalThis.__omg_exit_code = 0;
    const t0 = performance.now();
    let fatal = null;
    try {
        new Function(wrapped)();
    } catch (e) {
        if (e && e.kind && e.omgMessage !== undefined) {
            if (e.kind !== 'Exit') fatal = e.kind + ': ' + e.omgMessage;
        } else fatal = String(e && e.message ? e.message : e);
    }
    const ms = (performance.now() - t0).toFixed(0);
    return { sections: parse(globalThis.__omg_buf), ms, fatal, raw: globalThis.__omg_buf };
}

let failed = 0;
for (const s of SAMPLES) {
    const r = runSample(s);
    const present = STAGES.filter(st => r.sections[st] !== undefined);
    const missing = STAGES.filter(st => r.sections[st] === undefined);
    const hasEnd = r.sections.end !== undefined;
    const ok = !r.fatal && hasEnd;
    console.log('---');
    console.log(`[${ok ? 'OK' : 'FAIL'}] ${s.name} · ${r.ms} ms`);
    if (r.fatal) {
        console.log('  fatal: ' + r.fatal);
        failed++;
    }
    console.log('  present: ' + present.join(', '));
    if (missing.length) console.log('  missing: ' + missing.join(', '));
    for (const st of ['tokens', 'ast', 'bytecode', 'c', 'js', 'run']) {
        const v = r.sections[st];
        if (v === undefined) continue;
        const firstLine = v.split('\n').find(l => l.trim().length > 0) || '(blank)';
        console.log(`  ${st.padEnd(8)}: ${v.length} chars · "${firstLine.slice(0, 70)}"`);
    }
    // A stage that caught an internal error still produces framed
    // output, so check for the error markers explicitly, and compare
    // the run stage against the expected output where one is given.
    let stageError = null;
    for (const st of ['c', 'js']) {
        const v = r.sections[st] || '';
        if (/^(C-gen|JS-gen) error:/.test(v)) stageError = `${st} stage: ${v.split('\n')[0]}`;
    }
    if (s.expectRun !== undefined && r.sections.run !== s.expectRun) {
        stageError = `run stage printed ${JSON.stringify(r.sections.run)}, expected ${JSON.stringify(s.expectRun)}`;
    }
    if (stageError) console.log('  error: ' + stageError);
    if (!ok || stageError) failed++;
}

console.log('---');
if (failed > 0) {
    console.log(`FAILED: ${failed} sample(s) had errors`);
    process.exit(1);
} else {
    console.log(`PASSED: all ${SAMPLES.length} samples produced framed output`);
}
