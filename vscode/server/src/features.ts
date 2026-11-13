// Hover, completion and signature help.

import * as fs from 'fs';
import * as path from 'path';
import {
    CompletionItem, CompletionItemKind, Hover, MarkupKind,
    ParameterInformation, Position, Range, SignatureHelp, SignatureInformation
} from 'vscode-languageserver/node';

import { FileAnalysis, Sym } from './analysis';
import { BUILTINS, BuiltinDoc, RAISERS, RAW_OPS, RESERVED_GLOBALS, callable, rawOp } from './builtins';
import { Inference, Resolved, Value } from './infer';
import { HEADER_DOC, OPERATOR_DOCS, keywordDoc } from './language';
import { Token, isKeyword, offsetAt, positionAt } from './lexer';
import {
    builtinMarkdown, globalMarkdown, intLiteralMarkdown, keywordMarkdown, operatorMarkdown,
    paramDocOf, rawOpMarkdown, resolvedMarkdown, signatureOf, symbolMarkdown
} from './render';
import { Workspace, uriToPath } from './workspace';

export function toOffset(file: FileAnalysis, pos: Position): number {
    return offsetAt(file.lex.lineStarts, file.text.length, pos.line, pos.character);
}

export function toRange(file: FileAnalysis, start: number, end: number): Range {
    return Range.create(positionAt(file.lex.lineStarts, start), positionAt(file.lex.lineStarts, end));
}

function md(value: string): { kind: 'markdown'; value: string } {
    return { kind: MarkupKind.Markdown, value };
}

// ---- Hover ------------------------------------------------------------------

export function hover(ws: Workspace, file: FileAnalysis, pos: Position): Hover | null {
    const offset = toOffset(file, pos);
    const header = file.lex.header;
    if (header && offset >= header.start && offset <= header.end) {
        return { contents: md(HEADER_DOC), range: toRange(file, header.start, header.end) };
    }
    if (file.commentAt(offset)) return null;

    const occ = file.occurrenceAt(offset);
    if (occ) {
        const r = ws.infer.resolve(file, occ);
        if (r) {
            const text = resolvedMarkdown(ws.infer, r, file);
            if (text) return { contents: md(text), range: toRange(file, occ.start, occ.end) };
        }
        if (occ.target.t !== 'unresolved' && occ.target.t !== 'member') return null;
    }

    const tok = file.tokenAt(offset) ?? file.tokenAt(offset - 1);
    if (!tok) return null;
    const range = toRange(file, tok.start, tok.end);
    if (tok.kind === 'STR') {
        // The path of an import says what the alias does.
        for (const imp of file.imports) {
            const node = imp.node;
            if (node && node.type === 'import' && offset >= node.pathSpan.start && offset <= node.pathSpan.end) {
                return { contents: md(symbolMarkdown(ws.infer, imp, file)), range };
            }
        }
        return null;
    }
    if (tok.kind === 'ID') {
        // Code that did not parse: fall back to the name alone.
        const i = file.lex.tokens.indexOf(tok);
        const afterDot = i > 0 && file.lex.tokens[i - 1].kind === 'DOT';
        if (afterDot) {
            const names = chainBefore(file, i);
            if (names) {
                const got = ws.infer.chain(file, names, offset);
                const text = got.resolved ? resolvedMarkdown(ws.infer, got.resolved, file) : undefined;
                if (text) return { contents: md(text), range };
            }
            return null;
        }
        const r = ws.infer.resolveTarget(file, file.lookup(tok.text, offset));
        const text = r ? resolvedMarkdown(ws.infer, r, file) : undefined;
        return text ? { contents: md(text), range } : null;
    }
    if (isKeyword(tok.text)) {
        const doc = keywordDoc(tok.text);
        return doc ? { contents: md(keywordMarkdown(doc)), range } : null;
    }
    if (tok.kind === 'NUM' && typeof tok.value === 'bigint') {
        return { contents: md(intLiteralMarkdown(tok.text, tok.value)), range };
    }
    const op = OPERATOR_DOCS[tok.kind];
    if (op) return { contents: md(operatorMarkdown(op)), range };
    return null;
}

/** The names of `a.b.c` ending at token `i` (an ID), or undefined when it is not a plain chain. */
function chainBefore(file: FileAnalysis, i: number): string[] | undefined {
    const toks = file.lex.tokens;
    const names: string[] = [toks[i].text];
    let j = i - 1;
    while (j >= 1 && toks[j].kind === 'DOT') {
        if (toks[j - 1].kind !== 'ID') return undefined;
        names.unshift(toks[j - 1].text);
        j -= 2;
    }
    return names;
}

// ---- Completion -------------------------------------------------------------

const STATEMENT_KEYWORDS = ['alloc', 'proc', 'if', 'loop', 'for', 'try', 'import', 'emit', 'facts', 'return', 'break', 'continue'];
const EXPRESSION_KEYWORDS = ['true', 'false', 'not', 'and', 'or'];
const DECLARING: ReadonlySet<string> = new Set(['ALLOC', 'FUNC', 'FOR', 'EXCEPT', 'AS']);

export function completion(ws: Workspace, file: FileAnalysis, pos: Position): CompletionItem[] {
    const offset = toOffset(file, pos);
    if (file.commentAt(offset)) return [];
    const header = file.lex.header;
    if (header && offset > header.start && offset <= header.end) return [];
    const toks = file.lex.tokens;

    const str = file.stringAt(offset);
    if (str) {
        const si = toks.indexOf(str);
        if (si > 0 && toks[si - 1].kind === 'IMPORT') return importPathItems(file, str, offset);
        return [];
    }

    // `before`: the last whole token to the left of whatever is being typed.
    let bi = file.tokenIndexBefore(offset);
    if (bi >= 0) {
        const t = toks[bi];
        const word = t.kind === 'ID' || isKeyword(t.text);
        if (word && offset <= t.end) bi--;
    }
    const before: Token | undefined = bi >= 0 ? toks[bi] : undefined;
    const kind = before?.kind;

    if (kind === 'DOT') {
        if (bi < 1 || toks[bi - 1].kind !== 'ID') return [];
        const names = chainBefore(file, bi - 1);
        if (!names) return [];
        const got = ws.infer.chain(file, names, offset);
        return memberItems(ws.infer, got.value, file);
    }
    if (kind === 'NUM' || kind === 'FNUM') {
        if (before && before.end === offset) return [];
    }
    if (kind && DECLARING.has(kind)) return [];
    if (kind === 'IMPORT') return [];
    if (kind === 'STR' && bi >= 1 && toks[bi - 1].kind === 'IMPORT') {
        return [{ label: 'as', kind: CompletionItemKind.Keyword, documentation: md(keywordMarkdown(keywordDoc('as')!)) }];
    }
    if (kind === 'ID' && bi >= 1 && toks[bi - 1].kind === 'FOR') {
        return [{ label: 'in', kind: CompletionItemKind.Keyword, documentation: md(keywordMarkdown(keywordDoc('in')!)) }];
    }
    if (inParameterList(toks, bi)) return [];

    const out: CompletionItem[] = [];
    const atStatement = before === undefined || kind === 'NL' || kind === 'LBRACE' || kind === 'RBRACE';
    const keywords = atStatement ? STATEMENT_KEYWORDS.slice() : EXPRESSION_KEYWORDS.slice();
    if (kind === 'RBRACE') keywords.push('elif', 'else', 'except');
    for (const kw of keywords) {
        const doc = keywordDoc(kw);
        out.push({
            label: kw,
            kind: CompletionItemKind.Keyword,
            detail: doc?.syntax,
            documentation: doc ? md(keywordMarkdown(doc)) : undefined,
            sortText: '4' + kw
        });
    }

    const seen = new Set<string>();
    for (const sym of file.visibleSymbols(offset)) {
        // The name being declared on this very line is not a candidate yet.
        if (offset >= sym.start && offset <= sym.end) continue;
        seen.add(sym.name);
        out.push(symbolItem(ws.infer, sym, file, sym.global ? '1' : '0'));
    }

    if (file.rawMode) {
        for (const r of RAW_OPS) {
            out.push({
                label: r.name, kind: CompletionItemKind.Function, detail: r.signature,
                documentation: md(rawOpMarkdown(r)), sortText: '3' + r.name
            });
        }
    } else {
        for (const b of [...BUILTINS, ...RAISERS]) {
            out.push(builtinItem(b, seen.has(b.name) ? '5' : '2'));
        }
    }
    for (const g of RESERVED_GLOBALS) {
        if (seen.has(g.name)) continue;
        out.push({
            label: g.name, kind: CompletionItemKind.Constant, detail: 'built-in global · ' + g.type,
            documentation: md(globalMarkdown(g)), sortText: '2' + g.name
        });
    }
    return out;
}

/** Inside `proc name(a, b|`: these are new names. */
function inParameterList(toks: Token[], bi: number): boolean {
    let j = bi;
    while (j >= 0 && (toks[j].kind === 'ID' || toks[j].kind === 'COMMA' || toks[j].kind === 'NL')) j--;
    return j >= 2 && toks[j].kind === 'LPAREN' && toks[j - 1].kind === 'ID' && toks[j - 2].kind === 'FUNC';
}

function builtinItem(b: BuiltinDoc, sort: string): CompletionItem {
    return {
        label: b.name,
        kind: CompletionItemKind.Function,
        detail: b.signatures.join('  |  '),
        documentation: md(builtinMarkdown(b)),
        sortText: sort + b.name
    };
}

function symbolKind(sym: Sym): CompletionItemKind {
    switch (sym.kind) {
        case 'proc': return CompletionItemKind.Function;
        case 'import': return CompletionItemKind.Module;
        case 'param': return CompletionItemKind.Variable;
        default:
            return /^[A-Z][A-Z0-9_]*$/.test(sym.name) ? CompletionItemKind.Constant : CompletionItemKind.Variable;
    }
}

function symbolItem(infer: Inference, sym: Sym, from: FileAnalysis, sort: string): CompletionItem {
    let detail: string;
    switch (sym.kind) {
        case 'proc': detail = signatureOf(sym); break;
        case 'import': detail = 'import "' + (sym.importPath ?? '') + '"'; break;
        case 'param': detail = 'parameter'; break;
        case 'loopvar': detail = 'loop variable'; break;
        case 'except': detail = 'caught error'; break;
        default: detail = sym.global ? 'global variable' : 'local variable';
    }
    return {
        label: sym.name,
        kind: symbolKind(sym),
        detail,
        documentation: md(symbolMarkdown(infer, sym, from)),
        sortText: sort + sym.name
    };
}

function memberItems(infer: Inference, container: Value | undefined, from: FileAnalysis): CompletionItem[] {
    if (!container) return [];
    const out: CompletionItem[] = [];
    if (container.k === 'module') {
        for (const sym of container.file.exports.values()) {
            const item = symbolItem(infer, sym, from, sym.kind === 'proc' ? '0' : '1');
            if (sym.name.startsWith('_')) item.sortText = '9' + sym.name;
            out.push(item);
        }
    } else if (container.k === 'dict') {
        for (const fld of container.fields.values()) {
            const v = fld.value ? infer.valueOf(fld.file, fld.value, 0) : undefined;
            const r: Resolved = { k: 'field', field: fld, frozen: container.frozen };
            out.push({
                label: fld.name,
                kind: v && v.k === 'proc' ? CompletionItemKind.Method : CompletionItemKind.Field,
                detail: v && v.k === 'proc' ? signatureOf(v.sym) : 'dict entry',
                documentation: md(resolvedMarkdown(infer, r, from) ?? ''),
                sortText: '0' + fld.name
            });
        }
    }
    return out;
}

function importPathItems(file: FileAnalysis, str: Token, offset: number): CompletionItem[] {
    const self = uriToPath(file.uri);
    if (!self) return [];
    const typed = file.text.slice(str.start + 1, offset);
    const slash = typed.lastIndexOf('/');
    const dirPart = slash >= 0 ? typed.slice(0, slash + 1) : '';
    const dir = path.resolve(path.dirname(self), dirPart);
    let names: fs.Dirent[];
    try {
        names = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
        return [];
    }
    const start = str.start + 1 + dirPart.length;
    // Replace up to the closing quote, or to the cursor in an open string.
    const closed = str.end - str.start >= 2 && file.text[str.end - 1] === '"';
    let end = closed ? str.end - 1 : offset;
    const nl = file.text.indexOf('\n', offset);
    if (nl >= 0 && nl < end) end = offset;
    const range = toRange(file, start, end);
    const out: CompletionItem[] = [];
    for (const d of names) {
        if (d.name.startsWith('.')) continue;
        if (d.isDirectory()) {
            if (d.name === 'node_modules') continue;
            out.push({
                label: d.name + '/', kind: CompletionItemKind.Folder,
                textEdit: { range, newText: d.name + '/' },
                command: { title: 'Suggest', command: 'editor.action.triggerSuggest' },
                sortText: '1' + d.name
            });
        } else if (d.name.endsWith('.omg') && path.join(dir, d.name) !== self) {
            out.push({
                label: d.name, kind: CompletionItemKind.File,
                textEdit: { range, newText: d.name }, sortText: '0' + d.name
            });
        }
    }
    if (dirPart === '' || /^(\.\.\/)*$/.test(dirPart)) {
        out.push({
            label: '../', kind: CompletionItemKind.Folder,
            textEdit: { range, newText: '../' },
            command: { title: 'Suggest', command: 'editor.action.triggerSuggest' },
            sortText: '2'
        });
    }
    return out;
}

// ---- Signature help -----------------------------------------------------------

const STATEMENT_STARTERS: ReadonlySet<string> = new Set([
    'ALLOC', 'EMIT', 'FACTS', 'IMPORT', 'IF', 'ELIF', 'ELSE', 'LOOP', 'FOR', 'BREAK', 'CONTINUE',
    'FUNC', 'RETURN', 'TRY', 'EXCEPT', 'ASSIGN', 'EOF'
]);

/** The call the offset is inside of: where its `(` is and how many commas precede the offset. */
function enclosingCall(file: FileAnalysis, offset: number): { paren: number; commas: number } | undefined {
    const toks = file.lex.tokens;
    let depth = 0;
    let commas = 0;
    let steps = 0;
    for (let j = file.tokenIndexBefore(offset); j >= 0 && steps < 2000; j--, steps++) {
        const t = toks[j];
        // A closer the cursor sits right after still encloses nothing.
        if (t.end > offset && t.kind !== 'STR') continue;
        switch (t.kind) {
            case 'RPAREN': case 'RBRACK': case 'RBRACE':
                depth++;
                break;
            case 'LBRACK': case 'LBRACE':
                if (depth === 0) commas = 0; else depth--;
                break;
            case 'LPAREN':
                if (depth === 0) return { paren: j, commas };
                depth--;
                break;
            case 'COMMA':
                if (depth === 0) commas++;
                break;
            default:
                if (depth === 0 && STATEMENT_STARTERS.has(t.kind)) return undefined;
        }
    }
    return undefined;
}

export function signatureHelp(ws: Workspace, file: FileAnalysis, pos: Position): SignatureHelp | null {
    const offset = toOffset(file, pos);
    if (file.commentAt(offset)) return null;
    const found = enclosingCall(file, offset);
    if (!found) return null;
    const toks = file.lex.tokens;
    const ci = found.paren - 1;
    if (ci < 0 || toks[ci].kind !== 'ID') return null;
    if (ci >= 1 && toks[ci - 1].kind === 'FUNC') return null;
    const names = chainBefore(file, ci);
    if (!names) return null;
    const active = found.commas;

    if (names.length === 1) {
        const name = names[0];
        // The compiler's order: a builtin name called is the builtin.
        const raw = file.rawMode ? rawOp(name) : undefined;
        if (raw) {
            const params = raw.signature.slice(raw.signature.indexOf('(') + 1, -1).split(',').map((s) => s.trim()).filter((s) => s !== '');
            return {
                signatures: [makeSignature(raw.name, params, raw.summary, () => undefined)],
                activeSignature: 0,
                activeParameter: Math.min(active, Math.max(0, params.length - 1))
            };
        }
        const b = file.rawMode ? undefined : callable(name);
        if (b) return builtinSignatures(b, active);
    }

    const got = ws.infer.chain(file, names, toks[ci].start);
    const v = got.value;
    if (!v || v.k !== 'proc') return null;
    const sym = v.sym;
    const params = (sym.params ?? []).map((p) => p.name);
    const label = names[names.length - 1] === sym.name ? sym.name : names[names.length - 1];
    const doc = sym.doc ? stripParamLines(sym.doc) : undefined;
    return {
        signatures: [makeSignature(label, params, doc, (p) => paramDocOf(sym, p))],
        activeSignature: 0,
        activeParameter: Math.min(active, Math.max(0, params.length - 1))
    };
}

function stripParamLines(doc: string): string {
    return doc.split('\n').filter((l) => !/^- `[A-Za-z_][A-Za-z0-9_]*`: /.test(l)).join('\n').trim();
}

function makeSignature(
    name: string, params: string[], doc: string | undefined, paramDocFor: (p: string) => string | undefined
): SignatureInformation {
    let label = name + '(';
    const infos: ParameterInformation[] = [];
    params.forEach((p, i) => {
        if (i > 0) label += ', ';
        const start = label.length;
        label += p;
        const d = paramDocFor(p);
        infos.push({ label: [start, label.length], documentation: d ? md(d) : undefined });
    });
    label += ')';
    return { label, documentation: doc ? md(doc) : undefined, parameters: infos };
}

function builtinSignatures(b: BuiltinDoc, active: number): SignatureHelp {
    const signatures: SignatureInformation[] = [];
    let chosen = -1;
    b.signatures.forEach((sig, i) => {
        const inside = sig.slice(sig.indexOf('(') + 1, -1).trim();
        const names = inside === '' ? [] : inside.split(',').map((s) => s.trim());
        const summary = b.summary + (b.returns ? '\n\n**Returns** ' + b.returns.charAt(0).toLowerCase() + b.returns.slice(1) : '');
        signatures.push(makeSignature(b.name, names, summary, (p) => b.params.find((x) => x.name === p)?.doc));
        if (chosen < 0 && active < Math.max(1, names.length)) chosen = i;
    });
    if (chosen < 0) chosen = signatures.length - 1;
    const count = signatures[chosen].parameters?.length ?? 0;
    return {
        signatures,
        activeSignature: chosen,
        activeParameter: Math.min(active, Math.max(0, count - 1))
    };
}
