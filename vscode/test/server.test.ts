// The bundled language server, spoken to over stdio the way an editor does.

import { test } from 'node:test';
import * as assert from 'node:assert/strict';
import { ChildProcessWithoutNullStreams, spawn } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { pathToUri } from '../server/src/workspace';

const SERVER = path.resolve(__dirname, '..', 'server.js');

class Client {
    private buffer = Buffer.alloc(0);
    private nextId = 1;
    private readonly waiting = new Map<number, (result: unknown) => void>();
    private readonly notified: { method: string; params: unknown }[] = [];
    private readonly listeners: (() => void)[] = [];

    constructor(private readonly proc: ChildProcessWithoutNullStreams) {
        proc.stdout.on('data', (chunk: Buffer) => {
            this.buffer = Buffer.concat([this.buffer, chunk]);
            for (;;) {
                const head = this.buffer.indexOf('\r\n\r\n');
                if (head < 0) return;
                const m = /Content-Length: (\d+)/i.exec(this.buffer.subarray(0, head).toString());
                if (!m) throw new Error('bad header');
                const end = head + 4 + Number(m[1]);
                if (this.buffer.length < end) return;
                const msg = JSON.parse(this.buffer.subarray(head + 4, end).toString());
                this.buffer = this.buffer.subarray(end);
                if (msg.id !== undefined && msg.method === undefined) {
                    this.waiting.get(msg.id)?.(msg.error !== undefined ? msg.error : msg.result);
                    this.waiting.delete(msg.id);
                } else if (msg.id !== undefined) {
                    // A request from the server (configuration, capability registration).
                    this.write({ jsonrpc: '2.0', id: msg.id, result: msg.method === 'workspace/configuration' ? [null] : null });
                } else {
                    this.notified.push({ method: msg.method, params: msg.params });
                    for (const l of this.listeners.splice(0)) l();
                }
            }
        });
    }

    private write(msg: unknown): void {
        const body = Buffer.from(JSON.stringify(msg));
        this.proc.stdin.write('Content-Length: ' + body.length + '\r\n\r\n');
        this.proc.stdin.write(body);
    }

    request<T>(method: string, params: unknown): Promise<T> {
        const id = this.nextId++;
        return new Promise((resolve) => {
            this.waiting.set(id, resolve as (r: unknown) => void);
            this.write({ jsonrpc: '2.0', id, method, params });
        });
    }

    notify(method: string, params: unknown): void {
        this.write({ jsonrpc: '2.0', method, params });
    }

    /** The next notification that passes the test, among those already in and those to come. */
    async until<T>(method: string, pass: (params: T) => boolean): Promise<T> {
        const deadline = Date.now() + 10000;
        for (;;) {
            const i = this.notified.findIndex((n) => n.method === method && pass(n.params as T));
            if (i >= 0) return this.notified.splice(i, 1)[0].params as T;
            if (Date.now() > deadline) throw new Error('timed out waiting for ' + method);
            await new Promise<void>((resolve) => {
                const t = setTimeout(resolve, 200);
                this.listeners.push(() => { clearTimeout(t); resolve(); });
            });
        }
    }
}

interface Published {
    uri: string;
    diagnostics: { message: string; code: string; range: { start: { line: number } } }[];
}

test('the bundled server starts, checks a file and answers requests', async () => {
    assert.ok(fs.existsSync(SERVER), 'out/server.js is not built');
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'omg-lsp-stdio-'));
    const proc = spawn(process.execPath, [SERVER, '--stdio'], { stdio: ['pipe', 'pipe', 'pipe'] });
    const client = new Client(proc);
    try {
        const init = await client.request<{ capabilities: Record<string, unknown>; serverInfo: { name: string } }>('initialize', {
            processId: process.pid,
            rootUri: pathToUri(dir),
            workspaceFolders: [{ uri: pathToUri(dir), name: 'test' }],
            capabilities: { workspace: { configuration: true } }
        });
        assert.equal(init.serverInfo.name, 'omg-language-server');
        for (const cap of [
            'hoverProvider', 'completionProvider', 'signatureHelpProvider', 'definitionProvider', 'referencesProvider',
            'renameProvider', 'documentSymbolProvider', 'documentFormattingProvider', 'semanticTokensProvider',
            'codeActionProvider', 'foldingRangeProvider', 'documentHighlightProvider', 'workspaceSymbolProvider'
        ]) {
            assert.ok(init.capabilities[cap], cap);
        }
        client.notify('initialized', {});

        const uri = pathToUri(path.join(dir, 'a.omg'));
        const text = ';;;omg\nproc double(n) {\n    return n * 2\n}\nemit double(1, 2)\nemit length("abc")\n';
        client.notify('textDocument/didOpen', { textDocument: { uri, languageId: 'omg', version: 1, text } });
        const first = await client.until<Published>('textDocument/publishDiagnostics', (p) => p.uri === uri && p.diagnostics.length > 0);
        assert.equal(first.diagnostics.length, 1);
        assert.equal(first.diagnostics[0].code, 'arity');
        assert.equal(first.diagnostics[0].range.start.line, 4);

        const hover = await client.request<{ contents: { value: string } }>('textDocument/hover', {
            textDocument: { uri }, position: { line: 5, character: 7 }
        });
        assert.match(hover.contents.value, /length\(x\)/);

        const items = await client.request<{ label: string }[]>('textDocument/completion', {
            textDocument: { uri }, position: { line: 4, character: 7 }
        });
        assert.ok(items.some((i) => i.label === 'double'));

        const tokens = await client.request<{ data: number[] }>('textDocument/semanticTokens/full', { textDocument: { uri } });
        assert.ok(tokens.data.length >= 25 && tokens.data.length % 5 === 0);

        // Fixing the call clears the error; breaking the syntax brings the compiler's message.
        client.notify('textDocument/didChange', {
            textDocument: { uri, version: 2 },
            contentChanges: [{ range: { start: { line: 4, character: 13 }, end: { line: 4, character: 16 } }, text: '' }]
        });
        await client.until<Published>('textDocument/publishDiagnostics', (p) => p.uri === uri && p.diagnostics.length === 0);
        client.notify('textDocument/didChange', {
            textDocument: { uri, version: 3 },
            contentChanges: [{ range: { start: { line: 5, character: 0 }, end: { line: 5, character: 0 } }, text: 'emit (\n' }]
        });
        const broken = await client.until<Published>('textDocument/publishDiagnostics', (p) => p.uri === uri && p.diagnostics.length > 0);
        assert.match(broken.diagnostics[0].message, /^SyntaxError: /);

        // A request about a file the server has never seen must not hurt it.
        const nothing = await client.request('textDocument/hover', {
            textDocument: { uri: pathToUri(path.join(dir, 'missing.omg')) }, position: { line: 0, character: 0 }
        });
        assert.equal(nothing, null);

        await client.request('shutdown', null);
        client.notify('exit', null);
        const code = await new Promise<number | null>((resolve) => proc.on('exit', resolve));
        assert.equal(code, 0);
    } finally {
        proc.kill();
        fs.rmSync(dir, { recursive: true, force: true });
    }
});
