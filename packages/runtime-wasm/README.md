# @omg-lang/runtime-wasm

WebAssembly build of the OMG language runtime with TypeScript bindings.

```ts
import { createSession } from "@omg-lang/runtime-wasm";

const session = await createSession();
const result = await session.eval("emit 1+1");
console.log(result.stdout);
```
