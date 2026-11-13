// Go to definition, find references, rename, highlights, outline,
// folding and links.

import * as path from 'path';
import {
    DocumentHighlight, DocumentHighlightKind, DocumentLink, DocumentSymbol, FoldingRange,
    FoldingRangeKind, Location, Position, Range, SymbolInformation, SymbolKind, TextEdit,
    WorkspaceEdit
} from 'vscode-languageserver/node';

import { Block, Stmt, isBlock } from './ast';
import { FileAnalysis, Field, Occurrence, Sym } from './analysis';
import { toOffset, toRange } from './features';
import { Resolved } from './infer';
import { isKeyword, positionAt } from './lexer';
import { Workspace, pathToUri, uriToPath } from './workspace';

function locate(file: FileAnalysis, start: number, end: number): Location {
    return Location.create(file.uri, toRange(file, start, end));
}

/** What the cursor is on, as something that has a definition. */
function resolveAt(ws: Workspace, file: FileAnalysis, offset: number): { occ: Occurrence; r: Resolved } | undefined {
    const occ = file.occurrenceAt(offset);
    if (!occ) return undefined;
    const r = ws.infer.resolve(file, occ);
    return r ? { occ, r } : undefined;
}

export function definition(ws: Workspace, file: FileAnalysis, pos: Position): Location | null {
    const offset = toOffset(file, pos);
    const hit = resolveAt(ws, file, offset);
    if (hit) {
        if (hit.r.k === 'sym') {
            const sym = hit.r.sym;
            // On an import's own alias, go on to the file it names.
            if (sym.kind === 'import' && hit.occ.decl) {
                const mod = ws.moduleOf(sym);
                if (mod) return Location.create(mod.uri, Range.create(0, 0, 0, 0));
            }
            return locate(sym.file, sym.start, sym.end);
        }
        if (hit.r.k === 'field') return locate(hit.r.field.file, hit.r.field.span.start, hit.r.field.span.end);
        return null;
    }
    // The path of an import.
    const str = file.stringAt(offset) ?? file.tokenAt(offset);
    if (str && str.kind === 'STR') {
        const i = file.lex.tokens.indexOf(str);
        if (i > 0 && file.lex.tokens[i - 1].kind === 'IMPORT' && typeof str.value === 'string') {
            const p = ws.importPath(file, str.value);
            if (p && ws.get(pathToUri(p))) return Location.create(pathToUri(p), Range.create(0, 0, 0, 0));
        }
    }
    return null;
}

interface Use {
    file: FileAnalysis;
    start: number;
    end: number;
    write: boolean;
    decl: boolean;
}

function sameField(a: Field, b: Field): boolean {
    return a.file === b.file && a.span.start === b.span.start;
}

/** Every place that names the same thing, across the files that can see it. */
function usesOf(ws: Workspace, r: Resolved, widen: boolean): Use[] {
    const out: Use[] = [];
    if (r.k === 'sym') {
        const sym = r.sym;
        for (const ref of sym.refs) out.push({ file: sym.file, start: ref.start, end: ref.end, write: ref.write, decl: ref.decl });
        if (sym.exported || (sym.global && sym.file.rawMode)) {
            if (widen) ws.scan();
            for (const other of ws.all()) {
                if (other === sym.file) continue;
                for (const occ of other.occurrences) {
                    const t = occ.target;
                    const named = (t.t === 'member' || t.t === 'unresolved' || t.t === 'raw') && t.name === sym.name;
                    if (!named) continue;
                    const rr = ws.infer.resolve(other, occ);
                    if (rr && rr.k === 'sym' && rr.sym === sym) {
                        out.push({ file: other, start: occ.start, end: occ.end, write: occ.write, decl: false });
                    }
                }
            }
        }
    } else if (r.k === 'field') {
        const fld = r.field;
        const file = fld.file;
        out.push({ file, start: fld.span.start, end: fld.span.end, write: true, decl: true });
        for (const occ of file.occurrences) {
            if (occ.target.t !== 'member' || occ.target.name !== fld.name) continue;
            if (occ.start === fld.span.start) continue;
            const rr = ws.infer.resolve(file, occ);
            if (rr && rr.k === 'field' && sameField(rr.field, fld)) {
                out.push({ file, start: occ.start, end: occ.end, write: occ.write, decl: false });
            }
        }
    }
    return out;
}

export function references(ws: Workspace, file: FileAnalysis, pos: Position, includeDecl: boolean): Location[] {
    const hit = resolveAt(ws, file, toOffset(file, pos));
    if (!hit) return [];
    return usesOf(ws, hit.r, true)
        .filter((u) => includeDecl || !u.decl)
        .map((u) => locate(u.file, u.start, u.end));
}

export function highlights(ws: Workspace, file: FileAnalysis, pos: Position): DocumentHighlight[] {
    const hit = resolveAt(ws, file, toOffset(file, pos));
    if (!hit) return [];
    return usesOf(ws, hit.r, false)
        .filter((u) => u.file === file)
        .map((u) => DocumentHighlight.create(
            toRange(file, u.start, u.end),
            u.write ? DocumentHighlightKind.Write : DocumentHighlightKind.Read
        ));
}

export function prepareRename(ws: Workspace, file: FileAnalysis, pos: Position): { range: Range; placeholder: string } | null {
    const hit = resolveAt(ws, file, toOffset(file, pos));
    if (!hit) return null;
    if (hit.r.k !== 'sym' && hit.r.k !== 'field') return null;
    const text = file.text.slice(hit.occ.start, hit.occ.end);
    // A quoted dict key is a string, not a name.
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(text)) return null;
    return { range: toRange(file, hit.occ.start, hit.occ.end), placeholder: text };
}

export function rename(ws: Workspace, file: FileAnalysis, pos: Position, newName: string): WorkspaceEdit | string {
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(newName)) {
        return "'" + newName + "' is not a valid name: use letters, digits and underscores, not starting with a digit.";
    }
    if (isKeyword(newName)) return "'" + newName + "' is a keyword.";
    const hit = resolveAt(ws, file, toOffset(file, pos));
    if (!hit || (hit.r.k !== 'sym' && hit.r.k !== 'field')) return 'This cannot be renamed.';
    const changes: Record<string, TextEdit[]> = {};
    const seen = new Set<string>();
    const uses = usesOf(ws, hit.r, true);
    if (hit.r.k === 'sym') {
        // The new name must not already mean something where the old one
        // is used: the program would compile and do something else.
        const renamed = hit.r.sym;
        for (const u of uses) {
            if (u.file !== renamed.file) continue;
            const taken = u.file.lookup(newName, u.start);
            if (taken.t === 'builtin' || taken.t === 'raiser') return "'" + newName + "' is a builtin.";
            if (taken.t === 'reserved') return "'" + newName + "' is a built-in global.";
            if (taken.t === 'sym' && taken.sym !== renamed) {
                const line = positionAt(taken.sym.file.lex.lineStarts, taken.sym.start).line + 1;
                return "'" + newName + "' is already in use here (declared on line " + line + ').';
            }
        }
    }
    for (const u of uses) {
        const key = u.file.uri + ':' + u.start;
        if (seen.has(key)) continue;
        seen.add(key);
        if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(u.file.text.slice(u.start, u.end))) continue;
        (changes[u.file.uri] ??= []).push(TextEdit.replace(toRange(u.file, u.start, u.end), newName));
    }
    return { changes };
}

// ---- Outline ------------------------------------------------------------------

function symbolKindOf(sym: Sym): SymbolKind {
    switch (sym.kind) {
        case 'proc': return SymbolKind.Function;
        case 'import': return SymbolKind.Module;
        default:
            return /^[A-Z][A-Z0-9_]*$/.test(sym.name) ? SymbolKind.Constant : SymbolKind.Variable;
    }
}

export function documentSymbols(file: FileAnalysis): DocumentSymbol[] {
    const build = (stmts: Stmt[], top: boolean): DocumentSymbol[] => {
        const out: DocumentSymbol[] = [];
        const visit = (list: Stmt[]): void => {
            for (const s of list) {
                switch (s.type) {
                    case 'func': {
                        const children = build(s.body.stmts, false);
                        out.push({
                            name: s.name.name,
                            detail: '(' + s.params.map((p) => p.name).join(', ') + ')',
                            kind: SymbolKind.Function,
                            range: toRange(file, s.start, s.end),
                            selectionRange: toRange(file, s.name.start, s.name.end),
                            children
                        });
                        break;
                    }
                    case 'decl':
                        if (!top) break;
                        out.push({
                            name: s.name.name,
                            kind: /^[A-Z][A-Z0-9_]*$/.test(s.name.name) ? SymbolKind.Constant : SymbolKind.Variable,
                            range: toRange(file, s.start, s.end),
                            selectionRange: toRange(file, s.name.start, s.name.end)
                        });
                        break;
                    case 'import':
                        if (!top) break;
                        out.push({
                            name: s.alias.name,
                            detail: s.path,
                            kind: SymbolKind.Module,
                            range: toRange(file, s.start, s.end),
                            selectionRange: toRange(file, s.alias.start, s.alias.end)
                        });
                        break;
                    case 'if':
                        visit(s.then.stmts);
                        for (let e = s.otherwise; e; ) {
                            if (isBlock(e)) { visit(e.stmts); break; }
                            visit(e.then.stmts);
                            e = e.otherwise;
                        }
                        break;
                    case 'loop':
                    case 'for':
                        visit(s.body.stmts);
                        break;
                    case 'try':
                        visit(s.body.stmts);
                        visit(s.handler.stmts);
                        break;
                    default:
                        break;
                }
            }
        };
        visit(stmts);
        return out;
    };
    return build(file.stmts, true);
}

export function workspaceSymbols(ws: Workspace, query: string): SymbolInformation[] {
    ws.scan();
    const q = query.toLowerCase();
    const matches = (name: string): boolean => {
        if (q === '') return true;
        const n = name.toLowerCase();
        let i = 0;
        for (const ch of n) {
            if (ch === q[i]) i++;
            if (i === q.length) return true;
        }
        return false;
    };
    const out: SymbolInformation[] = [];
    for (const file of ws.all()) {
        const p = uriToPath(file.uri);
        const container = p ? path.basename(p) : undefined;
        for (const sym of file.globals.values()) {
            if (sym.kind !== 'proc' && sym.kind !== 'var') continue;
            if (!matches(sym.name)) continue;
            out.push(SymbolInformation.create(sym.name, symbolKindOf(sym), toRange(file, sym.start, sym.end), file.uri, container));
            if (out.length >= 2000) return out;
        }
    }
    return out;
}

// ---- Folding and links ----------------------------------------------------------

export function foldingRanges(file: FileAnalysis): FoldingRange[] {
    const out: FoldingRange[] = [];
    const lineOf = (offset: number): number => toRange(file, offset, offset).start.line;
    const fold = (b: Block): void => {
        const a = lineOf(b.start);
        const z = lineOf(Math.max(b.start, b.end - 1));
        // Keep the closing `}` on show.
        if (z - 1 > a) out.push(FoldingRange.create(a, b.closed ? z - 1 : z));
        else if (z > a && !b.closed) out.push(FoldingRange.create(a, z));
        stmts(b.stmts);
    };
    const stmts = (list: Stmt[]): void => {
        let importStart = -1;
        let importEnd = -1;
        const flushImports = (): void => {
            if (importStart >= 0 && importEnd > importStart) {
                out.push(FoldingRange.create(importStart, importEnd, undefined, undefined, FoldingRangeKind.Imports));
            }
            importStart = -1;
        };
        for (const s of list) {
            if (s.type === 'import') {
                const l = lineOf(s.start);
                if (importStart < 0) importStart = l;
                importEnd = l;
                continue;
            }
            flushImports();
            switch (s.type) {
                case 'func':
                case 'loop':
                case 'for':
                    fold(s.body);
                    break;
                case 'if':
                    fold(s.then);
                    for (let e = s.otherwise; e; ) {
                        if (isBlock(e)) { fold(e); break; }
                        fold(e.then);
                        e = e.otherwise;
                    }
                    break;
                case 'try':
                    fold(s.body);
                    fold(s.handler);
                    break;
                default: {
                    // A list or dict literal spread over several lines.
                    const a = lineOf(s.start);
                    const z = lineOf(s.end);
                    if (z - 1 > a) out.push(FoldingRange.create(a, z - 1));
                }
            }
        }
        flushImports();
    };
    stmts(file.stmts);

    // Comments: a block, or a run of `#` lines.
    const cs = file.lex.comments;
    const lineStarts = file.lex.lineStarts;
    const ownLine = (start: number, line: number): boolean => file.text.slice(lineStarts[line], start).trim() === '';
    for (let i = 0; i < cs.length; i++) {
        const c = cs[i];
        if (c.block) {
            if (c.endLine > c.startLine) out.push(FoldingRange.create(c.startLine, c.endLine, undefined, undefined, FoldingRangeKind.Comment));
            continue;
        }
        if (!ownLine(c.start, c.startLine)) continue;
        let j = i;
        while (j + 1 < cs.length && !cs[j + 1].block && cs[j + 1].startLine === cs[j].startLine + 1 && ownLine(cs[j + 1].start, cs[j + 1].startLine)) j++;
        if (j > i) out.push(FoldingRange.create(c.startLine, cs[j].startLine, undefined, undefined, FoldingRangeKind.Comment));
        i = j;
    }
    return out;
}

export function documentLinks(ws: Workspace, file: FileAnalysis): DocumentLink[] {
    const out: DocumentLink[] = [];
    for (const imp of file.imports) {
        const node = imp.node;
        if (!node || node.type !== 'import') continue;
        const p = ws.importPath(file, node.path);
        if (!p || !ws.get(pathToUri(p))) continue;
        out.push({
            range: toRange(file, node.pathSpan.start + 1, node.pathSpan.end - 1),
            target: pathToUri(p),
            tooltip: 'Open ' + path.basename(p)
        });
    }
    return out;
}
