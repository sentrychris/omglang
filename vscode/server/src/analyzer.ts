// Lightweight OMG analyzer for the language server.
//
// Performs a *line-based* scan of OMG source — not a real parser — to extract
// the symbols we need for completion, hover, and go-to-definition:
//
//   - top-level `proc name(params) { ... }`
//   - top-level `alloc name := ...`
//   - `import "path" as alias`
//   - per-`proc` parameter and `alloc` locals (best-effort, scoped to the
//     enclosing brace block)
//
// The line-based approach is intentional: a full parser would be wasted work
// for what the LSP needs, and would have to be kept in lockstep with the
// real Rust frontend. Regexes give us 95% of the value with 5% of the code.

import * as fs from 'fs';
import * as path from 'path';
import { URI } from 'vscode-uri';

/**
 * One identifier the LSP knows about.
 */
export type OMGSymbol = {
    name: string;
    kind: 'proc' | 'alloc' | 'import' | 'param';
    detail: string;          // human-readable signature for hover/completion
    uri: string;
    line: number;            // 0-based, where the name appears
    col: number;             // 0-based, column of the name's first char
    params?: string[];       // procs only
    docComment?: string;     // /** ... */ immediately above the def
    /** Inclusive line range over which a `param` symbol is in scope. */
    scopeStart?: number;
    scopeEnd?: number;
};

/**
 * Per-document analysis result.
 */
export type OMGDocumentInfo = {
    uri: string;
    /** Symbols visible at the top level of this document. */
    topLevel: OMGSymbol[];
    /** Per-proc parameters, keyed by proc name. */
    paramsByProc: Map<string, OMGSymbol[]>;
    /** Imports recorded at the top level. */
    imports: { rawPath: string; alias: string; line: number; col: number }[];
    /** Whether the document begins with the required ;;;omg header. */
    hasHeader: boolean;
};

const PROC_RE = /^(\s*)proc\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)/;
const ALLOC_RE = /^(\s*)alloc\s+([A-Za-z_][A-Za-z0-9_]*)\s*:=/;
const IMPORT_RE = /^\s*import\s+"([^"]+)"\s+as\s+([A-Za-z_][A-Za-z0-9_]*)/;

export function analyzeText(uri: string, text: string): OMGDocumentInfo {
    const lines = text.split(/\r?\n/);
    const topLevel: OMGSymbol[] = [];
    const paramsByProc = new Map<string, OMGSymbol[]>();
    const imports: OMGDocumentInfo['imports'] = [];

    const hasHeader = detectHeader(lines);

    let pendingDoc: string | undefined;
    let docBlockOpenLine: number | undefined;
    let inDocBlock = false;

    // Track brace depth to distinguish top-level from inside-fn
    let braceDepth = 0;

    // When we're inside a proc body, remember its name + body extent so we
    // can scope the params list correctly.
    let activeProcStack: { name: string; startLine: number }[] = [];

    for (let i = 0; i < lines.length; i++) {
        const line = lines[i];

        // Doc-block tracking. We only attach docs to the next def we see.
        if (inDocBlock) {
            if (line.includes('*/')) {
                inDocBlock = false;
                pendingDoc = collectDocBlock(lines, docBlockOpenLine!, i);
            }
            continue;
        }
        const docOpen = line.indexOf('/**');
        if (docOpen !== -1) {
            if (line.indexOf('*/', docOpen + 3) !== -1) {
                pendingDoc = collectDocBlock(lines, i, i);
            } else {
                inDocBlock = true;
                docBlockOpenLine = i;
            }
            continue;
        }

        // Strip line comment for matching, but keep column info aligned via
        // the original line.
        const codeLine = stripLineComment(line);

        // We only treat top-level bindings as "globals" the user expects in
        // completion. Procs nested inside other procs become params/locals
        // of the outer proc, not module-level symbols.
        const isTopLevel = braceDepth === 0;

        let m: RegExpMatchArray | null;
        if ((m = codeLine.match(PROC_RE))) {
            const indent = m[1] ?? '';
            const name = m[2];
            const paramsRaw = m[3];
            const params = paramsRaw
                .split(',')
                .map((p) => p.trim())
                .filter((p) => p.length > 0);
            const nameCol = indent.length + 'proc '.length;

            if (isTopLevel) {
                const sym: OMGSymbol = {
                    name,
                    kind: 'proc',
                    detail: `proc ${name}(${params.join(', ')})`,
                    uri,
                    line: i,
                    col: nameCol,
                    params,
                    docComment: pendingDoc
                };
                topLevel.push(sym);

                // Find the body extent so params get a proper scope range.
                const { closeLine } = findMatchingBrace(lines, i);
                const paramSyms: OMGSymbol[] = params.map((p, idx) => ({
                    name: p,
                    kind: 'param',
                    detail: `${p} (parameter of ${name})`,
                    uri,
                    line: i,
                    // Approximate column: just past the `(`.
                    col: line.indexOf('(') + 1 + idx,
                    scopeStart: i,
                    scopeEnd: closeLine
                }));
                paramsByProc.set(name, paramSyms);
            }

            activeProcStack.push({ name, startLine: i });
            pendingDoc = undefined;
        } else if ((m = codeLine.match(ALLOC_RE)) && isTopLevel) {
            const indent = m[1] ?? '';
            const name = m[2];
            const sym: OMGSymbol = {
                name,
                kind: 'alloc',
                detail: `alloc ${name}`,
                uri,
                line: i,
                col: indent.length + 'alloc '.length,
                docComment: pendingDoc
            };
            topLevel.push(sym);
            pendingDoc = undefined;
        } else if ((m = codeLine.match(IMPORT_RE))) {
            const rawPath = m[1];
            const alias = m[2];
            const aliasCol = line.indexOf(' as ') + ' as '.length;
            imports.push({ rawPath, alias, line: i, col: aliasCol });
            topLevel.push({
                name: alias,
                kind: 'import',
                detail: `import "${rawPath}"`,
                uri,
                line: i,
                col: aliasCol
            });
            pendingDoc = undefined;
        } else if (codeLine.trim() !== '') {
            // Any non-trivial line breaks the pending doc-comment streak.
            pendingDoc = undefined;
        }

        // Update brace depth (ignoring braces inside string literals).
        braceDepth += countBraces(codeLine);
        if (braceDepth < 0) {
            braceDepth = 0;
        }
        // Pop fn stack if we left a proc body.
        while (
            activeProcStack.length > 0 &&
            braceDepth === 0 &&
            i >= activeProcStack[activeProcStack.length - 1].startLine
        ) {
            // If we're back at brace depth 0 we're outside any proc.
            // (Heuristic — good enough for line-based scoping.)
            if (
                line.includes('}') &&
                lineClosesProc(lines, activeProcStack[activeProcStack.length - 1].startLine, i)
            ) {
                activeProcStack.pop();
            } else {
                break;
            }
        }
    }

    return { uri, topLevel, paramsByProc, imports, hasHeader };
}

function detectHeader(lines: string[]): boolean {
    for (const line of lines) {
        const trimmed = line.trim();
        if (trimmed === '') continue;
        return trimmed === ';;;omg';
    }
    return false;
}

function stripLineComment(line: string): string {
    // Don't strip `#` inside strings. Walk and respect quotes.
    let inStr = false;
    let escape = false;
    for (let i = 0; i < line.length; i++) {
        const c = line[i];
        if (escape) {
            escape = false;
            continue;
        }
        if (c === '\\') {
            escape = true;
            continue;
        }
        if (c === '"') {
            inStr = !inStr;
            continue;
        }
        if (!inStr && c === '#') {
            return line.slice(0, i);
        }
    }
    return line;
}

function countBraces(line: string): number {
    let count = 0;
    let inStr = false;
    let escape = false;
    for (let i = 0; i < line.length; i++) {
        const c = line[i];
        if (escape) {
            escape = false;
            continue;
        }
        if (c === '\\') {
            escape = true;
            continue;
        }
        if (c === '"') {
            inStr = !inStr;
            continue;
        }
        if (inStr) continue;
        if (c === '{') count++;
        else if (c === '}') count--;
    }
    return count;
}

/**
 * Heuristic: from the `proc name(...)` line, walk forward tracking braces
 * and return the line index where the matching `}` lives.
 */
function findMatchingBrace(
    lines: string[],
    procLine: number
): { closeLine: number } {
    let depth = 0;
    let started = false;
    for (let i = procLine; i < lines.length; i++) {
        const delta = countBraces(stripLineComment(lines[i]));
        depth += delta;
        if (delta > 0) started = true;
        if (started && depth === 0) {
            return { closeLine: i };
        }
    }
    // Unbalanced — let the user keep typing.
    return { closeLine: lines.length - 1 };
}

function lineClosesProc(
    lines: string[],
    procLine: number,
    currentLine: number
): boolean {
    let depth = 0;
    for (let i = procLine; i <= currentLine; i++) {
        depth += countBraces(stripLineComment(lines[i]));
    }
    return depth <= 0;
}

function collectDocBlock(
    lines: string[],
    open: number,
    close: number
): string {
    const parts: string[] = [];
    for (let i = open; i <= close; i++) {
        let line = lines[i];
        line = line.replace(/\/\*\*|\*\//g, '');
        line = line.replace(/^\s*\*\s?/, '');
        parts.push(line.trim());
    }
    return parts.join('\n').trim();
}

/**
 * Resolve an `import "rel"` path against the importing file's directory.
 * Returns `undefined` if the file URI isn't a regular `file://` URI or the
 * resolved file doesn't exist on disk.
 */
export function resolveImportUri(
    importingUri: string,
    rawPath: string
): string | undefined {
    let p: string;
    try {
        p = URI.parse(importingUri).fsPath;
    } catch {
        return undefined;
    }
    const dir = path.dirname(p);
    const candidate = path.resolve(dir, rawPath);
    try {
        if (!fs.statSync(candidate).isFile()) return undefined;
    } catch {
        return undefined;
    }
    return URI.file(candidate).toString();
}
