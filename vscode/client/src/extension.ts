// OMG VS Code extension entry point.
//
// Launches the bundled Node-based OMG language server (in `../server/out/server.js`)
// over stdio, then registers it as the language client for `.omg` files.
// Replaces the previous Python+pygls server, which depended on the legacy
// Python `omglang/` package and broke whenever Python wasn't on PATH.

import * as path from 'path';
import { ExtensionContext, workspace } from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export function activate(context: ExtensionContext): void {
    // Resolve the compiled server module relative to this extension.
    const serverModule = context.asAbsolutePath(
        path.join('server', 'out', 'server.js')
    );

    // Same module is used for run + debug; debug mode listens for an inspector.
    const serverOptions: ServerOptions = {
        run: { module: serverModule, transport: TransportKind.ipc },
        debug: {
            module: serverModule,
            transport: TransportKind.ipc,
            options: { execArgv: ['--nolazy', '--inspect=6009'] }
        }
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'omg' }],
        synchronize: {
            // Re-index when any .omg file in the workspace changes — even
            // ones that aren't currently open — so cross-file go-to-def
            // and completion stay accurate after edits.
            fileEvents: workspace.createFileSystemWatcher('**/*.omg')
        }
    };

    client = new LanguageClient(
        'omg',
        'OMG Language Server',
        serverOptions,
        clientOptions
    );

    client.start();
    context.subscriptions.push({
        dispose: () => {
            void client?.stop();
        }
    });
}

export function deactivate(): Thenable<void> | undefined {
    return client?.stop();
}
