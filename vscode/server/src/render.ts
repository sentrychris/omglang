// Markdown for hovers, completion items and signature help.

import * as path from 'path';

import { FileAnalysis, Sym } from './analysis';
import { BuiltinDoc, GlobalDoc, RawOpDoc } from './builtins';
import { Inference, Resolved, Value, describeValue } from './infer';
import { KeywordDoc, OperatorDoc, PRECEDENCE } from './language';
import { uriToPath } from './workspace';

export function omg(src: string): string {
    return '```omg\n' + src + '\n```';
}

/**
 * Markdown reads `<top-level>` as a tag and drops it, so a `<` in
 * someone's comment is written as an entity. Not inside a code span or
 * an indented block, where it is already literal.
 */
function prose(text: string): string {
    return text
        .split('\n')
        .map((l) => {
            if (/^( {4}|\t)/.test(l)) return l;
            return l.split(/(`[^`]*`)/).map((part, i) => (i % 2 === 1 ? part : part.replace(/</g, '&lt;'))).join('');
        })
        .join('\n');
}

export function builtinMarkdown(doc: BuiltinDoc, kind = 'builtin'): string {
    const out: string[] = [omg(doc.signatures.join('\n')), '*' + kind + ' · ' + doc.category + '*', '', doc.summary];
    if (doc.details) out.push('', doc.details);
    if (doc.params.length > 0) {
        out.push('', '**Parameters**', '');
        for (const p of doc.params) out.push('- `' + p.name + '`: ' + p.doc);
    }
    if (doc.returns) out.push('', '**Returns** ' + lowerFirst(doc.returns));
    if (doc.raises && doc.raises.length > 0) {
        out.push('', '**Raises**', '');
        for (const r of doc.raises) out.push('- ' + r);
    }
    if (doc.platform) out.push('', '**Platform** ' + lowerFirst(doc.platform));
    if (doc.example) out.push('', '**Example**', '', omg(doc.example));
    if (doc.see && doc.see.length > 0) out.push('', '*See also:* ' + doc.see.map((s) => '`' + s + '`').join(', '));
    return out.join('\n');
}

function lowerFirst(s: string): string {
    if (s.length > 1 && /[A-Z]/.test(s[0]) && /[a-z ]/.test(s[1])) return s[0].toLowerCase() + s.slice(1);
    return s;
}

export function keywordMarkdown(doc: KeywordDoc): string {
    const out: string[] = [omg(doc.syntax), '*keyword*', '', doc.summary];
    if (doc.details) out.push('', doc.details);
    if (doc.example) out.push('', '**Example**', '', omg(doc.example));
    return out.join('\n');
}

export function operatorMarkdown(doc: OperatorDoc): string {
    const out: string[] = ['**`' + doc.symbol + '`** ' + doc.name, '', doc.summary];
    if (doc.example) out.push('', omg(doc.example));
    const row = PRECEDENCE.findIndex((g) => g.split(' ').includes(doc.symbol));
    if (row >= 0) {
        const cells = PRECEDENCE.map((g, i) => (i === row ? '**`' + g + '`**' : '`' + g + '`'));
        out.push('', '*Binding, loosest first:* ' + cells.join(' · '));
    }
    return out.join('\n');
}

export function globalMarkdown(doc: GlobalDoc): string {
    const out: string[] = [omg(doc.name), '*built-in global · ' + doc.type + '*', '', doc.summary, '', doc.details];
    if (doc.example) out.push('', '**Example**', '', omg(doc.example));
    return out.join('\n');
}

export function rawOpMarkdown(doc: RawOpDoc): string {
    return [
        omg(doc.signature),
        '*raw mode · the x86-64 runtime only*',
        '',
        doc.summary,
        '',
        'Raw mode is how `src/runtime/x64/` is compiled: every value is one untyped 64-bit word. See `docs/06-runtime.md`.'
    ].join('\n');
}

export function intLiteralMarkdown(text: string, value: bigint): string {
    const neg = value < 0n;
    const abs = neg ? -value : value;
    const rows = [
        '**`' + text + '`** int',
        '',
        '| | |',
        '|---|---|',
        '| decimal | `' + value.toString() + '` |',
        '| hex | `' + (neg ? '-' : '') + abs.toString(16) + '` |',
        '| binary | `' + (neg ? '-' : '') + groupBits(abs.toString(2)) + '` |'
    ];
    if (value >= 32n && value <= 126n) rows.push('| character | `' + String.fromCharCode(Number(value)) + '` |');
    return rows.join('\n');
}

function groupBits(bits: string): string {
    if (bits.length <= 8) return bits;
    const pad = (8 - (bits.length % 8)) % 8;
    const padded = '0'.repeat(pad) + bits;
    const groups: string[] = [];
    for (let i = 0; i < padded.length; i += 8) groups.push(padded.slice(i, i + 8));
    return groups.join(' ');
}

// ---- Things the user wrote ---------------------------------------------------

export function signatureOf(sym: Sym): string {
    return 'proc ' + sym.name + '(' + (sym.params ?? []).map((p) => p.name).join(', ') + ')';
}

function fileLabel(file: FileAnalysis): string {
    const p = uriToPath(file.uri);
    return p ? path.basename(p) : file.uri;
}

/** One line of source, shortened, for showing an initialiser. */
function oneLine(text: string, max = 72): string {
    const firstBreak = text.search(/\r?\n/);
    let s = firstBreak >= 0 ? text.slice(0, firstBreak).trimEnd() + ' …' : text;
    if (s.length > max) s = s.slice(0, max - 1).trimEnd() + '…';
    return s;
}

function valueNote(infer: Inference, v: Value | undefined): string | undefined {
    if (!v) return undefined;
    switch (v.k) {
        case 'proc':
            return 'Holds the ' + (v.sym.global ? 'proc' : 'closure') + ' `' + signatureOf(v.sym).slice(5) + '`.';
        case 'module':
            return undefined;
        case 'dict': {
            const keys = [...v.fields.keys()];
            if (keys.length === 0) return undefined;
            const shown = keys.slice(0, 16).map((k) => {
                const fld = v.fields.get(k)!;
                const fv = fld.value ? infer.valueOf(fld.file, fld.value, 0) : undefined;
                return '`' + k + (fv && fv.k === 'proc' ? '(' + (fv.sym.params ?? []).map((p) => p.name).join(', ') + ')' : '') + '`';
            });
            return 'Keys: ' + shown.join(', ') + (keys.length > 16 ? ', …' : '');
        }
        case 'type':
            return undefined;
    }
}

function paramDoc(procDoc: string | undefined, name: string): string | undefined {
    if (!procDoc) return undefined;
    for (const line of procDoc.split('\n')) {
        const m = /^- `([A-Za-z_][A-Za-z0-9_]*)`: (.*)$/.exec(line);
        if (m && m[1] === name) return m[2];
    }
    return undefined;
}

export function paramDocOf(sym: Sym, name: string): string | undefined {
    return paramDoc(sym.doc, name);
}

export function symbolMarkdown(infer: Inference, sym: Sym, from: FileAnalysis): string {
    const file = sym.file;
    const elsewhere = file !== from ? ' · ' + fileLabel(file) : '';
    const out: string[] = [];
    switch (sym.kind) {
        case 'proc': {
            out.push(omg(signatureOf(sym)));
            if (sym.global) {
                out.push('*proc' + elsewhere + '*');
            } else {
                out.push('*nested proc in `' + (sym.owner?.name ?? '?') + '`, a closure' + elsewhere + '*');
            }
            if (sym.doc) out.push('', prose(sym.doc));
            const ret = infer.returnOf(sym, 0);
            if (ret) {
                if (ret.k === 'dict') {
                    const note = valueNote(infer, ret);
                    out.push('', '**Returns** a ' + (ret.frozen ? 'frozen dict' : 'dict') + (note ? '. ' + note : ''));
                } else if (ret.k === 'proc') {
                    out.push('', '**Returns** the ' + (ret.sym.global ? 'proc' : 'closure') + ' `' + signatureOf(ret.sym).slice(5) + '`');
                } else if (ret.k === 'type') {
                    out.push('', '**Returns** ' + ret.type);
                }
            }
            break;
        }
        case 'var': {
            const init = sym.init ? file.text.slice(sym.init.start, sym.init.end) : '…';
            out.push(omg('alloc ' + sym.name + ' := ' + oneLine(init)));
            const v = infer.valueOfSym(sym, 0);
            const t = describeValue(v);
            const where = sym.global ? 'global variable' : 'local variable of `' + (sym.owner?.name ?? '?') + '`';
            out.push('*' + where + (t && t !== 'proc' ? ' · ' + t : '') + elsewhere + '*');
            if (sym.doc) out.push('', prose(sym.doc));
            const note = valueNote(infer, v);
            if (note) out.push('', note);
            if (v && v.k === 'proc' && v.sym.doc) out.push('', prose(v.sym.doc));
            break;
        }
        case 'param': {
            out.push(omg(sym.name));
            const owner = sym.owner;
            out.push('*parameter of `' + (owner ? signatureOf(owner).slice(5) : '?') + '`*');
            const d = owner ? paramDoc(owner.doc, sym.name) : undefined;
            if (d) out.push('', d);
            break;
        }
        case 'import': {
            out.push(omg('import "' + (sym.importPath ?? '') + '" as ' + sym.name));
            const mod = infer.valueOfSym(sym, 0);
            if (mod && mod.k === 'module') {
                out.push('*module · ' + fileLabel(mod.file) + '*');
                const procs: string[] = [];
                const vars: string[] = [];
                for (const s of mod.file.exports.values()) {
                    if (s.kind === 'proc') procs.push('`' + signatureOf(s).slice(5) + '`');
                    else vars.push('`' + s.name + '`');
                }
                if (procs.length > 0) out.push('', '**Procs** ' + procs.slice(0, 40).join(', ') + (procs.length > 40 ? ', …' : ''));
                if (vars.length > 0) out.push('', '**Variables** ' + vars.slice(0, 40).join(', ') + (vars.length > 40 ? ', …' : ''));
                if (vars.length > 0) out.push('', 'Variables are copied when the import runs; the alias is read-only.');
            } else {
                out.push('*module · file not found*');
            }
            break;
        }
        case 'loopvar': {
            const iter = sym.iter ? oneLine(file.text.slice(sym.iter.start, sym.iter.end), 48) : '…';
            out.push(omg('for ' + sym.name + ' in ' + iter));
            const t = describeValue(infer.valueOfSym(sym, 0));
            out.push('*loop variable' + (t ? ' · ' + t : '') + '*', '', 'Declared by the `for` loop; like any variable it stays visible after the loop ends.');
            break;
        }
        case 'except':
            out.push(omg('except ' + sym.name));
            out.push('*caught error · string*', '', 'The error the `try` block raised, as text: `"Kind: message"`, for example `"IndexError: index 5 out of range for length 3"`.');
            break;
    }
    return out.join('\n');
}

export function resolvedMarkdown(infer: Inference, r: Resolved, from: FileAnalysis): string | undefined {
    switch (r.k) {
        case 'sym':
            return symbolMarkdown(infer, r.sym, from);
        case 'builtin':
            return builtinMarkdown(r.doc);
        case 'raiser':
            return r.doc ? builtinMarkdown(r.doc) : omg(r.name + '(message)') + '\n*internal raise helper of the toolchain*';
        case 'reserved':
            return globalMarkdown(r.doc);
        case 'raw':
            return r.doc ? rawOpMarkdown(r.doc) : undefined;
        case 'field': {
            const fld = r.field;
            const v = fld.value ? infer.valueOf(fld.file, fld.value, 0) : undefined;
            if (v && v.k === 'proc') {
                const out = [omg(fld.name + ': ' + signatureOf(v.sym)), '*dict entry holding a ' + (v.sym.global ? 'proc' : 'closure') + '*'];
                if (v.sym.doc) out.push('', prose(v.sym.doc));
                return out.join('\n');
            }
            const src = fld.value ? oneLine(fld.file.text.slice(fld.value.start, fld.value.end), 60) : '…';
            const t = describeValue(v);
            const out = [omg(fld.name + ': ' + src), '*dict entry' + (t ? ' · ' + t : '') + (r.frozen ? ' · read-only' : '') + '*'];
            const note = valueNote(infer, v);
            if (note) out.push('', note);
            return out.join('\n');
        }
        case 'missing-member':
            return undefined;
    }
}
