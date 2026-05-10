// Browser playground for OMG. Hosts the full compiler + VM + driver
// in a single ~1.4 MB JavaScript bundle (web/omg-web.js, built by
// `bootstrap/build-web.sh` via the native-js backend). Each Run click:
//
//   1. Sets globalThis.args = ["<playground>", <user source>]
//   2. Reroutes omg_emit / omg_print to write into the output pane
//   3. new Function(bundleSource)() — fresh evaluation per click so
//      the OMG-side globals are cleanly reset between runs.
//
// Note: the bundle's IIFE writes to file-scope `let` variables for
// every OMG global. Re-eval-ing the bundle (rather than calling an
// inner function) guarantees those start fresh each click. The cost
// is ~1.4 MB of JS to re-parse; V8's parse cache absorbs the repeat
// after the first run.

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

const $select   = document.getElementById('example');
const $source   = document.getElementById('source');
const $sourceHL = document.getElementById('source-hl');
const $output   = document.getElementById('output');
const $status   = document.getElementById('status');
const $run      = document.getElementById('run');

// Make the source editable now that we can compile any input.
$source.removeAttribute('readonly');

// === OMG syntax highlighting =============================================
// Mirrors the scopes in vscode/syntaxes/omg.tmLanguage.json. The
// tokenizer is intentionally line-by-line and forgiving: it never
// throws on malformed input, it just falls back to a plain ident
// token so the editor stays usable while the user is mid-typing.

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
    // Returns an array of {type, text}. `ws`/`ident`/`punct` carry
    // a falsy type so they emit as plain text in renderHighlight.
    const tokens = [];
    const n = src.length;
    let i = 0;
    while (i < n) {
        const ch = src[i];

        // Whitespace runs — pass through untouched.
        if (ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r') {
            let j = i;
            while (j < n && (src[j] === ' ' || src[j] === '\t' || src[j] === '\n' || src[j] === '\r')) j++;
            tokens.push({ text: src.slice(i, j) });
            i = j;
            continue;
        }

        // ;;;omg header (only at start of a line).
        if (ch === ';') {
            const lineStart = i === 0 || src[i - 1] === '\n';
            if (lineStart && src.slice(i, i + 6) === ';;;omg') {
                let j = i + 6;
                while (j < n && src[j] !== '\n') j++;
                tokens.push({ type: 'comment', text: src.slice(i, j) });
                i = j;
                continue;
            }
        }

        // # line comment to EOL.
        if (ch === '#') {
            let j = i;
            while (j < n && src[j] !== '\n') j++;
            tokens.push({ type: 'comment', text: src.slice(i, j) });
            i = j;
            continue;
        }

        // /* ... */ block comment (also catches /** docblocks).
        if (ch === '/' && src[i + 1] === '*') {
            let j = i + 2;
            while (j < n - 1 && !(src[j] === '*' && src[j + 1] === '/')) j++;
            j = Math.min(j + 2, n);
            tokens.push({ type: 'comment', text: src.slice(i, j) });
            i = j;
            continue;
        }

        // "..." string with \-escapes; stops at unescaped " or EOL
        // (so an unterminated string still highlights up to the
        // newline rather than swallowing the rest of the file).
        if (ch === '"') {
            let j = i + 1;
            while (j < n) {
                if (src[j] === '\\' && j + 1 < n) { j += 2; continue; }
                if (src[j] === '"') { j++; break; }
                if (src[j] === '\n') break;
                j++;
            }
            tokens.push({ type: 'string', text: src.slice(i, j) });
            i = j;
            continue;
        }

        // Numbers: 0bNNNN binary, or plain decimal.
        if (ch >= '0' && ch <= '9') {
            let j = i;
            if (ch === '0' && (src[i + 1] === 'b' || src[i + 1] === 'B')) {
                j = i + 2;
                while (j < n && (src[j] === '0' || src[j] === '1')) j++;
            } else {
                while (j < n && src[j] >= '0' && src[j] <= '9') j++;
            }
            tokens.push({ type: 'number', text: src.slice(i, j) });
            i = j;
            continue;
        }

        // Identifier — bucket into keyword / builtin / boolean /
        // langvar / fn (followed by `(`) / plain.
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
            i = j;
            continue;
        }

        // Multi-char operators first.
        const two = src.slice(i, i + 2);
        if (two === ':=' || two === '==' || two === '!=' ||
            two === '<=' || two === '>=' || two === '<<' || two === '>>') {
            tokens.push({ type: 'op', text: two });
            i += 2;
            continue;
        }
        if ('+-*/%<>=&|^~'.indexOf(ch) >= 0) {
            tokens.push({ type: 'op', text: ch });
            i++;
            continue;
        }

        // Punctuation / unrecognised — pass through plain.
        tokens.push({ text: ch });
        i++;
    }
    return tokens;
}

function escapeHTML(s) {
    return s
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;');
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
    // A trailing newline in a <pre> collapses to zero height; pad
    // with a space so the highlight layer always has the same line
    // count as the textarea.
    if (src.endsWith('\n')) html += ' ';
    $sourceHL.innerHTML = html;
}

function syncEditorScroll() {
    $sourceHL.scrollTop = $source.scrollTop;
    $sourceHL.scrollLeft = $source.scrollLeft;
}

$source.addEventListener('input', renderHighlight);
$source.addEventListener('scroll', syncEditorScroll);

// === Example dropdown ====================================================

// Populate dropdown.
STARTERS.forEach((s, i) => {
    const o = document.createElement('option');
    o.value = i;
    o.textContent = s.name;
    $select.appendChild(o);
});
$select.addEventListener('change', () => {
    $source.value = STARTERS[$select.value].src;
    renderHighlight();
});
$source.value = STARTERS[0].src;
renderHighlight();

// Load the bundle text once, eval per click.
let bundleSource = null;
async function loadBundle() {
    $status.textContent = 'loading bundle…';
    try {
        bundleSource = await fetch('omg-web.js').then(r => r.text());
        $status.textContent = '';
    } catch (e) {
        $status.textContent = 'bundle load failed: ' + e.message;
    }
}
loadBundle();

function runUserSource(src) {
    if (!bundleSource) { $status.textContent = 'bundle not yet loaded'; return; }
    $output.classList.remove('error');
    $output.textContent = '';

    let buf = '';
    // The IIFE in the bundle is wrapped in a try/catch that calls
    // process.exit on uncaught OmgError. In the browser process is
    // undefined, so the catch rethrows — we catch it here.
    //
    // Override args + emit/print sinks via the same find-replace
    // dance the static playground used. The bundle's omg_rt.js
    // declares them as `let` so this works.
    const wrapped =
        bundleSource
            .replace(
                /let args = \(typeof process !== 'undefined'\) \? process\.argv\.slice\(1\) : \[\];/,
                'let args = ["<playground>", ' + JSON.stringify(src) + '];'
            )
            .replace(
                /let v_args = args;/,
                'let v_args = args;'
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
    const t0 = performance.now();
    try {
        // eslint-disable-next-line no-new-func
        new Function(wrapped)();
    } catch (e) {
        if (e && e.kind && e.omgMessage !== undefined) {
            if (e.kind !== 'Exit') {
                window.__omg_buf += '\n' + e.kind + ': ' + e.omgMessage + '\n';
                $output.classList.add('error');
            }
        } else {
            window.__omg_buf += '\n[playground error] ' + e.message + '\n';
            $output.classList.add('error');
        }
    }
    const ms = (performance.now() - t0).toFixed(0);
    $output.textContent = window.__omg_buf || '(no output)';
    $status.textContent = ms + ' ms';
}

$run.addEventListener('click', () => runUserSource($source.value));

// Run hello_world on load so the page has something to show.
window.addEventListener('DOMContentLoaded', () => {
    setTimeout(() => {
        if (bundleSource) runUserSource($source.value);
        else loadBundle().then(() => runUserSource($source.value));
    }, 100);
});
