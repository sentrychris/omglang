import * as path from 'path';
import * as vscode from 'vscode';
import { LanguageClient, LanguageClientOptions, ServerOptions } from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext) {
    console.log('OMG Language Server extension is activating...');

    const serverModule = context.asAbsolutePath(path.join('server', 'main.py'));
    const serverOptions: ServerOptions = {
        command: 'python',
        args: [serverModule],
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'omg' }],
    };

    client = new LanguageClient('omg', 'OMG Language Server', serverOptions, clientOptions);
    client.start();
    context.subscriptions.push(client);
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
