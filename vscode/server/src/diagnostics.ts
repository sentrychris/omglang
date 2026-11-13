// Turning what the analysis found into what the editor shows, and the
// quick fixes that go with it.

import {
    CodeAction, CodeActionKind, Diagnostic, DiagnosticSeverity, DiagnosticTag, Range, TextEdit
} from 'vscode-languageserver/node';

import { Diag, FileAnalysis } from './analysis';
import { toRange } from './features';
import { SyntaxIssue } from './lexer';
import { Workspace } from './workspace';

export interface DiagnosticSettings {
    enabled: boolean;
    /** Greyed-out hints for unused names and unreachable code. */
    unused: boolean;
    /** Warnings for names not defined in the file. */
    undefinedNames: boolean;
}

export const DEFAULT_DIAGNOSTICS: DiagnosticSettings = { enabled: true, unused: true, undefinedNames: true };

const SOURCE = 'omg';

interface FixData {
    title: string;
    start: number;
    end: number;
    text: string;
}

function syntaxDiagnostic(file: FileAnalysis, e: SyntaxIssue): Diagnostic {
    let message = e.message;
    if (message.endsWith('starting')) message += ' here';
    if (e.severity !== 'warning') message = 'SyntaxError: ' + message;
    if (e.hint) message += '\n' + e.hint;
    let end = e.end;
    if (end <= e.start) end = Math.min(file.text.length, e.start + 1);
    return {
        range: toRange(file, e.start, end),
        severity: e.severity === 'warning' ? DiagnosticSeverity.Warning : DiagnosticSeverity.Error,
        source: SOURCE,
        code: e.severity === 'warning' ? 'glued-number' : 'syntax',
        message
    };
}

function fixFor(file: FileAnalysis, d: Diag): FixData | undefined {
    const text = file.text;
    switch (d.code) {
        case 'undeclared-assignment':
            if (d.message.includes('has not been declared')) {
                return { title: 'Declare with alloc', start: d.start, end: d.start, text: 'alloc ' };
            }
            return undefined;
        case 'duplicate-alloc': {
            // `alloc name`: the keyword sits just before the name.
            const before = text.slice(Math.max(0, d.start - 32), d.start);
            const m = /alloc[ \t]+$/.exec(before);
            if (!m) return undefined;
            return { title: 'Assign instead of declaring again', start: d.start - m[0].length, end: d.start, text: '' };
        }
        case 'undefined-name': {
            const m = /Did you mean '([A-Za-z_][A-Za-z0-9_]*)'\?/.exec(d.message);
            if (!m) return undefined;
            return { title: "Change to '" + m[1] + "'", start: d.start, end: d.end, text: m[1] };
        }
        case 'unused': {
            if (!d.message.startsWith('The import')) return undefined;
            // Take the whole line with it.
            let s = d.start;
            while (s > 0 && (text[s - 1] === ' ' || text[s - 1] === '\t')) s--;
            let e = d.end;
            while (e < text.length && text[e] !== '\n') e++;
            if (e < text.length) e++;
            if (s > 0 && text[s - 1] !== '\n') return undefined;
            return { title: 'Remove unused import', start: s, end: e, text: '' };
        }
        default:
            return undefined;
    }
}

function toDiagnostic(file: FileAnalysis, d: Diag): Diagnostic {
    const out: Diagnostic = {
        range: toRange(file, d.start, Math.max(d.end, d.start + 1)),
        severity: d.severity === 'error' ? DiagnosticSeverity.Error
            : d.severity === 'warning' ? DiagnosticSeverity.Warning
            : DiagnosticSeverity.Hint,
        source: SOURCE,
        code: d.code,
        message: d.message
    };
    if (d.unnecessary) out.tags = [DiagnosticTag.Unnecessary];
    if (d.related && d.related.length > 0) {
        out.relatedInformation = d.related.map((r) => ({
            location: { uri: file.uri, range: toRange(file, r.span.start, r.span.end) },
            message: r.message
        }));
    }
    const fix = fixFor(file, d);
    if (fix) out.data = fix;
    return out;
}

export function diagnose(ws: Workspace, file: FileAnalysis, settings: DiagnosticSettings): Diagnostic[] {
    if (!settings.enabled) return [];
    const out: Diagnostic[] = [];
    for (const w of file.lex.warnings) out.push(syntaxDiagnostic(file, w));
    if (file.hasSyntaxError) {
        // What the compiler would say, and nothing built on top of a
        // file that does not parse.
        for (const e of file.syntaxErrors) out.push(syntaxDiagnostic(file, e));
        return out;
    }
    const all = file.diagnostics.concat(ws.crossFileDiagnostics(file));
    for (const d of all) {
        if (!settings.unused && (d.code === 'unused' || d.code === 'unreachable')) continue;
        if (!settings.undefinedNames && d.code === 'undefined-name') continue;
        out.push(toDiagnostic(file, d));
    }
    return out;
}

export function codeActions(file: FileAnalysis, uri: string, range: Range, diagnostics: Diagnostic[]): CodeAction[] {
    const out: CodeAction[] = [];
    for (const d of diagnostics) {
        if (d.source !== SOURCE) continue;
        const fix = d.data as FixData | undefined;
        if (!fix || typeof fix.title !== 'string') continue;
        if (d.range.end.line < range.start.line || d.range.start.line > range.end.line) continue;
        out.push({
            title: fix.title,
            kind: CodeActionKind.QuickFix,
            diagnostics: [d],
            isPreferred: true,
            edit: { changes: { [uri]: [TextEdit.replace(toRange(file, fix.start, fix.end), fix.text)] } }
        });
    }
    return out;
}
