export interface EvalOptions { timeoutMs?: number; fuel?: number }
export interface EvalResult {
  stdout: string[];
  returnValue: unknown;
  diagnostics: Array<{ message: string; line?: number; column?: number; kind: 'error' | 'warning' }>;
  elapsedMs: number;
  fuelUsed?: number;
}
export interface Session {
  eval(code: string, opts?: EvalOptions): Promise<EvalResult>;
  reset(): void;
  setFuelLimit(fuel: number): void;
  setStdout(fn: (text: string) => void): void;
  dispose(): void;
}

async function postAndWait(worker: Worker, msg: any): Promise<any> {
  return new Promise((resolve, reject) => {
    const id = msg.id;
    const listener = (e: MessageEvent) => {
      if (e.data?.id !== id && e.data?.type !== `${msg.type}:ok` && e.data?.type !== `${msg.type}:err`) {
        return;
      }
      worker.removeEventListener("message", listener);
      if (e.data.type.endsWith(":ok")) {
        resolve(e.data.result);
      } else {
        reject(new Error(e.data.error));
      }
    };
    worker.addEventListener("message", listener);
    worker.postMessage(msg);
  });
}

export async function createSession(): Promise<Session> {
  const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
  await postAndWait(worker, { type: "init" });
  let stdoutFn: (text: string) => void = () => {};
  worker.onmessage = (e) => {
    if (e.data?.type === "stdout") {
      stdoutFn(e.data.text);
    }
  };
  return {
    async eval(code: string, opts?: EvalOptions): Promise<EvalResult> {
      const id = crypto.randomUUID();
      return await postAndWait(worker, { type: "eval", id, code, opts });
    },
    reset() {
      worker.postMessage({ type: "reset" });
    },
    setFuelLimit(_fuel: number) {
      // Placeholder: fuel limits handled inside WASM in future revisions.
    },
    setStdout(fn: (text: string) => void) {
      stdoutFn = fn;
    },
    dispose() {
      worker.terminate();
    },
  };
}

export const version = "0.1.0";
