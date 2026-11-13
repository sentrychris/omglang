// Bundles the extension: one file for the client, one for the language
// server. Everything they need from node_modules is folded in, so the
// packaged extension carries no node_modules of its own.
//
//   node scripts/build.mjs               development build
//   node scripts/build.mjs --production  minified, no source maps
//   node scripts/build.mjs --watch       rebuild on change
//   node scripts/build.mjs --tests       also bundle test/*.test.ts

import { build, context } from 'esbuild';
import { readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const production = process.argv.includes('--production');
const watch = process.argv.includes('--watch');
const tests = process.argv.includes('--tests');

const common = {
    bundle: true,
    platform: 'node',
    target: 'node18',
    format: 'cjs',
    sourcemap: !production,
    minify: production,
    logLevel: 'info',
    absWorkingDir: root
};

const targets = [
    { ...common, entryPoints: ['client/src/extension.ts'], outfile: 'out/extension.js', external: ['vscode'] },
    { ...common, entryPoints: ['server/src/server.ts'], outfile: 'out/server.js' }
];

if (tests) {
    const files = readdirSync(join(root, 'test')).filter((f) => f.endsWith('.test.ts'));
    targets.push({
        ...common,
        entryPoints: files.map((f) => 'test/' + f),
        outdir: 'out/test',
        sourcemap: 'inline',
        minify: false,
        external: ['vscode-oniguruma', 'vscode-textmate']
    });
}

if (watch) {
    for (const t of targets) {
        const ctx = await context(t);
        await ctx.watch();
    }
} else {
    await Promise.all(targets.map((t) => build(t)));
}
