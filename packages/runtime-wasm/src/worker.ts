import init, { WasmSession } from "../../runtime-wasm/pkg/omg_runtime_wasm.js";

let session: WasmSession | null = null;

self.onmessage = async (e: MessageEvent) => {
  const { type, id, code, opts } = (e.data || {}) as any;
  try {
    if (type === "init") {
      await init();
      session = new WasmSession();
      (self as any).postMessage({ type: "init:ok" });
      return;
    }
    if (!session) {
      throw new Error("not initialized");
    }
    if (type === "eval") {
      const res = session.eval(code, opts);
      (self as any).postMessage({ type: "eval:ok", id, result: res });
      return;
    }
    if (type === "reset") {
      session.reset();
      (self as any).postMessage({ type: "reset:ok" });
      return;
    }
  } catch (err: any) {
    (self as any).postMessage({ type: `${type}:err`, id, error: String(err?.message || err) });
  }
};

export {};
