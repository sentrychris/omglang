// Browser playground for OMG.
//
// Hosts the meta-circular compiler + VM + driver in a single
// ~1.8 MB JavaScript bundle (web/omg-web.js, built by
// bootstrap/build-web.sh via the native-js backend). Each Run click:
//
//   1. Sets globalThis.args = ["<playground>", <user source>]
//   2. Reroutes omg_emit / omg_print to write into the output pane
//   3. new Function(bundleSource)() — fresh evaluation per click so
//      the OMG-side globals are cleanly reset between runs.

const STARTERS = [
    {
        name: 'hello_world',
        src:
`;;;omg

emit "Hello, world!"
`
    },
    {
        name: 'closures',
        src:
`;;;omg

# Each call to make_adder produces an "add" that remembers its own n.
proc make_adder(n) {
    proc add(x) {
        return x + n
    }
    return add
}

alloc add5 := make_adder(5)
alloc add100 := make_adder(100)

emit add5(10)         # 15
emit add100(7)        # 107
emit add5(add100(0))  # 105
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
        name: 'prime_sieve',
        src:
`;;;omg

# Sieve of Eratosthenes up to 100.

alloc N := 100
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
    {
        name: 'classify',
        src:
`;;;omg

proc digit_sum(n) {
    alloc s := 0
    loop n > 0 {
        s := s + n % 10
        n := n / 10
    }
    return s
}

proc is_prime(n) {
    if n <= 1 { return false }
    alloc i := 2
    loop i * i <= n {
        if n % i == 0 { return false }
        i := i + 1
    }
    return true
}

alloc n := 13
emit "n = " + n
emit "digit_sum = " + digit_sum(n)
emit "prime = " + is_prime(n)
`
    },
];

// === DOM handles =========================================================

const $select   = document.getElementById('example');
const $source   = document.getElementById('source');
const $sourceHL = document.getElementById('source-hl');
const $gutter   = document.getElementById('source-gutter');
const $output   = document.getElementById('output');
const $run      = document.getElementById('run');
const $share    = document.getElementById('share');

const $sourceMeta   = document.getElementById('sourceMeta');
const $outputMeta   = document.getElementById('outputMeta');
const $statusLabel  = document.getElementById('statusLabel');
const $statusDot    = document.getElementById('statusDot');
const $statusDetail = document.getElementById('statusDetail');
const $bundleSize   = document.getElementById('bundleSize');
const $sourcePane   = document.getElementById('sourcePane');

// === OMG syntax highlighting =============================================
// Mirrors the scopes in vscode/syntaxes/omg.tmLanguage.json.

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
        runUserSource($source.value);
    } else if (v.startsWith('example:')) {
        const name = v.slice('example:'.length);
        try {
            $source.value = await loadExampleSource(name);
        } catch (err) {
            $source.value = '# Failed to load examples/' + name + '.omg: ' + err.message + '\n';
        }
        renderHighlight();
        persistCurrent();
        runUserSource($source.value);
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

// If the URL carries a `#code=...` fragment, replace the default source
// with the shared program before the first auto-run.
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
    setStatus('busy', 'fetching', 'omg-web.js');
    try {
        const t0 = performance.now();
        const r = await fetch('omg-web.js');
        bundleSource = await r.text();
        const ms = (performance.now() - t0).toFixed(0);
        $bundleSize.textContent = fmtBytes(bundleSource.length);
        setStatus('ok', 'ready', 'bundle · ' + ms + ' ms');
    } catch (e) {
        setStatus('err', 'load failed', e.message);
    }
}
loadBundle();

// === Run =================================================================

function runUserSource(src) {
    if (!bundleSource) { setStatus('warn', 'pending', 'bundle still loading'); return; }
    $output.classList.remove('is-error', 'is-empty');
    $output.textContent = '';

    const wrapped =
        bundleSource
            .replace(
                /let args = \(typeof process !== 'undefined'\) \? process\.argv\.slice\(1\) : \[\];/,
                'let args = ["<playground>", ' + JSON.stringify(src) + '];'
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
    setStatus('busy', 'running', '');
    const t0 = performance.now();
    let failed = false;
    try {
        // eslint-disable-next-line no-new-func
        new Function(wrapped)();
    } catch (e) {
        if (e && e.kind && e.omgMessage !== undefined) {
            if (e.kind !== 'Exit') {
                window.__omg_buf += '\n' + e.kind + ': ' + e.omgMessage + '\n';
                failed = true;
            }
        } else {
            window.__omg_buf += '\n[playground error] ' + e.message + '\n';
            failed = true;
        }
    }
    const ms = (performance.now() - t0).toFixed(0);
    const out = window.__omg_buf;

    if (failed) {
        $output.classList.add('is-error');
        $output.textContent = out;
        setStatus('err', 'failed', ms + ' ms');
    } else if (!out) {
        $output.classList.add('is-empty');
        $output.textContent = '(no output)';
        setStatus('ok', 'done', ms + ' ms · no output');
        $outputMeta.textContent = '—';
    } else {
        $output.textContent = out;
        const lines = out.split('\n').length - (out.endsWith('\n') ? 1 : 0);
        setStatus('ok', 'done', ms + ' ms');
        $outputMeta.textContent = lines + ' line' + (lines === 1 ? '' : 's') + ' · ' + out.length + ' chars';
    }
}

$run.addEventListener('click', () => runUserSource($source.value));

// === Share ===============================================================

$share.addEventListener('click', async () => {
    try {
        await OMGShare.copyShareLink($source.value);
        OMGShare.showToast('Link copied to clipboard');
    } catch (e) {
        OMGShare.showToast('Could not copy link: ' + e.message, 'error');
    }
});

// === Keyboard ============================================================

document.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
        e.preventDefault();
        runUserSource($source.value);
    }
});

// === First run ===========================================================

window.addEventListener('DOMContentLoaded', () => {
    setTimeout(async () => {
        await sharedSourcePromise;
        if (bundleSource) runUserSource($source.value);
        else loadBundle().then(() => runUserSource($source.value));
    }, 80);
});
