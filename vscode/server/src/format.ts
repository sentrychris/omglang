// Formatting: indentation and trailing whitespace, nothing else. The
// same job as tools/dev/omg-fmt.omg. Lines that begin inside a string or
// a block comment are left exactly as they are, and so is the indent of
// lines aligned by hand under an open bracket.

import { FileAnalysis } from './analysis';
import { positionAt } from './lexer';

export interface LineEdit {
    line: number;
    /** Columns to replace on that line. */
    from: number;
    to: number;
    text: string;
}

export function formatLines(
    file: FileAnalysis, tabSize: number, insertSpaces: boolean, firstLine = 0, lastLine = Number.MAX_SAFE_INTEGER
): LineEdit[] {
    // A string or comment that never closes swallows the rest of the
    // file; re-indenting that would be guesswork.
    if (file.lex.errors.length > 0) return [];

    const text = file.text;
    const lineStarts = file.lex.lineStarts;
    const lineCount = lineStarts.length;
    const unit = insertSpaces ? ' '.repeat(Math.max(1, tabSize)) : '\t';

    // Lines that start inside a string or a block comment.
    const verbatim = new Uint8Array(lineCount);
    // Lines whose end is inside a string: their trailing spaces are data.
    const endsInString = new Uint8Array(lineCount);
    const lineOf = (offset: number): number => positionAt(lineStarts, offset).line;
    for (const t of file.lex.tokens) {
        if (t.kind !== 'STR') continue;
        const a = lineOf(t.start);
        const z = lineOf(Math.max(t.start, t.end - 1));
        for (let l = a + 1; l <= z; l++) verbatim[l] = 1;
        for (let l = a; l < z; l++) endsInString[l] = 1;
    }
    for (const c of file.lex.comments) {
        if (!c.block) continue;
        for (let l = c.startLine + 1; l <= c.endLine; l++) verbatim[l] = 1;
    }

    // The indent of each line: one level deeper than the line that opened
    // the innermost bracket still open, however many brackets that line
    // opened. A line that starts by closing brackets lines up with the
    // line that opened them.
    //
    // A bracket with something after it on its line starts an aligned
    // run instead (`f(a,` with `b)` under the `a`): those lines were
    // laid out by hand and keep the indent they have.
    const indent = new Int32Array(lineCount).fill(-1);
    const aligned = new Uint8Array(lineCount);
    const stack: number[] = [];
    const hanging: boolean[] = [];
    const toks = file.lex.tokens;
    let ti = 0;
    for (let l = 0; l < lineCount; l++) {
        const lineEnd = l + 1 < lineCount ? lineStarts[l + 1] : text.length + 1;
        let level = stack.length > 0 ? stack[stack.length - 1] + 1 : 0;
        if (hanging.length > 0 && hanging[hanging.length - 1]) aligned[l] = 1;
        let leading = true;
        let j = ti;
        // Leading closers decide this line's own indent.
        while (j < toks.length && toks[j].start < lineEnd && toks[j].kind !== 'EOF') {
            const k = toks[j].kind;
            if (leading && (k === 'RBRACE' || k === 'RBRACK' || k === 'RPAREN')) {
                if (stack.length > 0) {
                    level = stack.pop()!;
                    hanging.pop();
                }
                j++;
                continue;
            }
            break;
        }
        indent[l] = level;
        leading = false;
        for (; j < toks.length && toks[j].start < lineEnd && toks[j].kind !== 'EOF'; j++) {
            const k = toks[j].kind;
            if (k === 'LBRACE' || k === 'LBRACK' || k === 'LPAREN') {
                const next = toks[j + 1];
                stack.push(level);
                hanging.push(next !== undefined && next.kind !== 'NL' && next.kind !== 'EOF' && next.start < lineEnd);
            } else if (k === 'RBRACE' || k === 'RBRACK' || k === 'RPAREN') {
                stack.pop();
                hanging.pop();
            }
        }
        ti = j;
    }

    // A comment that carries on under the comment at the end of the
    // line above stays under it.
    const commentCol = new Int32Array(lineCount).fill(-1);
    for (const c of file.lex.comments) {
        if (!c.block) commentCol[c.startLine] = c.start - lineStarts[c.startLine];
    }

    const edits: LineEdit[] = [];
    const header = file.lex.header;
    const headerLine = header ? lineOf(header.start) : -1;
    for (let l = Math.max(0, firstLine); l < lineCount && l <= lastLine; l++) {
        if (verbatim[l]) continue;
        const start = lineStarts[l];
        let end = l + 1 < lineCount ? lineStarts[l + 1] - 1 : text.length;
        if (end > start && text.charCodeAt(end - 1) === 13) end--;
        const line = text.slice(start, end);
        const body = line.replace(/^[ \t]+/, '');
        const lead = line.length - body.length;
        if (body === '') {
            if (line.length > 0) edits.push({ line: l, from: 0, to: line.length, text: '' });
            continue;
        }
        const want = l === headerLine ? '' : unit.repeat(indent[l]);
        const continues = body.startsWith('#') && l > 0 && commentCol[l - 1] === lead && lead > want.length;
        if (!aligned[l] && !continues && line.slice(0, lead) !== want) edits.push({ line: l, from: 0, to: lead, text: want });
        if (!endsInString[l]) {
            const trimmed = body.replace(/[ \t]+$/, '');
            if (trimmed.length !== body.length) {
                edits.push({ line: l, from: lead + trimmed.length, to: line.length, text: '' });
            }
        }
    }
    return edits;
}
