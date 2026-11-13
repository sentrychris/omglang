// The editor side of the OMG extension: starts the language server and
// adds the commands that need the `omg` binary (run, build, disassemble,
// REPL). Everything the editor knows about the language itself comes
// from the server in ../../server.

import { execFile } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import {
    ExtensionContext, ProcessExecution, StatusBarAlignment, StatusBarItem, Task, TaskRevealKind,
    TaskScope, TextDocument, TextDocumentContentProvider, Uri, ViewColumn, commands, tasks,
    window, workspace
} from 'vscode';
import {
    LanguageClient, LanguageClientOptions, ServerOptions, TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;
let status: StatusBarItem | undefined;

const EXE = process.platform === 'win32' ? 'omg.exe' : 'omg';

function isFile(p: string): boolean {
    try {
        return fs.statSync(p).isFile();
    } catch {
        return false;
    }
}

/**
 * Where `omg` is: the `omg.path` setting, else `bin/omg` in the folder
 * of the file or any folder above it (the layout of the OMG repository
 * itself), else in a workspace folder, else whatever PATH finds.
 */
function findOmg(near?: string): { path: string; found: boolean } {
    const configured = workspace.getConfiguration('omg').get<string>('path', '').trim();
    if (configured) {
        let p = configured;
        if (p.startsWith('~')) p = path.join(process.env.HOME ?? process.env.USERPROFILE ?? '', p.slice(1));
        const folder = workspace.workspaceFolders?.[0]?.uri.fsPath;
        if (!path.isAbsolute(p) && folder) p = path.join(folder, p);
        return { path: p, found: isFile(p) };
    }
    const starts: string[] = [];
    if (near) starts.push(path.dirname(near));
    for (const f of workspace.workspaceFolders ?? []) starts.push(f.uri.fsPath);
    for (const start of starts) {
        let dir = start;
        for (let i = 0; i < 12; i++) {
            const candidate = path.join(dir, 'bin', EXE);
            if (isFile(candidate)) return { path: candidate, found: true };
            const up = path.dirname(dir);
            if (up === dir) break;
            dir = up;
        }
    }
    for (const dir of (process.env.PATH ?? '').split(path.delimiter)) {
        if (dir && isFile(path.join(dir, EXE))) return { path: path.join(dir, EXE), found: true };
    }
    return { path: EXE, found: false };
}

async function omgOrExplain(near?: string): Promise<string | undefined> {
    const omg = findOmg(near);
    if (omg.found) return omg.path;
    const pick = await window.showErrorMessage(
        "Could not find the 'omg' binary. Put it on your PATH, or set its location in the settings.",
        'Open Settings'
    );
    if (pick) void commands.executeCommand('workbench.action.openSettings', 'omg.path');
    return undefined;
}

/** The OMG document a command applies to: the one given, else the active editor's. */
async function targetDocument(uri?: Uri): Promise<TextDocument | undefined> {
    let doc: TextDocument | undefined;
    if (uri instanceof Uri) doc = await workspace.openTextDocument(uri);
    else doc = window.activeTextEditor?.document;
    if (!doc || doc.languageId !== 'omg') {
        void window.showInformationMessage('Open an OMG file first.');
        return undefined;
    }
    if (doc.isUntitled) {
        void window.showInformationMessage('Save the file first: omg runs files from disk.');
        return undefined;
    }
    if (doc.isDirty && !(await doc.save())) return undefined;
    return doc;
}

function runTask(label: string, omg: string, args: string[], cwd: string, focus: boolean): void {
    const task = new Task(
        { type: 'omg', label },
        // A file opened on its own, with no folder, has no workspace to belong to.
        workspace.workspaceFolders?.length ? TaskScope.Workspace : TaskScope.Global,
        label,
        'omg',
        new ProcessExecution(omg, args, { cwd }),
        '$omg'
    );
    task.presentationOptions = { reveal: TaskRevealKind.Always, focus, clear: true, showReuseMessage: false };
    void tasks.executeTask(task);
}

async function runFile(uri?: Uri): Promise<void> {
    const doc = await targetDocument(uri);
    if (!doc) return;
    const file = doc.uri.fsPath;
    const omg = await omgOrExplain(file);
    if (!omg) return;
    const extra = workspace.getConfiguration('omg', doc.uri).get<string[]>('run.args', []);
    runTask('Run ' + path.basename(file), omg, [file, ...extra], path.dirname(file), true);
}

async function buildFile(uri?: Uri): Promise<void> {
    const doc = await targetDocument(uri);
    if (!doc) return;
    const file = doc.uri.fsPath;
    const omg = await omgOrExplain(file);
    if (!omg) return;
    const targets = [
        { label: 'Native executable', description: 'for this machine', target: '' },
        { label: 'Linux x86-64', description: '--target linux-x64', target: 'linux-x64' },
        { label: 'Windows x86-64', description: '--target windows-x64', target: 'windows-x64' },
        { label: 'Through a C compiler', description: '--target c', target: 'c' }
    ];
    const pick = await window.showQuickPick(targets, { placeHolder: 'Build ' + path.basename(file) + ' for' });
    if (!pick) return;
    // omg names the output itself: the file without .omg, plus .exe for Windows.
    const args = ['--build', file];
    if (pick.target) args.push('--target', pick.target);
    runTask('Build ' + path.basename(file), omg, args, path.dirname(file), false);
}

const DISASM_SCHEME = 'omg-disasm';

class Disassembly implements TextDocumentContentProvider {
    provideTextDocumentContent(uri: Uri): Promise<string> {
        const file = uri.query;
        const omg = findOmg(file);
        return new Promise((resolve) => {
            execFile(omg.path, ['--disasm', file], { maxBuffer: 64 * 1024 * 1024 }, (err, stdout, stderr) => {
                if (err && !stdout) resolve(String(stderr || err.message));
                else resolve(stdout);
            });
        });
    }
}

async function disassemble(uri?: Uri): Promise<void> {
    let file: string | undefined;
    if (uri instanceof Uri && uri.fsPath.toLowerCase().endsWith('.omgb')) file = uri.fsPath;
    else file = (await targetDocument(uri))?.uri.fsPath;
    if (!file) return;
    if (!(await omgOrExplain(file))) return;
    const view = Uri.from({ scheme: DISASM_SCHEME, path: '/' + path.basename(file) + '.disasm', query: file });
    const doc = await workspace.openTextDocument(view);
    await window.showTextDocument(doc, { viewColumn: ViewColumn.Beside, preview: true, preserveFocus: true });
}

async function openRepl(): Promise<void> {
    const near = window.activeTextEditor?.document.uri.fsPath;
    const omg = await omgOrExplain(near);
    if (!omg) return;
    const cwd = workspace.workspaceFolders?.[0]?.uri.fsPath;
    window.createTerminal({ name: 'OMG REPL', shellPath: omg, cwd }).show();
}

function startClient(context: ExtensionContext): LanguageClient {
    const serverModule = context.asAbsolutePath(path.join('out', 'server.js'));
    const serverOptions: ServerOptions = {
        run: { module: serverModule, transport: TransportKind.ipc },
        debug: { module: serverModule, transport: TransportKind.ipc, options: { execArgv: ['--nolazy', '--inspect=6009'] } }
    };
    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'omg' }, { scheme: 'untitled', language: 'omg' }],
        synchronize: {
            // Files that are not open still feed imports and go to
            // definition, so the server hears about every change.
            fileEvents: workspace.createFileSystemWatcher('**/*.omg')
        }
    };
    const c = new LanguageClient('omg', 'OMG Language Server', serverOptions, clientOptions);
    void c.start();
    return c;
}

function refreshStatus(): void {
    if (!status) return;
    const editor = window.activeTextEditor;
    if (!editor || editor.document.languageId !== 'omg') {
        status.hide();
        return;
    }
    const omg = findOmg(editor.document.uri.fsPath);
    status.text = omg.found ? '$(play) OMG' : '$(warning) OMG';
    status.tooltip = omg.found
        ? 'Run this file with ' + omg.path
        : "The 'omg' binary was not found. Set omg.path in the settings to run files from here.";
    status.command = omg.found ? 'omg.run' : { title: 'Settings', command: 'workbench.action.openSettings', arguments: ['omg.path'] };
    status.show();
}

export function activate(context: ExtensionContext): void {
    client = startClient(context);

    status = window.createStatusBarItem(StatusBarAlignment.Right, 90);
    status.name = 'OMG';
    refreshStatus();

    context.subscriptions.push(
        status,
        commands.registerCommand('omg.run', runFile),
        commands.registerCommand('omg.build', buildFile),
        commands.registerCommand('omg.disassemble', disassemble),
        commands.registerCommand('omg.repl', openRepl),
        commands.registerCommand('omg.restartServer', async () => {
            if (client) await client.restart();
            else client = startClient(context);
        }),
        workspace.registerTextDocumentContentProvider(DISASM_SCHEME, new Disassembly()),
        window.onDidChangeActiveTextEditor(refreshStatus),
        workspace.onDidChangeConfiguration((e) => {
            if (e.affectsConfiguration('omg.path')) refreshStatus();
        }),
        { dispose: () => { void client?.stop(); } }
    );
}

export function deactivate(): Thenable<void> | undefined {
    return client?.stop();
}
