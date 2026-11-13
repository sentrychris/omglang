// Print how the TextMate grammar scopes a file: `node scripts/tokens.mjs file.omg`.
// For checking a grammar change without opening the editor.

import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const oniguruma = require('vscode-oniguruma');
const textmate = require('vscode-textmate');

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

export async function loadGrammar() {
    const wasm = readFileSync(require.resolve('vscode-oniguruma/release/onig.wasm'));
    await oniguruma.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength));
    const registry = new textmate.Registry({
        onigLib: Promise.resolve({
            createOnigScanner: (patterns) => new oniguruma.OnigScanner(patterns),
            createOnigString: (s) => new oniguruma.OnigString(s)
        }),
        loadGrammar: async (scope) => {
            if (scope !== 'source.omg') return null;
            const path = join(root, 'syntaxes', 'omg.tmLanguage.json');
            return textmate.parseRawGrammar(readFileSync(path, 'utf8'), path);
        }
    });
    return { grammar: await registry.loadGrammar('source.omg'), INITIAL: textmate.INITIAL };
}

/** Every token of a text: `{ line, text, scopes }`, whitespace left out. */
export function tokenize(grammar, INITIAL, source) {
    const out = [];
    let stack = INITIAL;
    source.split('\n').forEach((line, i) => {
        const r = grammar.tokenizeLine(line, stack);
        for (const t of r.tokens) {
            const text = line.slice(t.startIndex, t.endIndex);
            if (text.trim() === '') continue;
            out.push({ line: i + 1, text, scopes: t.scopes.slice(1) });
        }
        stack = r.ruleStack;
    });
    return out;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
    const file = process.argv[2];
    if (!file) {
        console.error('usage: node scripts/tokens.mjs <file.omg>');
        process.exit(2);
    }
    const { grammar, INITIAL } = await loadGrammar();
    for (const t of tokenize(grammar, INITIAL, readFileSync(file, 'utf8'))) {
        console.log(String(t.line).padStart(4) + '  ' + JSON.stringify(t.text).padEnd(28) + ' ' + t.scopes.join('  '));
    }
}
