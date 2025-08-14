import { createSession } from "@omg-lang/runtime-wasm";

const container = document.getElementById("app")!;
const editor = document.createElement("textarea");
editor.value = "emit 1+1";
container.appendChild(editor);

const run = document.createElement("button");
run.textContent = "Run";
container.appendChild(run);

const output = document.createElement("pre");
container.appendChild(output);

let session: any = null;
createSession().then(s => {
  session = s;
});

run.onclick = async () => {
  if (!session) return;
  output.textContent = "";
  const res = await session.eval(editor.value);
  output.textContent = res.stdout.join("\n");
};
