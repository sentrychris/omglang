// The OMG language server.
//
// Everything it knows comes from a port of the compiler's own lexer and
// parser (lexer.ts, parser.ts) and a scope analysis that follows the
// compiler's name rules (analysis.ts), so what it reports is what
// `omg` would say, as you type and without running anything.

import {
    CodeAction, CodeActionKind, CompletionItem, Diagnostic, DidChangeConfigurationNotification,
    DocumentHighlight, DocumentLink, DocumentSymbol, FoldingRange, Hover, InitializeParams,
    InitializeResult, Location, ProposedFeatures, ResponseError, SemanticTokens, SignatureHelp,
    SymbolInformation, TextDocumentSyncKind, TextDocuments, TextEdit, WorkspaceEdit,
    createConnection
} from 'vscode-languageserver/node';
import { TextDocument } from 'vscode-languageserver-textdocument';

import { FileAnalysis, registerBuiltinNames } from './analysis';
import { BUILTINS } from './builtins';
import { DEFAULT_DIAGNOSTICS, DiagnosticSettings, codeActions, diagnose } from './diagnostics';
import { completion, hover, signatureHelp } from './features';
import { formatLines } from './format';
import {
    definition, documentLinks, documentSymbols, foldingRanges, highlights, prepareRename,
    references, rename, workspaceSymbols
} from './navigation';
import { TOKEN_MODIFIERS, TOKEN_TYPES, semanticTokens } from './semantic';
import { Workspace, canonicalUri, uriToPath } from './workspace';

registerBuiltinNames(BUILTINS.map((b) => b.name));

const connection = createConnection(ProposedFeatures.all);
const documents = new TextDocuments(TextDocument);
const ws = new Workspace();

let settings: DiagnosticSettings = { ...DEFAULT_DIAGNOSTICS };
let canConfigure = false;

connection.onInitialize((params: InitializeParams): InitializeResult => {
    const roots: string[] = [];
    for (const f of params.workspaceFolders ?? []) {
        const p = uriToPath(f.uri);
        if (p) roots.push(p);
    }
    if (roots.length === 0 && params.rootUri) {
        const p = uriToPath(params.rootUri);
        if (p) roots.push(p);
    }
    ws.setRoots(roots);
    canConfigure = params.capabilities.workspace?.configuration === true;

    return {
        serverInfo: { name: 'omg-language-server', version: '0.3.0' },
        capabilities: {
            textDocumentSync: TextDocumentSyncKind.Incremental,
            completionProvider: { triggerCharacters: ['.', '"', '/'] },
            hoverProvider: true,
            signatureHelpProvider: { triggerCharacters: ['(', ','], retriggerCharacters: [','] },
            definitionProvider: true,
            referencesProvider: true,
            documentHighlightProvider: true,
            renameProvider: { prepareProvider: true },
            documentSymbolProvider: true,
            workspaceSymbolProvider: true,
            foldingRangeProvider: true,
            documentLinkProvider: {},
            documentFormattingProvider: true,
            documentRangeFormattingProvider: true,
            codeActionProvider: { codeActionKinds: [CodeActionKind.QuickFix] },
            semanticTokensProvider: {
                legend: { tokenTypes: [...TOKEN_TYPES], tokenModifiers: [...TOKEN_MODIFIERS] },
                full: true
            }
        }
    };
});

connection.onInitialized(() => {
    if (canConfigure) {
        void connection.client.register(DidChangeConfigurationNotification.type, { section: 'omg' });
        void loadSettings();
    }
});

async function loadSettings(): Promise<void> {
    if (!canConfigure) return;
    try {
        const cfg = (await connection.workspace.getConfiguration('omg')) as { diagnostics?: Partial<DiagnosticSettings> } | null;
        settings = { ...DEFAULT_DIAGNOSTICS, ...(cfg?.diagnostics ?? {}) };
    } catch {
        settings = { ...DEFAULT_DIAGNOSTICS };
    }
    publishAll();
}

connection.onDidChangeConfiguration(() => {
    void loadSettings();
});

// ---- Documents and diagnostics -----------------------------------------------

/** The client's spelling of each open document's URI. */
const clientUri = new Map<string, string>();
let pending: ReturnType<typeof setTimeout> | undefined;

function publish(uri: string): void {
    const file = ws.get(uri);
    const target = clientUri.get(canonicalUri(uri)) ?? uri;
    let diagnostics: Diagnostic[] = [];
    if (file) {
        try {
            diagnostics = diagnose(ws, file, settings);
        } catch (e) {
            connection.console.error('diagnostics failed for ' + uri + ': ' + String(e));
        }
    }
    void connection.sendDiagnostics({ uri: target, diagnostics });
}

function publishAll(): void {
    for (const uri of ws.openUris()) publish(uri);
}

/** A change in one file can change what is true of the files that import it. */
function schedulePublishAll(): void {
    if (pending) clearTimeout(pending);
    pending = setTimeout(() => {
        pending = undefined;
        publishAll();
    }, 150);
}

documents.onDidOpen((e) => {
    clientUri.set(canonicalUri(e.document.uri), e.document.uri);
});

documents.onDidChangeContent((e) => {
    clientUri.set(canonicalUri(e.document.uri), e.document.uri);
    ws.open(e.document.uri, e.document.getText());
    publish(e.document.uri);
    schedulePublishAll();
});

documents.onDidClose((e) => {
    ws.close(e.document.uri);
    clientUri.delete(canonicalUri(e.document.uri));
    void connection.sendDiagnostics({ uri: e.document.uri, diagnostics: [] });
    schedulePublishAll();
});

connection.onDidChangeWatchedFiles((change) => {
    for (const ev of change.changes) {
        if (!ws.isOpen(ev.uri)) ws.invalidate(ev.uri);
    }
    schedulePublishAll();
});

function fileFor(uri: string): FileAnalysis | undefined {
    return ws.get(uri);
}

/** A request must never take the server down with it. */
function guard<T>(what: string, fallback: T, run: () => T): T {
    try {
        return run();
    } catch (e) {
        connection.console.error(what + ' failed: ' + (e instanceof Error ? e.stack ?? e.message : String(e)));
        return fallback;
    }
}

// ---- Requests -----------------------------------------------------------------

connection.onHover((p): Hover | null => guard('hover', null, () => {
    const f = fileFor(p.textDocument.uri);
    return f ? hover(ws, f, p.position) : null;
}));

connection.onCompletion((p): CompletionItem[] => guard('completion', [], () => {
    const f = fileFor(p.textDocument.uri);
    return f ? completion(ws, f, p.position) : [];
}));

connection.onSignatureHelp((p): SignatureHelp | null => guard('signature help', null, () => {
    const f = fileFor(p.textDocument.uri);
    return f ? signatureHelp(ws, f, p.position) : null;
}));

connection.onDefinition((p): Location | null => guard('definition', null, () => {
    const f = fileFor(p.textDocument.uri);
    return f ? definition(ws, f, p.position) : null;
}));

connection.onReferences((p): Location[] => guard('references', [], () => {
    const f = fileFor(p.textDocument.uri);
    return f ? references(ws, f, p.position, p.context.includeDeclaration) : [];
}));

connection.onDocumentHighlight((p): DocumentHighlight[] => guard('highlight', [], () => {
    const f = fileFor(p.textDocument.uri);
    return f ? highlights(ws, f, p.position) : [];
}));

connection.onPrepareRename((p) => guard('prepare rename', null, () => {
    const f = fileFor(p.textDocument.uri);
    return f ? prepareRename(ws, f, p.position) : null;
}));

connection.onRenameRequest((p): WorkspaceEdit | ResponseError<void> | null => {
    const f = fileFor(p.textDocument.uri);
    if (!f) return null;
    try {
        const result = rename(ws, f, p.position, p.newName);
        if (typeof result === 'string') return new ResponseError(-32602, result);
        return result;
    } catch (e) {
        connection.console.error('rename failed: ' + String(e));
        return null;
    }
});

connection.onDocumentSymbol((p): DocumentSymbol[] => guard('document symbols', [], () => {
    const f = fileFor(p.textDocument.uri);
    return f ? documentSymbols(f) : [];
}));

connection.onWorkspaceSymbol((p): SymbolInformation[] => guard('workspace symbols', [], () => workspaceSymbols(ws, p.query)));

connection.onFoldingRanges((p): FoldingRange[] => guard('folding', [], () => {
    const f = fileFor(p.textDocument.uri);
    return f ? foldingRanges(f) : [];
}));

connection.onDocumentLinks((p): DocumentLink[] => guard('links', [], () => {
    const f = fileFor(p.textDocument.uri);
    return f ? documentLinks(ws, f) : [];
}));

connection.onCodeAction((p): CodeAction[] => guard('code actions', [], () => {
    const f = fileFor(p.textDocument.uri);
    return f ? codeActions(f, p.textDocument.uri, p.range, p.context.diagnostics) : [];
}));

connection.languages.semanticTokens.on((p): SemanticTokens => guard('semantic tokens', { data: [] }, () => {
    const f = fileFor(p.textDocument.uri);
    return { data: f ? semanticTokens(f) : [] };
}));

function formatEdits(uri: string, tabSize: number, insertSpaces: boolean, first?: number, last?: number): TextEdit[] {
    const f = fileFor(uri);
    if (!f) return [];
    return formatLines(f, tabSize, insertSpaces, first, last).map((e) =>
        TextEdit.replace({ start: { line: e.line, character: e.from }, end: { line: e.line, character: e.to } }, e.text)
    );
}

connection.onDocumentFormatting((p): TextEdit[] => guard('formatting', [], () =>
    formatEdits(p.textDocument.uri, p.options.tabSize, p.options.insertSpaces)));

connection.onDocumentRangeFormatting((p): TextEdit[] => guard('formatting', [], () => {
    // A range that ends at the start of a line does not include that line.
    const last = p.range.end.character === 0 && p.range.end.line > p.range.start.line ? p.range.end.line - 1 : p.range.end.line;
    return formatEdits(p.textDocument.uri, p.options.tabSize, p.options.insertSpaces, p.range.start.line, last);
}));

documents.listen(connection);
connection.listen();
