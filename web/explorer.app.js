// Compiler Explorer frontend for OMG.
//
// What loads here:
//   web/omg-explorer.js — the OMG-side driver (bootstrap/src/omg-explorer
//   .omg) transpiled to JS. The bundle is an IIFE that reads args from
//   `let args = ...`, runs the user source through every compiler stage,
//   and emits each result framed by a `___OMG_EXPLORER_STAGE___<name>___
//   OMG_EXPLORER_STAGE___` marker via omg_emit → _omg_write.
//
// What this file does:
//   - Loads the bundle text once, re-evals it on each Explore click so
//     the OMG-side globals are cleanly reset.
//   - Before eval, find-replaces the args declaration to inject
//     ["<omg-explorer>", "<source>", userSourceText] and rewires
//     _omg_write to accumulate into window.__omg_buf instead of
//     console.log.
//   - Splits the captured buffer on the stage markers and pours each
//     section into its tab pane.
//   - Marks tabs in error (lex/parse/compile failures) so the user can
//     see at a glance which stage failed.

// === Stage metadata =====================================================

const STAGES = ['tokens', 'ast', 'bytecode', 'c', 'js', 'run'];

const MARKER_RE = /___OMG_EXPLORER_STAGE___([a-z_]+)___OMG_EXPLORER_STAGE___\n?/g;

// === Starter examples ====================================================

const STARTERS = [
    {
        name: 'hello_world',
        src:
`;;;omg

emit "Hello, world!"
`
    },
    {
        name: 'arithmetic',
        src:
`;;;omg

# Watch the binary op lower from (bin add ...) in the AST
# to a single Add opcode in the bytecode.
emit 1 + 2 * 3
`
    },
    {
        name: 'closures',
        src:
`;;;omg

# Closures show up as MakeFunc + StoreLocal in the bytecode;
# inspect the C tab to see how captures are boxed.
proc make_adder(n) {
    proc add(x) {
        return x + n
    }
    return add
}

alloc add5 := make_adder(5)
emit add5(10)
emit add5(100)
`
    },
    {
        name: 'fibonacci',
        src:
`;;;omg

proc fib(n) {
    if n < 2 { return n }
    return fib(n - 1) + fib(n - 2)
}

alloc i := 0
loop i < 10 {
    emit "fib(" + i + ") = " + fib(i)
    i := i + 1
}
`
    },
    {
        name: 'try_except',
        src:
`;;;omg

# SETUP_EXCEPT + POP_BLOCK + RAISE are the bytecode primitives
# that implement try/except. See them on stage iii.
try {
    alloc xs := [1, 2, 3]
    emit xs[99]
} except err {
    emit "caught: " + err
}
`
    },
    {
        name: 'prime_sieve',
        src:
`;;;omg

alloc N := 30
alloc sieve := []
alloc i := 0
loop i <= N {
    sieve := sieve + [true]
    i := i + 1
}

alloc p := 2
loop p * p <= N {
    if sieve[p] {
        alloc m := p * p
        loop m <= N {
            sieve[m] := false
            m := m + p
        }
    }
    p := p + 1
}

alloc primes := []
alloc k := 2
loop k <= N {
    if sieve[k] { primes := primes + [k] }
    k := k + 1
}
emit primes
`
    },
];

// === DOM handles =========================================================

const $select   = document.getElementById('example');
const $source   = document.getElementById('source');
const $sourceHL = document.getElementById('source-hl');
const $gutter   = document.getElementById('source-gutter');
const $run      = document.getElementById('run');
const $share    = document.getElementById('share');
const $sourceMeta = document.getElementById('sourceMeta');
const $sourcePane = document.getElementById('sourcePane');

const $statusLabel  = document.getElementById('statusLabel');
const $statusDot    = document.getElementById('statusDot');
const $statusDetail = document.getElementById('statusDetail');
const $bundleSize   = document.getElementById('bundleSize');

const $tabBar   = document.getElementById('tabBar');
const $pipeline = document.getElementById('pipeline');

const $panes = {};
const $tabs  = {};
const $tabSizes = {};
const $psteps = {};
for (const s of STAGES) {
    $panes[s] = document.getElementById('pane-' + s);
    $tabs[s]  = $tabBar.querySelector(`.tab[data-stage="${s}"]`);
    $tabSizes[s] = $tabs[s].querySelector('[data-size]');
    $psteps[s] = $pipeline.querySelector(`.pstep[data-stage="${s}"]`);
}

// === Syntax highlighting (mirrors app.js) ================================

const OMG_KEYWORDS = new Set([
    'if', 'elif', 'else', 'loop', 'break', 'return',
    'try', 'except', 'facts',
    'alloc', 'proc', 'import', 'as', 'emit',
    'and', 'or'
]);

const OMG_BUILTINS = new Set([
    'length', 'chr', 'ascii', 'hex', 'binary', 'freeze',
    'panic', 'raise',
    'read_file', 'file_exists', 'file_open', 'file_read',
    'file_write', 'file_close', 'call_builtin'
]);

const OMG_LANGVARS = new Set(['args', 'module_file', 'current_dir']);

function omgTokenize(src) {
    const tokens = [];
    const n = src.length;
    let i = 0;
    while (i < n) {
        const ch = src[i];
        if (ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r') {
            let j = i;
            while (j < n && (src[j] === ' ' || src[j] === '\t' || src[j] === '\n' || src[j] === '\r')) j++;
            tokens.push({ text: src.slice(i, j) });
            i = j; continue;
        }
        if (ch === ';') {
            const lineStart = i === 0 || src[i - 1] === '\n';
            if (lineStart && src.slice(i, i + 6) === ';;;omg') {
                let j = i + 6;
                while (j < n && src[j] !== '\n') j++;
                tokens.push({ type: 'comment', text: src.slice(i, j) });
                i = j; continue;
            }
        }
        if (ch === '#') {
            let j = i;
            while (j < n && src[j] !== '\n') j++;
            tokens.push({ type: 'comment', text: src.slice(i, j) });
            i = j; continue;
        }
        if (ch === '/' && src[i + 1] === '*') {
            let j = i + 2;
            while (j < n - 1 && !(src[j] === '*' && src[j + 1] === '/')) j++;
            j = Math.min(j + 2, n);
            tokens.push({ type: 'comment', text: src.slice(i, j) });
            i = j; continue;
        }
        if (ch === '"') {
            let j = i + 1;
            while (j < n) {
                if (src[j] === '\\' && j + 1 < n) { j += 2; continue; }
                if (src[j] === '"') { j++; break; }
                if (src[j] === '\n') break;
                j++;
            }
            tokens.push({ type: 'string', text: src.slice(i, j) });
            i = j; continue;
        }
        if (ch >= '0' && ch <= '9') {
            let j = i;
            if (ch === '0' && (src[i + 1] === 'b' || src[i + 1] === 'B')) {
                j = i + 2;
                while (j < n && (src[j] === '0' || src[j] === '1')) j++;
            } else {
                while (j < n && src[j] >= '0' && src[j] <= '9') j++;
            }
            tokens.push({ type: 'number', text: src.slice(i, j) });
            i = j; continue;
        }
        if ((ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || ch === '_') {
            let j = i;
            while (j < n && /[A-Za-z0-9_]/.test(src[j])) j++;
            const word = src.slice(i, j);
            let type;
            if (OMG_KEYWORDS.has(word)) type = 'keyword';
            else if (OMG_BUILTINS.has(word)) type = 'builtin';
            else if (word === 'true' || word === 'false') type = 'boolean';
            else if (OMG_LANGVARS.has(word)) type = 'langvar';
            else {
                let k = j;
                while (k < n && (src[k] === ' ' || src[k] === '\t')) k++;
                if (src[k] === '(') type = 'fn';
            }
            tokens.push({ type, text: word });
            i = j; continue;
        }
        const two = src.slice(i, i + 2);
        if (two === ':=' || two === '==' || two === '!=' ||
            two === '<=' || two === '>=' || two === '<<' || two === '>>') {
            tokens.push({ type: 'op', text: two });
            i += 2; continue;
        }
        if ('+-*/%<>=&|^~'.indexOf(ch) >= 0) {
            tokens.push({ type: 'op', text: ch });
            i++; continue;
        }
        tokens.push({ text: ch });
        i++;
    }
    return tokens;
}

function escapeHTML(s) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function renderHighlight() {
    const src = $source.value;
    const tokens = omgTokenize(src);
    let html = '';
    for (const t of tokens) {
        const safe = escapeHTML(t.text);
        if (t.type) html += '<span class="tok-' + t.type + '">' + safe + '</span>';
        else html += safe;
    }
    if (src.endsWith('\n')) html += ' ';
    $sourceHL.innerHTML = html;
    updateGutter(src);
    updateSourceMeta(src);
}

function updateGutter(src) {
    const lines = src.split('\n').length;
    if (updateGutter._last === lines) return;
    updateGutter._last = lines;
    let g = '';
    for (let i = 1; i <= lines; i++) g += i + '\n';
    $gutter.textContent = g;
}

function updateSourceMeta(src) {
    const lines = src.split('\n').length;
    $sourceMeta.textContent = lines + ' line' + (lines === 1 ? '' : 's') + ' · ' + src.length + ' chars';
}

function syncEditorScroll() {
    $sourceHL.scrollTop = $source.scrollTop;
    $sourceHL.scrollLeft = $source.scrollLeft;
    $gutter.scrollTop = $source.scrollTop;
}

$source.addEventListener('input', () => {
    renderHighlight();
    if (typeof OMGShare !== 'undefined' && OMGShare.storeSource) {
        OMGShare.storeSource($source.value);
    }
});
$source.addEventListener('scroll', syncEditorScroll);
$source.addEventListener('focus', () => $sourcePane.classList.add('is-focused'));
$source.addEventListener('blur',  () => $sourcePane.classList.remove('is-focused'));

// === Example dropdown ====================================================
// Two groups:
//   - "Starters": the inlined STARTERS above (zero fetch — always works
//     even if examples/ wasn't built).
//   - "Examples": the full set under examples/*.omg, listed in
//     examples/manifest.json and fetched lazily on selection.

const $starterGroup = document.createElement('optgroup');
$starterGroup.label = 'Starters';
STARTERS.forEach((s, i) => {
    const o = document.createElement('option');
    o.value = 'starter:' + i;
    o.textContent = s.name;
    $starterGroup.appendChild(o);
});
$select.appendChild($starterGroup);

const $exampleGroup = document.createElement('optgroup');
$exampleGroup.label = 'Examples';
$select.appendChild($exampleGroup);

async function loadExampleSource(name) {
    const res = await fetch('examples/' + encodeURIComponent(name) + '.omg');
    if (!res.ok) throw new Error('HTTP ' + res.status);
    return await res.text();
}

const exampleManifestPromise = fetch('examples/manifest.json')
    .then((r) => r.ok ? r.json() : [])
    .catch(() => [])
    .then((names) => {
        for (const name of names) {
            const o = document.createElement('option');
            o.value = 'example:' + name;
            o.textContent = name;
            $exampleGroup.appendChild(o);
        }
    });

function persistCurrent() {
    if (typeof OMGShare !== 'undefined' && OMGShare.storeSource) {
        OMGShare.storeSource($source.value);
    }
}

$select.addEventListener('change', async () => {
    const v = $select.value;
    if (v.startsWith('starter:')) {
        const i = parseInt(v.slice('starter:'.length), 10);
        $source.value = STARTERS[i].src;
        renderHighlight();
        persistCurrent();
        runExplorer();
    } else if (v.startsWith('example:')) {
        const name = v.slice('example:'.length);
        try {
            $source.value = await loadExampleSource(name);
        } catch (err) {
            $source.value = '# Failed to load examples/' + name + '.omg: ' + err.message + '\n';
        }
        renderHighlight();
        persistCurrent();
        runExplorer();
    }
});

// Initial source priority: localStorage > STARTERS[0]. URL `#code=...`
// resolves asynchronously below and wins if present.
const storedSource = (typeof OMGShare !== 'undefined' && OMGShare.loadStoredSource)
    ? OMGShare.loadStoredSource() : null;
if (storedSource !== null) {
    $source.value = storedSource;
    $select.selectedIndex = -1;
} else {
    $source.value = STARTERS[0].src;
}
renderHighlight();

// If `#code=...` is in the URL, replace the default source with it before
// the first auto-explore.
const sharedSourcePromise = (typeof OMGShare !== 'undefined' && OMGShare.readSharedSource)
    ? OMGShare.readSharedSource().then((shared) => {
        if (shared !== null && shared !== undefined) {
            $source.value = shared;
            renderHighlight();
            $select.selectedIndex = -1;
            persistCurrent();
        }
      })
    : Promise.resolve();

// === Tab management ======================================================

let currentStage = 'tokens';
function selectStage(name) {
    if (!STAGES.includes(name)) return;
    currentStage = name;
    for (const s of STAGES) {
        $tabs[s].classList.toggle('is-active', s === name);
        $panes[s].classList.toggle('is-active', s === name);
        $psteps[s].classList.toggle('is-current', s === name);
    }
}

$tabBar.addEventListener('click', (e) => {
    const t = e.target.closest('.tab');
    if (!t) return;
    selectStage(t.getAttribute('data-stage'));
});
$pipeline.addEventListener('click', (e) => {
    const p = e.target.closest('.pstep');
    if (!p) return;
    selectStage(p.getAttribute('data-stage'));
});

// === Status helpers ======================================================

function setStatus(state, label, detail) {
    $statusLabel.textContent = label;
    $statusDetail.textContent = detail || '';
    $statusDot.classList.remove('is-busy', 'is-ok', 'is-error');
    const $row = $statusLabel.parentElement;
    $row.classList.remove('sb-accent', 'sb-warn', 'sb-error', 'sb-ok');
    if (state === 'busy') { $statusDot.classList.add('is-busy'); $row.classList.add('sb-accent'); }
    if (state === 'ok')   { $statusDot.classList.add('is-ok');   $row.classList.add('sb-ok'); }
    if (state === 'warn') { $row.classList.add('sb-warn'); }
    if (state === 'err')  { $statusDot.classList.add('is-error'); $row.classList.add('sb-error'); }
}

function fmtBytes(n) {
    if (n < 1024) return n + ' B';
    if (n < 1024 * 1024) return (n / 1024).toFixed(1) + ' KB';
    return (n / (1024 * 1024)).toFixed(2) + ' MB';
}

// === Bundle loader =======================================================

let bundleSource = null;
async function loadBundle() {
    setStatus('busy', 'fetching', 'omg-explorer.js');
    try {
        const t0 = performance.now();
        const r = await fetch('omg-explorer.js');
        bundleSource = await r.text();
        const ms = (performance.now() - t0).toFixed(0);
        $bundleSize.textContent = fmtBytes(bundleSource.length);
        setStatus('ok', 'ready', 'bundle · ' + ms + ' ms');
    } catch (e) {
        setStatus('err', 'load failed', e.message);
    }
}
loadBundle();

// === Stage parsing =======================================================

function parseStageBuffer(buf) {
    const out = {};
    MARKER_RE.lastIndex = 0;
    const matches = [];
    let m;
    while ((m = MARKER_RE.exec(buf)) !== null) {
        matches.push({ name: m[1], start: m.index, end: MARKER_RE.lastIndex });
    }
    for (let i = 0; i < matches.length; i++) {
        const cur = matches[i];
        const next = matches[i + 1];
        const sliceEnd = next ? next.start : buf.length;
        const content = buf.slice(cur.end, sliceEnd);
        out[cur.name] = content.replace(/\n$/, '');
    }
    return out;
}

function isStageError(s) {
    const t = s.trim();
    return (
        t.startsWith('lex error:') ||
        t.startsWith('parse error:') ||
        t.startsWith('compile error:') ||
        t.startsWith('C-gen error:') ||
        t.startsWith('JS-gen error:') ||
        t.startsWith('(skipped — ')
    );
}

function renderStage(name, content) {
    const $pane = $panes[name];
    const $tab  = $tabs[name];
    const $pstep = $psteps[name];
    const $size = $tabSizes[name];
    if (!$pane) return;

    $pane.classList.remove('is-error', 'is-empty');
    $tab.classList.remove('is-error', 'is-empty');
    $pstep.classList.remove('is-error', 'is-empty');
    $size.textContent = '';

    if (content === undefined || content === '') {
        $pane.textContent = '(no output)';
        $pane.classList.add('is-empty');
        $tab.classList.add('is-empty');
        $pstep.classList.add('is-empty');
        return;
    }
    if (isStageError(content)) {
        $pane.classList.add('is-error');
        $tab.classList.add('is-error');
        $pstep.classList.add('is-error');
    }
    $size.textContent = fmtBytes(content.length);
    $pane.textContent = content;
}

// === Run the explorer ===================================================

function runExplorer() {
    if (!bundleSource) { setStatus('warn', 'pending', 'bundle still loading'); return; }
    const src = $source.value;

    const wrapped =
        bundleSource
            .replace(
                /let args = \(typeof process !== 'undefined'\) \? process\.argv\.slice\(1\) : \[\];/,
                'let args = ["<omg-explorer>", "<source>", ' + JSON.stringify(src) + '];'
            )
            .replace(
                "let _omg_write = (s) => { console.log(s); };",
                "let _omg_write = (s) => { window.__omg_buf += String(s) + '\\n'; };"
            )
            .replace(
                "let _omg_print_raw = (s) => { process.stdout.write(s); };",
                "let _omg_print_raw = (s) => { window.__omg_buf += String(s); };"
            )
            .replace(
                "let _omg_exit = (code) => { if (typeof process !== 'undefined') process.exit(Number(code)); };",
                "let _omg_exit = (code) => { window.__omg_exit_code = Number(code); throw new OmgError('Exit', 'exit ' + code); };"
            );

    window.__omg_buf = '';
    window.__omg_exit_code = 0;
    setStatus('busy', 'exploring', '');
    const t0 = performance.now();
    let fatalError = null;
    try {
        // eslint-disable-next-line no-new-func
        new Function(wrapped)();
    } catch (e) {
        if (e && e.kind && e.omgMessage !== undefined) {
            if (e.kind !== 'Exit') {
                fatalError = e.kind + ': ' + e.omgMessage;
            }
        } else {
            fatalError = '[explorer] ' + (e && e.message ? e.message : String(e));
        }
    }
    const ms = (performance.now() - t0).toFixed(0);

    const sections = parseStageBuffer(window.__omg_buf || '');
    for (const s of STAGES) {
        renderStage(s, sections[s]);
    }
    const finishedCleanly = sections.end !== undefined;

    if (fatalError) {
        setStatus('err', 'aborted', fatalError.slice(0, 80));
    } else if (!finishedCleanly) {
        setStatus('err', 'incomplete', 'driver exited early');
    } else {
        setStatus('ok', 'done', ms + ' ms');
    }

    // If the active stage is empty, jump to the first errored stage so
    // failures aren't hidden behind a happy default tab.
    const activeIsEmpty = $panes[currentStage].classList.contains('is-empty');
    if (activeIsEmpty) {
        const firstError = STAGES.find(s => $tabs[s].classList.contains('is-error'));
        if (firstError) selectStage(firstError);
    }
}

$run.addEventListener('click', runExplorer);

$share.addEventListener('click', async () => {
    try {
        await OMGShare.copyShareLink($source.value);
        OMGShare.showToast('Link copied to clipboard');
    } catch (e) {
        OMGShare.showToast('Could not copy link: ' + e.message, 'error');
    }
});

// Run on load.
window.addEventListener('DOMContentLoaded', () => {
    selectStage('tokens');
    setTimeout(async () => {
        await sharedSourcePromise;
        if (bundleSource) runExplorer();
        else loadBundle().then(() => runExplorer());
    }, 80);
});

// Keyboard: Ctrl/Cmd+Enter to Explore, 1-6 (when not editing) for tabs.
document.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
        e.preventDefault();
        runExplorer();
        return;
    }
    if (document.activeElement === $source) return;
    const idx = '123456'.indexOf(e.key);
    if (idx >= 0) selectStage(STAGES[idx]);
});
