// Draws the pictures in the README: sample/wordcount.omg as the
// extension colours it, in both themes.
//
// Nothing here is drawn by hand. The colours come from the grammar (run
// by vscode-textmate, the engine VS Code uses), the language server's
// semantic tokens and the theme files; the hover text and the problems
// are what the language server answers for that file. Only the frame
// around them is this script's.
//
//   node scripts/preview.mjs        writes images/preview-dark.png, preview-light.png

import { build } from 'esbuild';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const oniguruma = require('vscode-oniguruma');
const textmate = require('vscode-textmate');

await build({
    stdin: {
        contents: `
            export { registerBuiltinNames } from './server/src/analysis';
            export { BUILTINS } from './server/src/builtins';
            export { DEFAULT_DIAGNOSTICS, diagnose } from './server/src/diagnostics';
            export { hover } from './server/src/features';
            export { positionAt } from './server/src/lexer';
            export { TOKEN_MODIFIERS, TOKEN_TYPES, semanticTokens } from './server/src/semantic';
            export { Workspace, pathToUri } from './server/src/workspace';`,
        resolveDir: root,
        loader: 'ts'
    },
    bundle: true, platform: 'node', format: 'cjs', outfile: join(root, 'out', 'preview-lib.cjs'), logLevel: 'error'
});
const server = require(join(root, 'out', 'preview-lib.cjs'));
server.registerBuiltinNames(server.BUILTINS.map((b) => b.name));

const wasm = readFileSync(require.resolve('vscode-oniguruma/release/onig.wasm'));
await oniguruma.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength));

async function grammarFor(theme) {
    const registry = new textmate.Registry({
        onigLib: Promise.resolve({
            createOnigScanner: (patterns) => new oniguruma.OnigScanner(patterns),
            createOnigString: (s) => new oniguruma.OnigString(s)
        }),
        loadGrammar: async (scope) => {
            if (scope !== 'source.omg') return null;
            const p = join(root, 'syntaxes', 'omg.tmLanguage.json');
            return textmate.parseRawGrammar(readFileSync(p, 'utf8'), p);
        }
    });
    registry.setTheme({ name: theme.name, settings: [{ settings: { foreground: theme.colors['editor.foreground'] } }, ...theme.tokenColors] });
    return { grammar: await registry.loadGrammar('source.omg'), colorMap: registry.getColorMap() };
}

const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

/** Per character of each line: colour and font style, as the grammar and theme give them. */
function paint(source, grammar, colorMap) {
    const lines = source.split('\n');
    const cells = [];
    const brackets = [];
    let stack = textmate.INITIAL;
    lines.forEach((line, l) => {
        const named = grammar.tokenizeLine(line, stack);
        const binary = grammar.tokenizeLine2(line, stack);
        stack = binary.ruleStack;
        const row = Array.from({ length: line.length }, () => ({ color: colorMap[1], style: 0 }));
        for (let i = 0; i < binary.tokens.length; i += 2) {
            const start = binary.tokens[i];
            const end = i + 2 < binary.tokens.length ? binary.tokens[i + 2] : line.length;
            const meta = binary.tokens[i + 1];
            const color = colorMap[(meta & 0xff8000) >>> 15];
            const style = (meta & 0x7800) >>> 11;
            for (let c = start; c < end && c < line.length; c++) row[c] = { color, style };
        }
        for (const t of named.tokens) {
            if (t.scopes.some((s) => /^punctuation\.(section\.(braces|brackets|parens)|definition\.parameters)\./.test(s))) {
                for (let c = t.startIndex; c < t.endIndex; c++) brackets.push({ l, c, open: '{[('.includes(line[c]) });
            }
        }
        cells.push(row);
    });
    return { lines, cells, brackets };
}

/** What the language server adds: the colour of each name by what it is. */
function paintSemantic(painted, file, theme) {
    const rules = Object.entries(theme.semanticTokenColors).map(([selector, value]) => {
        const [type, ...mods] = selector.split('.');
        return { type, mods, color: typeof value === 'string' ? value : value.foreground, italic: typeof value === 'object' && value.italic };
    });
    const data = server.semanticTokens(file);
    let line = 0;
    let ch = 0;
    for (let i = 0; i < data.length; i += 5) {
        line += data[i];
        ch = data[i] === 0 ? ch + data[i + 1] : data[i + 1];
        const type = server.TOKEN_TYPES[data[i + 3]];
        const mods = server.TOKEN_MODIFIERS.filter((_, b) => data[i + 4] & (1 << b));
        const fits = rules.filter((r) => r.type === type && r.mods.every((m) => mods.includes(m)));
        fits.sort((a, b) => b.mods.length - a.mods.length);
        if (fits.length === 0) continue;
        for (let c = ch; c < ch + data[i + 2]; c++) painted.cells[line][c] = { color: fits[0].color, style: fits[0].italic ? 1 : 0 };
    }
}

/** Bracket pairs by depth, as the editor colours them. */
function paintBrackets(painted, theme) {
    const palette = [1, 2, 3, 4, 5, 6].map((n) => theme.colors['editorBracketHighlight.foreground' + n]);
    let depth = 0;
    for (const b of painted.brackets) {
        if (!b.open) depth = Math.max(0, depth - 1);
        painted.cells[b.l][b.c] = { color: palette[depth % palette.length], style: 0 };
        if (b.open) depth++;
    }
}

function lineHtml(text, row, squiggles = []) {
    let html = '';
    let c = 0;
    while (c < text.length) {
        let e = c + 1;
        const under = squiggles.find((s) => c >= s.from && c < s.to);
        while (e < text.length && row[e].color === row[c].color && row[e].style === row[c].style
            && squiggles.find((s) => e >= s.from && e < s.to) === under) e++;
        const st = row[c].style;
        let css = 'color:' + row[c].color;
        if (st & 1) css += ';font-style:italic';
        if (st & 2) css += ';font-weight:600';
        if (st & 4) css += ';text-decoration:underline';
        if (under) css += ';text-decoration:underline wavy ' + under.color + ';text-decoration-skip-ink:none;text-underline-offset:4px';
        html += '<span style="' + css + '">' + esc(text.slice(c, e)) + '</span>';
        c = e;
    }
    return html || '&nbsp;';
}

function codeHtml(source, grammar, colorMap, theme) {
    const p = paint(source, grammar, colorMap);
    paintBrackets(p, theme);
    return p.lines.map((l, i) => lineHtml(l, p.cells[i])).join('\n');
}

/** The Markdown the server writes for a hover: paragraphs, lists, code blocks, bold, italic, code. */
function markdownHtml(md, grammar, colorMap, theme) {
    const inline = (s) => esc(s)
        .replace(/`([^`]+)`/g, '<code>$1</code>')
        .replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>')
        .replace(/(^|[^*])\*([^*]+)\*/g, '$1<i>$2</i>');
    const out = [];
    const lines = md.split('\n');
    for (let i = 0; i < lines.length; i++) {
        if (lines[i].startsWith('```')) {
            const body = [];
            for (i++; i < lines.length && !lines[i].startsWith('```'); i++) body.push(lines[i]);
            out.push('<pre>' + codeHtml(body.join('\n'), grammar, colorMap, theme) + '</pre>');
        } else if (lines[i].startsWith('- ')) {
            const items = [];
            for (; i < lines.length && lines[i].startsWith('- '); i++) items.push('<li>' + inline(lines[i].slice(2)) + '</li>');
            i--;
            out.push('<ul>' + items.join('') + '</ul>');
        } else if (lines[i].trim() !== '') {
            out.push('<p>' + inline(lines[i]) + '</p>');
        }
    }
    return out.join('');
}

function page(theme, opts) {
    const c = theme.colors;
    const { painted, hover, problems } = opts;
    const rows = painted.lines.map((text, i) => {
        const squiggles = (problems ?? []).filter((d) => d.range.start.line === i).map((d) => ({
            from: d.range.start.character,
            to: d.range.end.character,
            color: d.severity === 1 ? c['editorError.foreground'] : c['editorWarning.foreground']
        }));
        const active = hover && hover.line === i;
        return '<div class="row' + (active ? ' active' : '') + '"><span class="no">' + (i + 1) + '</span><span class="code">'
            + lineHtml(text, painted.cells[i], squiggles) + '</span></div>';
    }).join('');
    const icon = readFileSync(join(root, 'icons', 'omg.svg'), 'utf8').replace('<svg ', '<svg width="16" height="16" ');
    const hoverCard = hover
        ? '<div class="hover" style="top:' + (hover.line + 1) * 22 + 'px;left:' + Math.max(396, 64 + hover.character * 8.4) + 'px">' + hover.html + '</div>'
        : '';
    const panel = problems
        ? '<div class="panel"><div class="ptitle">PROBLEMS <span class="badge">' + problems.length + '</span></div>'
            + problems.map((d) => '<div class="prob"><span class="dot" style="background:' + (d.severity === 1 ? c['editorError.foreground'] : c['editorWarning.foreground']) + '"></span>'
                + '<span>' + esc(d.message.split('\n')[0]) + '</span><span class="src">omg(' + d.code + ')  [Ln ' + (d.range.start.line + 1) + ', Col ' + (d.range.start.character + 1) + ']</span></div>').join('')
            + '</div>'
        : '';
    return `<!doctype html><html><head><meta charset="utf-8"><style>
        * { box-sizing: border-box; }
        body { margin: 0; padding: 28px; background: ${c['titleBar.activeBackground']}; font-family: Inter, sans-serif; }
        .win { width: 980px; border-radius: 12px; overflow: hidden; background: ${c['editor.background']}; box-shadow: 0 12px 40px ${c['widget.shadow']}; border: 1px solid ${c['editorGroup.border']}; }
        .tabs { display: flex; background: ${c['editorGroupHeader.tabsBackground']}; font-size: 13px; }
        .tab { display: flex; align-items: center; gap: 7px; padding: 9px 16px; color: ${c['tab.inactiveForeground']}; }
        .tab.on { background: ${c['tab.activeBackground']}; color: ${c['tab.activeForeground']}; box-shadow: inset 0 2px 0 ${c['tab.activeBorderTop']}; }
        .editor { position: relative; padding: 10px 0 14px; font: 14px/22px 'JetBrains Mono', monospace; font-variant-ligatures: none; }
        .row { display: flex; white-space: pre; height: 22px; }
        .row.active { background: ${c['editor.lineHighlightBackground']}; }
        .no { width: 48px; padding-right: 16px; text-align: right; color: ${c['editorLineNumber.foreground']}; flex: none; }
        .row.active .no { color: ${c['editorLineNumber.activeForeground']}; }
        .code { padding-left: 16px; }
        .hover { position: absolute; margin-top: 12px; width: 560px; background: ${c['editorHoverWidget.background']}; border: 1px solid ${c['editorHoverWidget.border']}; border-radius: 6px; box-shadow: 0 6px 24px ${c['widget.shadow']}; color: ${c['editorHoverWidget.foreground']}; font: 13px/1.5 Inter, sans-serif; padding: 4px 12px 8px; }
        .hover p { margin: 7px 0; }
        .hover ul { margin: 6px 0; padding-left: 20px; }
        .hover pre { margin: 8px -12px; padding: 6px 12px; font: 13px/20px 'JetBrains Mono', monospace; font-variant-ligatures: none; border-bottom: 1px solid ${c['editorHoverWidget.border']}; }
        .hover pre + p { margin-top: 8px; }
        .hover p + pre { border-top: 1px solid ${c['editorHoverWidget.border']}; border-bottom: 0; margin-bottom: 0; padding-bottom: 2px; }
        .hover code { font: 12.5px 'JetBrains Mono', monospace; color: ${c['textPreformat.foreground']}; background: ${c['textPreformat.background']}; padding: 1px 4px; border-radius: 3px; }
        .hover i { color: ${c['descriptionForeground']}; }
        .panel { border-top: 1px solid ${c['panel.border']}; background: ${c['panel.background']}; padding: 8px 16px 12px; font-size: 13px; color: ${c['foreground']}; }
        .ptitle { font-size: 11px; letter-spacing: .04em; color: ${c['panelTitle.activeForeground']}; padding-bottom: 6px; display: inline-block; border-bottom: 1px solid ${c['panelTitle.activeBorder']}; margin-bottom: 8px; }
        .badge { background: ${c['badge.background']}; color: ${c['badge.foreground']}; border-radius: 9px; padding: 1px 6px; margin-left: 4px; }
        .prob { display: flex; align-items: center; gap: 8px; padding: 3px 0; }
        .dot { width: 9px; height: 9px; border-radius: 50%; flex: none; }
        .src { color: ${c['descriptionForeground']}; margin-left: 4px; white-space: pre; }
    </style></head><body><div class="win">
        <div class="tabs"><div class="tab on">${icon}<span>wordcount.omg</span></div><div class="tab">${icon}<span>text.omg</span></div></div>
        <div class="editor">${rows}${hoverCard}</div>${panel}
    </div></body></html>`;
}

const samplePath = join(root, 'sample', 'wordcount.omg');
const source = readFileSync(samplePath, 'utf8').replace(/\n$/, '');

function analyse(text) {
    const ws = new server.Workspace();
    ws.setRoots([join(root, 'sample')]);
    const uri = server.pathToUri(samplePath);
    ws.open(uri, text);
    return { ws, file: ws.get(uri) };
}

const { chromium } = require('playwright');
const browser = await chromium.launch();

async function shoot(themeFile, out, make) {
    const theme = JSON.parse(readFileSync(join(root, 'themes', themeFile), 'utf8'));
    const { grammar, colorMap } = await grammarFor(theme);
    const html = page(theme, make(theme, grammar, colorMap));
    const tab = await browser.newPage({ viewport: { width: 1036, height: 600 }, deviceScaleFactor: 2 });
    await tab.setContent(html);
    await tab.screenshot({ path: join(root, 'images', out), fullPage: true });
    await tab.close();
    console.log('wrote images/' + out);
}

// Dark: the file as it is, with the hover for has_key open.
await shoot('omg-dark-color-theme.json', 'preview-dark.png', (theme, grammar, colorMap) => {
    const { ws, file } = analyse(source);
    const painted = paint(source, grammar, colorMap);
    paintSemantic(painted, file, theme);
    paintBrackets(painted, theme);
    const at = source.indexOf('has_key');
    const pos = server.positionAt(file.lex.lineStarts, at + 2);
    const h = server.hover(ws, file, pos);
    return { painted, hover: { line: pos.line, character: pos.character - 2, html: markdownHtml(h.contents.value, grammar, colorMap, theme) } };
});

// Light: two slips typed into the same file, and what the server says about them.
await shoot('omg-light-color-theme.json', 'preview-light.png', (theme, grammar, colorMap) => {
    const slipped = source
        .replace('tally(text.lower(word))', 'tally(text.lower(word), 1)')
        .replace('emit word + "\\t" + counts[word]', 'emit word + "\\t" + count[word]');
    const { ws, file } = analyse(slipped);
    const painted = paint(slipped, grammar, colorMap);
    paintSemantic(painted, file, theme);
    paintBrackets(painted, theme);
    const problems = server.diagnose(ws, file, server.DEFAULT_DIAGNOSTICS).filter((d) => d.severity <= 2);
    return { painted, problems };
});

await browser.close();
