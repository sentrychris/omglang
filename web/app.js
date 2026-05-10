// Browser playground for OMG. Loads pre-transpiled examples and
// evals them with stdout redirected into the page.

const EXAMPLES = [
    'hello_world',
    'higher_order',
    'prime_sieve',
    'maze_solver',
    'merge_sort',
    'rot_13',
    'hex_to_rgb',
    'matrix_ops',
    'tabula_recta',
    'stack_vm',
    'dictionaries',
    'bitwise',
    'floats',
    'assignment',
];

const $select  = document.getElementById('example');
const $source  = document.getElementById('source');
const $output  = document.getElementById('output');
const $status  = document.getElementById('status');
const $run     = document.getElementById('run');

EXAMPLES.forEach(name => {
    const o = document.createElement('option');
    o.value = name;
    o.textContent = name + '.omg';
    $select.appendChild(o);
});

async function loadExample(name) {
    $status.textContent = 'loading…';
    $output.classList.remove('error');
    $output.textContent = '';
    try {
        const [src, js] = await Promise.all([
            fetch('examples/' + name + '.omg').then(r => r.text()),
            fetch('examples/' + name + '.js').then(r => r.text()),
        ]);
        $source.value = src;
        $select.dataset.js = js;
        $status.textContent = '';
    } catch (e) {
        $status.textContent = 'load failed: ' + e.message;
    }
}

function runJS(jsSource) {
    $output.classList.remove('error');
    $output.textContent = '';
    // Override stdout sinks before running. The transpiled program
    // declares `_omg_write` / `_omg_print_raw` with `let`, so we
    // can't reassign from outside — but the runtime exposes them
    // as module-scope let-bindings that the program reads at every
    // emit. We append a one-line override before the IIFE block.
    const wrapped =
        jsSource
            // Redirect omg_emit and omg_print into the output pane.
            .replace(
                "let _omg_write = (s) => { console.log(s); };",
                "let _omg_write = (s) => { window.__omg_emit_line(String(s)); };"
            )
            .replace(
                "let _omg_print_raw = (s) => { process.stdout.write(s); };",
                "let _omg_print_raw = (s) => { window.__omg_emit_raw(String(s)); };"
            )
            // Likewise for exit and exit-with-error: keep the
            // browser alive instead of trying process.exit.
            .replace(
                "let _omg_exit = (code) => { if (typeof process !== 'undefined') process.exit(Number(code)); };",
                "let _omg_exit = (code) => { window.__omg_exit_code = Number(code); throw new OmgError('Exit', 'exit ' + code); };"
            );
    let buf = '';
    window.__omg_emit_line = (line) => { buf += line + '\n'; };
    window.__omg_emit_raw  = (s)    => { buf += s; };
    window.__omg_exit_code = 0;
    try {
        // eslint-disable-next-line no-new-func
        new Function(wrapped)();
    } catch (e) {
        // Top-level OmgError already gets caught inside the IIFE
        // (the transpiler emits a try/catch that writes to
        // process.stderr); in the browser process is undefined, so
        // it rethrows. Catch it here and write to the output pane.
        if (e && e.kind && e.omgMessage !== undefined) {
            buf += '\n' + e.kind + ': ' + e.omgMessage + '\n';
            $output.classList.add('error');
        } else {
            buf += '\n[playground error] ' + e.message + '\n';
            $output.classList.add('error');
        }
    }
    $output.textContent = buf || '(no output)';
}

$select.addEventListener('change', () => loadExample($select.value));
$run.addEventListener('click', () => {
    const js = $select.dataset.js;
    if (!js) { $status.textContent = 'pick an example first'; return; }
    runJS(js);
});

// Default load.
$select.value = EXAMPLES[0];
loadExample(EXAMPLES[0]);
