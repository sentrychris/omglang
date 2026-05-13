// Renders the language reference sidebar and wires its toggle.
//
// Used by both the playground and the explorer. The reference content is
// declared as plain OMG snippets; this module tokenises them with a copy
// of the highlighter from app.js / explorer.app.js so the colours match
// the live editor.

(function () {
    const REFERENCE = [
        { kind: 'section', title: 'Syntax' },
        {
            title: 'Variables',
            code:
`alloc x := 10       # declare
x := x + 1          # reassign`
        },
        {
            title: 'Numbers & math',
            code:
`alloc n := 7
emit n + 1
emit n * 2
emit n / 3    # 2.333...  (/ is true div)
emit n // 3   # 2         (// is floor div)
emit n % 3    # 1`
        },
        {
            title: 'Floats',
            code:
`alloc pi := 3.14
emit pi * 2.0
emit 10 / 4    # 2.5    (/ always returns float)
emit 10 // 4   # 2      (// keeps int)`
        },
        {
            title: 'Strings',
            code:
`alloc s := "hello"
emit s + ", world"
emit s[0]
emit length(s)`
        },
        {
            title: 'Comparisons & logic',
            code:
`if x == 1 and y != 2 { emit "ok" }
if x > 0 or x < -10 { emit "out" }`
        },
        {
            title: 'Conditionals',
            code:
`if n < 0 {
    emit "neg"
} elif n == 0 {
    emit "zero"
} else {
    emit "pos"
}`
        },
        {
            title: 'Loops',
            code:
`alloc i := 0
loop i < 5 {
    if i == 3 { break }
    emit i
    i := i + 1
}`
        },
        {
            title: 'Lists & slicing',
            code:
`alloc xs := [1, 2, 3]
xs := xs + [4]
emit xs[0]
emit xs[1:3]   # slice
emit length(xs)`
        },
        {
            title: 'Dictionaries',
            code:
`alloc d := {"a": 1, "b": 2}
emit d["a"]
d["c"] := 3
emit d`
        },
        {
            title: 'Functions',
            code:
`proc add(a, b) {
    return a + b
}
emit add(2, 3)`
        },
        {
            title: 'Closures',
            code:
`proc make_counter() {
    alloc n := 0
    proc tick() {
        n := n + 1
        return n
    }
    return tick
}`
        },
        {
            title: 'Try / except',
            code:
`try {
    alloc xs := [1, 2]
    emit xs[99]
} except err {
    emit "caught: " + err
}`
        },
        {
            title: 'Imports',
            code:
`import "math.omg" as math
emit math.sqrt(16)`
        },

        { kind: 'section', title: 'Built-ins' },
        {
            title: 'length, chr, ascii',
            code:
`emit length("hi")    # 2
emit chr(65)         # "A"
emit ascii("A")      # 65`
        },
        {
            title: 'hex, binary',
            code:
`emit hex(255)        # "ff"
emit binary(5)       # "101"`
        },
        {
            title: 'freeze',
            code:
`alloc xs := [1, 2, 3]
freeze(xs)           # now immutable`
        },
        {
            title: 'panic, raise',
            code:
`try { raise "oops" } except e { emit e }
panic("fatal")       # aborts`
        },
        {
            title: 'Files',
            code:
`alloc src := read_file("a.txt")
if file_exists("b.txt") { emit "yes" }
alloc f := file_open("o.txt", "w")
file_write(f, "hi")
file_close(f)`
        },

        { kind: 'section', title: 'Language variables' },
        {
            title: 'args, module_file, current_dir',
            code:
`emit args            # CLI argv
emit module_file     # current path
emit current_dir     # cwd`
        },
    ];

    // --- Tokeniser (mirrors app.js / explorer.app.js) ---------------------

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

    function tokenize(src) {
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
            if (ch === '#') {
                let j = i;
                while (j < n && src[j] !== '\n') j++;
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
                while (j < n && (src[j] === '.' || (src[j] >= '0' && src[j] <= '9'))) j++;
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

    function highlight(src) {
        let html = '';
        for (const t of tokenize(src)) {
            const safe = escapeHTML(t.text);
            html += t.type ? '<span class="tok-' + t.type + '">' + safe + '</span>' : safe;
        }
        return html;
    }

    // --- Render -----------------------------------------------------------

    function render(host) {
        let html = '';
        for (const item of REFERENCE) {
            if (item.kind === 'section') {
                html += '<h3 class="ref-section">' + escapeHTML(item.title) + '</h3>';
            } else {
                html +=
                    '<div class="ref-entry">' +
                        '<div class="ref-title">' + escapeHTML(item.title) + '</div>' +
                        '<pre class="ref-code"><code>' + highlight(item.code) + '</code></pre>' +
                    '</div>';
            }
        }
        host.innerHTML = html;
    }

    // --- Sidebar toggle ---------------------------------------------------

    const STORAGE_KEY = 'omg-sidebar';

    function applyState(state) {
        document.documentElement.setAttribute('data-sidebar', state);
        const btn = document.getElementById('sidebarToggle');
        if (btn) btn.setAttribute('aria-pressed', state === 'open' ? 'true' : 'false');
    }

    function initialState() {
        try {
            const stored = localStorage.getItem(STORAGE_KEY);
            if (stored === 'open' || stored === 'closed') return stored;
        } catch (_) {}
        // Default: open on wide screens, closed on narrow.
        return (window.innerWidth >= 980) ? 'open' : 'closed';
    }

    function init() {
        const host = document.getElementById('sidebarBody');
        if (host) render(host);

        applyState(initialState());

        const btn = document.getElementById('sidebarToggle');
        if (btn) {
            btn.addEventListener('click', () => {
                const cur = document.documentElement.getAttribute('data-sidebar') || 'open';
                const next = cur === 'open' ? 'closed' : 'open';
                applyState(next);
                try { localStorage.setItem(STORAGE_KEY, next); } catch (_) {}
            });
        }

        const close = document.getElementById('sidebarClose');
        if (close) {
            close.addEventListener('click', () => {
                applyState('closed');
                try { localStorage.setItem(STORAGE_KEY, 'closed'); } catch (_) {}
            });
        }
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();
