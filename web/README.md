# OMG playground (browser)

A static page that runs **any** OMG program you type, in your
browser. The full OMG-in-OMG compiler and VM are bundled to a single
`omg-web.js` file (~1.4 MB) by the same self-hosted toolchain that
produces the native binaries — so what runs in the page is the exact
same compiler the rest of the project uses, transpiled to JavaScript.

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

The bundle is built by `bootstrap/src/native-js.omg` from
`bootstrap/src/omg-web.omg` (a stripped-down driver that takes the
user source from `args[1]` instead of reading a file). Every Run
click re-evaluates the bundle so OMG-side globals start fresh.

## Running locally

```sh
bootstrap/build-web.sh                    # builds web/omg-web.js + web/examples/
cd web && python3 -m http.server          # any static server works
open http://localhost:8000
```

## What's here

```
web/
├── README.md           this file
├── index.html          page shell + styling
├── app.js              loads the bundle, redirects emit/print into <pre>
├── omg-web.js          compiler + VM + driver bundled to JavaScript
└── examples/           pre-built reference pairs (.omg + transpiled .js)
                        kept around so visitors can inspect what
                        native-js.omg's output looks like
```

## What's not here

- **Mobile-friendly UI.** Layout works but isn't optimised for touch.
- **Save / share.** The page has no persistence — refresh wipes your
  edits. URL-fragment encoding for share-this-snippet would be a
  small addition.
- **AOT-build.** `omg --build foo.omg foo` compiles to a native ELF;
  there's no equivalent here. The browser path uses the bytecode VM.
