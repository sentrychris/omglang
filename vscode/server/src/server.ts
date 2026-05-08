// OMG Language Server.
//
// Provides:
//   - Completion: keywords, built-ins, top-level user symbols (procs,
//     allocs, import aliases), per-proc parameters, and member completion
//     after `alias.` for imported namespaces.
//   - Hover: shows the signature / detail / doc-comment of the symbol
//     under the cursor.
//   - Go-to-definition: jumps to the originating `proc`, `alloc`, or
//     `import as ...` line.
//   - Document symbols: outline of top-level procs and allocs.
//
// The server keeps a per-file index of `OMGSymbol`s, refreshed on
// open/change/save and on filesystem-watcher events forwarded by the
// client. Imports are resolved transitively so cross-file references work
// without opening every file manually.

import * as fs from 'fs';
import {
    createConnection,
    TextDocuments,
    ProposedFeatures,
    InitializeParams,
    TextDocumentSyncKind,
    InitializeResult,
    CompletionItem,
    CompletionItemKind,
    Hover,
    MarkupKind,
    Location,
    Range,
    Position,
    DocumentSymbol,
    SymbolKind,
    TextDocumentPositionParams,
    DocumentSymbolParams,
    DefinitionParams,
    CompletionParams,
    HoverParams
} from 'vscode-languageserver/node';
import { TextDocument } from 'vscode-languageserver-textdocument';
import { URI } from 'vscode-uri';

import {
    BUILTINS,
    BUILTIN_NAMES,
    KEYWORDS,
    RESERVED_GLOBALS
} from './builtins';
import {
    analyzeText,
    OMGDocumentInfo,
    OMGSymbol,
    resolveImportUri
} from './analyzer';

// --- Connection / document tracking --------------------------------------

const connection = createConnection(ProposedFeatures.all);
const documents: TextDocuments<TextDocument> = new TextDocuments(TextDocument);

/** Per-URI analysis cache, including content read from disk for imports. */
const indexByUri = new Map<string, OMGDocumentInfo>();
/** Source text per URI (we read imports from disk if not already open). */
const textByUri = new Map<string, string>();

// --- Lifecycle -----------------------------------------------------------

connection.onInitialize((_params: InitializeParams): InitializeResult => {
    return {
        capabilities: {
            textDocumentSync: TextDocumentSyncKind.Incremental,
            completionProvider: {
                resolveProvider: false,
                triggerCharacters: ['.']
            },
            hoverProvider: true,
            definitionProvider: true,
            documentSymbolProvider: true
        }
    };
});

documents.onDidOpen((e) => {
    indexDocument(e.document.uri, e.document.getText());
});
documents.onDidChangeContent((e) => {
    indexDocument(e.document.uri, e.document.getText());
});

connection.onDidChangeWatchedFiles((change) => {
    // Files modified outside the editor (or imported targets the user
    // hasn't opened) — re-index from disk so cross-file go-to-def stays
    // accurate.
    for (const event of change.changes) {
        const uri = event.uri;
        if (!uri.endsWith('.omg')) continue;
        const fsPath = URI.parse(uri).fsPath;
        try {
            const text = fs.readFileSync(fsPath, 'utf8');
            indexDocument(uri, text);
        } catch {
            indexByUri.delete(uri);
            textByUri.delete(uri);
        }
    }
});

documents.listen(connection);
connection.listen();

// --- Indexing ------------------------------------------------------------

function indexDocument(uri: string, text: string): void {
    textByUri.set(uri, text);
    const info = analyzeText(uri, text);
    indexByUri.set(uri, info);
    // Eagerly index transitive imports so member completion works without
    // the user having to open every imported file.
    for (const imp of info.imports) {
        const target = resolveImportUri(uri, imp.rawPath);
        if (!target || indexByUri.has(target)) continue;
        try {
            const text = fs.readFileSync(URI.parse(target).fsPath, 'utf8');
            indexDocument(target, text);
        } catch {
            // ignore — the user gets a diagnostic at runtime
        }
    }
}

function getInfo(uri: string): OMGDocumentInfo | undefined {
    return indexByUri.get(uri);
}

// --- Completion ----------------------------------------------------------

connection.onCompletion((params: CompletionParams): CompletionItem[] => {
    const doc = documents.get(params.textDocument.uri);
    if (!doc) return [];
    const text = doc.getText();
    const offset = doc.offsetAt(params.position);

    // Member completion: did the user just type `alias.` ?
    const dotMatch = matchDotPrefix(text, offset);
    if (dotMatch) {
        const items = completeMembers(params.textDocument.uri, dotMatch);
        if (items.length > 0) return items;
    }

    return completeGlobal(params.textDocument.uri, params.position);
});

/**
 * Return everything visible at `pos` — keywords, builtins, reserved globals,
 * top-level user symbols, plus parameters of the enclosing proc.
 */
function completeGlobal(uri: string, pos: Position): CompletionItem[] {
    const out: CompletionItem[] = [];

    for (const kw of KEYWORDS) {
        out.push({
            label: kw,
            kind: CompletionItemKind.Keyword,
            sortText: '3_' + kw
        });
    }
    for (const b of BUILTINS) {
        out.push({
            label: b.name,
            kind: CompletionItemKind.Function,
            detail: b.signature,
            documentation: { kind: MarkupKind.Markdown, value: b.detail },
            sortText: '2_' + b.name
        });
    }
    for (const g of RESERVED_GLOBALS) {
        out.push({
            label: g.name,
            kind: CompletionItemKind.Variable,
            detail: '(global)',
            documentation: { kind: MarkupKind.Markdown, value: g.detail },
            sortText: '2_' + g.name
        });
    }

    const info = getInfo(uri);
    if (info) {
        for (const sym of info.topLevel) {
            out.push(symbolToCompletionItem(sym, '1_'));
        }
        // Parameters of the enclosing proc, if any.
        for (const [, paramSyms] of info.paramsByProc) {
            for (const p of paramSyms) {
                if (
                    p.scopeStart !== undefined &&
                    p.scopeEnd !== undefined &&
                    pos.line >= p.scopeStart &&
                    pos.line <= p.scopeEnd
                ) {
                    out.push(symbolToCompletionItem(p, '0_'));
                }
            }
        }
    }
    return out;
}

/**
 * Member completion: the user typed `OMGI.` and we return the imported
 * module's exported symbols.
 */
function completeMembers(
    importingUri: string,
    aliasName: string
): CompletionItem[] {
    const importing = getInfo(importingUri);
    if (!importing) return [];
    const imp = importing.imports.find((i) => i.alias === aliasName);
    if (!imp) return [];
    const targetUri = resolveImportUri(importingUri, imp.rawPath);
    if (!targetUri) return [];
    const target = getInfo(targetUri);
    if (!target) return [];
    return target.topLevel
        .filter((s) => s.kind === 'proc' || s.kind === 'alloc')
        .map((s) => symbolToCompletionItem(s, '0_'));
}

function symbolToCompletionItem(sym: OMGSymbol, sortPrefix: string): CompletionItem {
    let kind: CompletionItemKind;
    switch (sym.kind) {
        case 'proc':
            kind = CompletionItemKind.Function;
            break;
        case 'alloc':
            kind = CompletionItemKind.Variable;
            break;
        case 'param':
            kind = CompletionItemKind.Variable;
            break;
        case 'import':
            kind = CompletionItemKind.Module;
            break;
        default:
            kind = CompletionItemKind.Text;
    }
    const item: CompletionItem = {
        label: sym.name,
        kind,
        detail: sym.detail,
        sortText: sortPrefix + sym.name
    };
    if (sym.docComment) {
        item.documentation = {
            kind: MarkupKind.Markdown,
            value: sym.docComment
        };
    }
    return item;
}

/**
 * Walk back from `offset` past an identifier to see if the immediately
 * preceding character is `.`. If so, return the identifier *before* the
 * dot — that's the alias the user wants to complete members of.
 */
function matchDotPrefix(text: string, offset: number): string | undefined {
    // Skip back over the partial identifier the user is typing now.
    let i = offset;
    while (i > 0 && /[A-Za-z0-9_]/.test(text[i - 1])) {
        i--;
    }
    if (i === 0 || text[i - 1] !== '.') return undefined;
    let j = i - 1;
    while (j > 0 && /[A-Za-z0-9_]/.test(text[j - 1])) {
        j--;
    }
    return text.slice(j, i - 1);
}

// --- Hover ---------------------------------------------------------------

connection.onHover((params: HoverParams): Hover | null => {
    const sym = symbolAtPosition(params);
    if (!sym) return null;
    const lines: string[] = ['```omg', sym.detail, '```'];
    if (sym.docComment) {
        lines.push('', sym.docComment);
    }
    return {
        contents: { kind: MarkupKind.Markdown, value: lines.join('\n') }
    };
});

// --- Definition ----------------------------------------------------------

connection.onDefinition((params: DefinitionParams): Location | null => {
    const sym = symbolAtPosition(params);
    if (!sym) return null;
    const range = Range.create(
        Position.create(sym.line, sym.col),
        Position.create(sym.line, sym.col + sym.name.length)
    );
    return Location.create(sym.uri, range);
});

// --- Document symbols ----------------------------------------------------

connection.onDocumentSymbol(
    (params: DocumentSymbolParams): DocumentSymbol[] => {
        const info = getInfo(params.textDocument.uri);
        if (!info) return [];
        return info.topLevel.map((sym) => {
            const range = Range.create(
                Position.create(sym.line, sym.col),
                Position.create(sym.line, sym.col + sym.name.length)
            );
            const kind =
                sym.kind === 'proc'
                    ? SymbolKind.Function
                    : sym.kind === 'import'
                    ? SymbolKind.Module
                    : SymbolKind.Variable;
            return DocumentSymbol.create(
                sym.name,
                sym.detail,
                kind,
                range,
                range
            );
        });
    }
);

// --- Symbol resolution helper -------------------------------------------

/**
 * Find the OMGSymbol the cursor is on. Handles three cases:
 *   - bare identifier  → top-level symbol or in-scope param
 *   - `alias.member`   → member of an imported module
 *   - keyword/builtin  → synthesised symbol for hover only
 */
function symbolAtPosition(
    params: TextDocumentPositionParams
): OMGSymbol | undefined {
    const doc = documents.get(params.textDocument.uri);
    if (!doc) return undefined;
    const text = doc.getText();
    const offset = doc.offsetAt(params.position);
    const word = wordAt(text, offset);
    if (!word) return undefined;

    // Detect alias.member by checking the character preceding the word.
    let memberOf: string | undefined;
    {
        const wordStart = offset - findIdentLeft(text, offset);
        if (wordStart > 0 && text[wordStart - 1] === '.') {
            let j = wordStart - 1;
            while (j > 0 && /[A-Za-z0-9_]/.test(text[j - 1])) j--;
            memberOf = text.slice(j, wordStart - 1);
        }
    }
    if (memberOf) {
        const importing = getInfo(params.textDocument.uri);
        const imp = importing?.imports.find((i) => i.alias === memberOf);
        if (imp) {
            const targetUri = resolveImportUri(params.textDocument.uri, imp.rawPath);
            if (targetUri) {
                const target = getInfo(targetUri);
                const found = target?.topLevel.find((s) => s.name === word);
                if (found) return found;
            }
        }
    }

    const info = getInfo(params.textDocument.uri);
    if (info) {
        // Parameters in scope at the cursor.
        for (const [, paramSyms] of info.paramsByProc) {
            for (const p of paramSyms) {
                if (
                    p.name === word &&
                    p.scopeStart !== undefined &&
                    p.scopeEnd !== undefined &&
                    params.position.line >= p.scopeStart &&
                    params.position.line <= p.scopeEnd
                ) {
                    return p;
                }
            }
        }
        const top = info.topLevel.find((s) => s.name === word);
        if (top) return top;
    }

    // Built-ins
    const builtin = BUILTINS.find((b) => b.name === word);
    if (builtin) {
        return {
            name: builtin.name,
            kind: 'proc',
            detail: builtin.signature,
            uri: params.textDocument.uri,
            line: params.position.line,
            col: params.position.character,
            docComment: builtin.detail
        };
    }
    const reserved = RESERVED_GLOBALS.find((g) => g.name === word);
    if (reserved) {
        return {
            name: reserved.name,
            kind: 'alloc',
            detail: `${reserved.name} (built-in global)`,
            uri: params.textDocument.uri,
            line: params.position.line,
            col: params.position.character,
            docComment: reserved.detail
        };
    }
    if (KEYWORDS.includes(word)) {
        return {
            name: word,
            kind: 'alloc',
            detail: `${word} (keyword)`,
            uri: params.textDocument.uri,
            line: params.position.line,
            col: params.position.character
        };
    }
    return undefined;
}

function wordAt(text: string, offset: number): string | undefined {
    let start = offset;
    while (start > 0 && /[A-Za-z0-9_]/.test(text[start - 1])) start--;
    let end = offset;
    while (end < text.length && /[A-Za-z0-9_]/.test(text[end])) end++;
    if (start === end) return undefined;
    const w = text.slice(start, end);
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(w)) return undefined;
    return w;
}

function findIdentLeft(text: string, offset: number): number {
    let n = 0;
    while (offset - n > 0 && /[A-Za-z0-9_]/.test(text[offset - n - 1])) n++;
    return n;
}

// Ensure builtins module is reachable for IDE go-to-def.
void BUILTIN_NAMES;
