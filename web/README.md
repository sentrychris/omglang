# OMG playground (browser)

Static HTML page that runs OMG programs compiled to JavaScript, in
your browser. The full toolchain is OMG → bytecode → JavaScript:

```
foo.omg --[bootstrap/bin/omg --compile]--> foo.omgb
       --[bootstrap/src/native-js.omg]--> foo.js
       --[node, or this page]--> output
```

## Running locally

```sh
bootstrap/build-web.sh                    # rebuild web/examples/
cd web && python3 -m http.server          # any static server works
open http://localhost:8000
```

Pick an example from the dropdown, hit Run, see the output. The
displayed source is the original `.omg`; what actually runs is the
pre-built `.js` next to it.

## What's here

```
web/
├── README.md           this file
├── index.html          page shell + styling
├── app.js              dropdown loader + JS evaluator
└── examples/           pre-built OMG → JS pairs
    ├── hello_world.omg
    ├── hello_world.js
    └── ...
```

## What's not here yet

- **In-browser compilation.** The plan calls for compiling
  `bootstrap/src/vm.omg` to JS so the browser can run any user-typed
  OMG source through the embedded compiler. That's a bigger project
  (compile compiler + VM + runtime to a single JS bundle, then drive
  it from the page); for now the playground ships a fixed corpus.
- **File I/O.** Programs that touch the file system get a TypeError
  in-browser. The node-targeted JS bundles work fine outside the
  browser (see [bootstrap/src/omg_rt.js](../bootstrap/src/omg_rt.js)
  for the fs shims).
