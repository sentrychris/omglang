# OMG playground and compiler explorer (browser)

Two static pages. The **playground** (`index.html`) runs any OMG
program you type, in your browser. The **explorer** (`explorer.html`)
shows what the compiler makes of it at every stage: tokens, AST,
bytecode, and the C and JavaScript the back ends write.

The OMG compiler and VM are bundled to a single `omg-web.js` file
(~2.2 MB) by the same toolchain that builds `omg` itself — so what
runs in the page is the exact same compiler the rest of the project
uses, put through OMG's JavaScript back end. `omg-explorer.js`
(~3.7 MB) is the same plus the C and JS back ends.

## How it actually works

```
Your textarea source
        │
        ▼
omg-web.js  (compiler.omg + vm.omg + driver, all transpiled to JS)
        │
        ├── parse + compile_source → bytecode
        ├── vm.run on that bytecode
        ▼
emit output → page <pre>
```

The bundle is built by `scripts/build-web.sh`, which puts
`src/browser/omg-web.omg` (a stripped-down driver that takes the
user source from `args[1]` instead of reading a file) through the JS
back end: `omg --compile`, then `omg --emit-js`. Every Run
click re-evaluates the bundle so OMG-side globals start fresh.

## Running locally

```sh
scripts/build-web.sh                    # builds web/omg-web.js, web/omg-explorer.js, web/examples/
cd web && python3 -m http.server          # any static server works
open http://localhost:8000
```

## What's here

```
web/
├── README.md           this file
├── index.html          the playground: page shell
├── app.js              loads the bundle, redirects emit/print into <pre>
├── omg-web.js          compiler + VM + driver bundled to JavaScript
├── explorer.html       the compiler explorer
├── explorer.app.js     drives each stage and renders its output
├── omg-explorer.js     compiler + VM + the C and JS back ends, bundled
├── reference.js        the language reference shown beside the editor
├── share.js            share links: the source, compressed into the URL
├── style.css           styling for both pages
├── test-explorer.mjs   node test: every explorer stage still works (CI runs it)
└── examples/           copies of a few programs from examples/, plus
                        manifest.json. The playground and explorer
                        populate their "Examples" dropdown from the
                        manifest and lazy-fetch the .omg on selection.
                        To see what `omg --emit-js` writes for a
                        program, use the explorer page or run
                        `omg --emit-js prog.omg prog.js`
```

## What's not here

- **Mobile-friendly UI.** Layout works but isn't optimised for touch.
- **Building executables.** `omg --build foo.omg foo` writes a native
  executable; there's no equivalent here. The browser path uses the
  bytecode VM.
- **The machine-code back end.** The explorer shows the C and
  JavaScript routes. The x86-64 back end is not in the bundle.
