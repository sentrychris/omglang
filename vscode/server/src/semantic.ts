// Semantic colouring: what each name *is*, which a grammar of regular
// expressions cannot know. A parameter, a local, a global, a module, a
// builtin and a dict key all look like plain words to it.

import { FileAnalysis, Occurrence } from './analysis';
import { positionAt } from './lexer';

export const TOKEN_TYPES = ['namespace', 'function', 'method', 'variable', 'parameter', 'property'] as const;
export const TOKEN_MODIFIERS = ['declaration', 'defaultLibrary', 'readonly', 'global'] as const;

type TokenType = typeof TOKEN_TYPES[number];
type TokenModifier = typeof TOKEN_MODIFIERS[number];

const TYPE_INDEX = new Map<string, number>(TOKEN_TYPES.map((t, i) => [t, i]));
const MOD_BIT = new Map<string, number>(TOKEN_MODIFIERS.map((m, i) => [m, 1 << i]));

function classify(file: FileAnalysis, occ: Occurrence): [TokenType, TokenModifier[]] | undefined {
    const mods: TokenModifier[] = [];
    if (occ.decl) mods.push('declaration');
    const t = occ.target;
    switch (t.t) {
        case 'sym': {
            const sym = t.sym;
            if (sym.global && sym.kind !== 'import') mods.push('global');
            switch (sym.kind) {
                case 'proc':
                    return ['function', mods];
                case 'import':
                    return ['namespace', mods];
                case 'param':
                    return [occ.call ? 'function' : 'parameter', mods];
                default:
                    if (occ.call) return ['function', mods];
                    if (sym.kind === 'var' && sym.global && sym.writes === 0 && /^[A-Z][A-Z0-9_]*$/.test(sym.name)) {
                        mods.push('readonly');
                    }
                    return ['variable', mods];
            }
        }
        case 'builtin':
            mods.push('defaultLibrary');
            return ['function', mods];
        case 'raw':
            // In a runtime file a `__name` is a reserved operation; a
            // plain unresolved name there lives in a sibling file.
            if (!file.rawMode) return undefined;
            mods.push('defaultLibrary');
            return [occ.call ? 'function' : 'variable', mods];
        case 'reserved':
            mods.push('defaultLibrary', 'readonly');
            return ['variable', mods];
        case 'member':
            return [occ.call ? 'method' : 'property', mods];
        case 'key':
            return ['property', mods];
        case 'unresolved':
            if (file.rawMode) return [occ.call ? 'function' : 'variable', mods];
            return undefined;
        case 'raiser':
            // Left to the grammar, which colours panic/raise as control flow.
            return undefined;
    }
}

/** The LSP wire format: five integers per token, positions relative to the one before. */
export function semanticTokens(file: FileAnalysis): number[] {
    const data: number[] = [];
    let prevLine = 0;
    let prevChar = 0;
    const lineStarts = file.lex.lineStarts;
    let lastStart = -1;
    for (const occ of file.occurrences) {
        if (occ.start <= lastStart) continue;
        const c = classify(file, occ);
        if (!c) continue;
        // A quoted key is a string; let it stay one.
        if (file.text.charCodeAt(occ.start) === 34) continue;
        const pos = positionAt(lineStarts, occ.start);
        const length = occ.end - occ.start;
        let mods = 0;
        for (const m of c[1]) mods |= MOD_BIT.get(m) ?? 0;
        data.push(
            pos.line - prevLine,
            pos.line === prevLine ? pos.character - prevChar : pos.character,
            length,
            TYPE_INDEX.get(c[0]) ?? 0,
            mods
        );
        prevLine = pos.line;
        prevChar = pos.character;
        lastStart = occ.start;
    }
    return data;
}
