// The files the server knows about: open editors first, the disk for
// everything an `import` reaches. Also the checks that need more than
// one file.

import * as fs from 'fs';
import * as path from 'path';
import { URI } from 'vscode-uri';

import { Diag, FileAnalysis, Sym } from './analysis';
import { Inference, ModuleSource } from './infer';

interface Entry {
    analysis: FileAnalysis;
    /** Open in an editor: the text comes from the client, not the disk. */
    open: boolean;
    mtimeMs: number;
    size: number;
    checkedAt: number;
}

export function canonicalUri(uri: string): string {
    try {
        return URI.parse(uri).toString();
    } catch {
        return uri;
    }
}

export function uriToPath(uri: string): string | undefined {
    try {
        const u = URI.parse(uri);
        return u.scheme === 'file' ? u.fsPath : undefined;
    } catch {
        return undefined;
    }
}

export function pathToUri(p: string): string {
    return URI.file(p).toString();
}

const STAT_TTL_MS = 750;
const MAX_INDEXED_FILES = 3000;
const SKIP_DIRS = new Set(['node_modules', '.git', 'target', 'out', 'dist', '.vscode-test']);

export class Workspace implements ModuleSource {
    private readonly entries = new Map<string, Entry>();
    private inference: Inference | undefined;
    private roots: string[] = [];
    private scanned = false;
    private readonly unitCache = new Map<string, string[]>();

    setRoots(roots: string[]): void {
        this.roots = roots;
        this.scanned = false;
    }

    /** One inference per state of the world: any edit starts a new one. */
    get infer(): Inference {
        this.inference ??= new Inference(this);
        return this.inference;
    }

    private touched(): void {
        this.inference = undefined;
    }

    // ---- Documents ------------------------------------------------------

    open(uri: string, text: string): FileAnalysis {
        uri = canonicalUri(uri);
        const analysis = new FileAnalysis(uri, text);
        this.entries.set(uri, { analysis, open: true, mtimeMs: 0, size: 0, checkedAt: 0 });
        this.touched();
        return analysis;
    }

    close(uri: string): void {
        uri = canonicalUri(uri);
        // Fall back to what is on disk.
        this.entries.delete(uri);
        this.touched();
    }

    /** A file changed on disk behind our back. */
    invalidate(uri: string): void {
        uri = canonicalUri(uri);
        const e = this.entries.get(uri);
        if (e && !e.open) this.entries.delete(uri);
        this.unitCache.clear();
        this.touched();
    }

    isOpen(uri: string): boolean {
        return this.entries.get(canonicalUri(uri))?.open === true;
    }

    openUris(): string[] {
        const out: string[] = [];
        for (const [uri, e] of this.entries) if (e.open) out.push(uri);
        return out;
    }

    get(uri: string): FileAnalysis | undefined {
        uri = canonicalUri(uri);
        const e = this.entries.get(uri);
        if (e && e.open) return e.analysis;
        const p = uriToPath(uri);
        if (!p) return e?.analysis;
        const now = Date.now();
        if (e && now - e.checkedAt < STAT_TTL_MS) return e.analysis;
        let st: fs.Stats;
        try {
            st = fs.statSync(p);
            if (!st.isFile()) throw new Error('not a file');
        } catch {
            if (e) { this.entries.delete(uri); this.touched(); }
            return undefined;
        }
        if (e && e.mtimeMs === st.mtimeMs && e.size === st.size) {
            e.checkedAt = now;
            return e.analysis;
        }
        let text: string;
        try {
            text = fs.readFileSync(p, 'utf8');
        } catch {
            return undefined;
        }
        if (text.charCodeAt(0) === 0xfeff) text = text.slice(1);
        const analysis = new FileAnalysis(uri, text);
        this.entries.set(uri, { analysis, open: false, mtimeMs: st.mtimeMs, size: st.size, checkedAt: now });
        this.touched();
        return analysis;
    }

    // ---- Imports --------------------------------------------------------

    /** Where `import "path"` points, from the file that says it. */
    importPath(file: FileAnalysis, raw: string): string | undefined {
        const from = uriToPath(file.uri);
        if (!from) return undefined;
        return path.resolve(path.dirname(from), raw);
    }

    moduleOf(sym: Sym): FileAnalysis | undefined {
        if (sym.kind !== 'import' || sym.importPath === undefined) return undefined;
        const p = this.importPath(sym.file, sym.importPath);
        if (!p) return undefined;
        return this.get(pathToUri(p));
    }

    /** Does `start` reach `goal` by following imports? */
    private reaches(start: FileAnalysis, goal: string, seen: Set<string>): boolean {
        if (start.uri === goal) return true;
        if (seen.has(start.uri) || seen.size > 200) return false;
        seen.add(start.uri);
        for (const imp of start.imports) {
            const m = this.moduleOf(imp);
            if (m && this.reaches(m, goal, seen)) return true;
        }
        return false;
    }

    // ---- Raw mode units -------------------------------------------------

    /**
     * The x86-64 runtime is several files compiled as one unit, with no
     * imports between them: a name one file uses may live in its sibling.
     */
    unitLookup(file: FileAnalysis, name: string): Sym | undefined {
        if (!file.rawMode) return undefined;
        const p = uriToPath(file.uri);
        if (!p) return undefined;
        const dir = path.dirname(p);
        let siblings = this.unitCache.get(dir);
        if (!siblings) {
            try {
                siblings = fs.readdirSync(dir).filter((n) => n.endsWith('.omg')).map((n) => path.join(dir, n));
            } catch {
                siblings = [];
            }
            this.unitCache.set(dir, siblings);
        }
        for (const s of siblings) {
            if (s === p) continue;
            const a = this.get(pathToUri(s));
            const sym = a?.globals.get(name);
            if (sym) return sym;
        }
        return undefined;
    }

    // ---- Whole-workspace index -------------------------------------------

    /** Load every .omg file under the workspace folders, once. */
    scan(): void {
        if (this.scanned) return;
        this.scanned = true;
        let budget = MAX_INDEXED_FILES;
        const walk = (dir: string, depth: number): void => {
            if (budget <= 0 || depth > 12) return;
            let names: fs.Dirent[];
            try {
                names = fs.readdirSync(dir, { withFileTypes: true });
            } catch {
                return;
            }
            for (const d of names) {
                if (budget <= 0) return;
                if (d.isDirectory()) {
                    if (!SKIP_DIRS.has(d.name) && !d.name.startsWith('.')) walk(path.join(dir, d.name), depth + 1);
                } else if (d.isFile() && d.name.endsWith('.omg')) {
                    budget--;
                    this.get(pathToUri(path.join(dir, d.name)));
                }
            }
        };
        for (const r of this.roots) walk(r, 0);
    }

    /** Everything loaded so far. */
    all(): FileAnalysis[] {
        const out: FileAnalysis[] = [];
        for (const e of this.entries.values()) out.push(e.analysis);
        return out;
    }

    // ---- Diagnostics ----------------------------------------------------

    /** The checks that look beyond one file. */
    crossFileDiagnostics(file: FileAnalysis): Diag[] {
        const out: Diag[] = [];
        const infer = this.infer;

        for (const imp of file.imports) {
            const node = imp.node;
            if (!node || node.type !== 'import') continue;
            const p = this.importPath(file, node.path);
            if (!p) continue;
            const target = this.get(pathToUri(p));
            if (!target) {
                out.push({
                    severity: 'error', code: 'import-not-found',
                    message: "ModuleImportError: cannot read '" + p + "'. Import paths are relative to the importing file.",
                    start: node.pathSpan.start, end: node.pathSpan.end
                });
                continue;
            }
            if (this.reaches(target, file.uri, new Set())) {
                out.push({
                    severity: 'error', code: 'import-cycle',
                    message: "ModuleImportError: recursive import of '" + p + "'. That file imports this one back.",
                    start: node.pathSpan.start, end: node.pathSpan.end
                });
                continue;
            }
            if (target.hasSyntaxError) {
                const e = target.syntaxErrors[0];
                out.push({
                    severity: 'error', code: 'import-broken',
                    message: path.basename(p) + ' does not compile: ' + e.message + ' on line ' + e.line + '.',
                    start: node.pathSpan.start, end: node.pathSpan.end
                });
            }
        }

        if (file.rawMode) return out;

        for (const occ of file.occurrences) {
            if (occ.target.t !== 'member') continue;
            const container = infer.valueOf(file, occ.target.obj, 0);
            if (!container) continue;
            if (container.k === 'module') {
                const name = occ.target.name;
                if (occ.write) {
                    out.push({
                        severity: 'warning', code: 'frozen-write',
                        message: 'Imported modules are read-only: this raises a FrozenWriteError.',
                        start: occ.start, end: occ.end
                    });
                } else if (!container.file.exports.has(name)) {
                    const base = path.basename(uriToPath(container.file.uri) ?? container.file.uri);
                    const inner = container.file.globals.get(name);
                    out.push({
                        severity: 'warning', code: 'unknown-member',
                        message: inner
                            ? base + " declares '" + name + "' inside a block, so it is not exported. Only top-level `proc` and `alloc` statements are."
                            : base + " has no top-level proc or alloc named '" + name + "'. This raises a KeyError.",
                        start: occ.start, end: occ.end
                    });
                }
            } else if (container.k === 'dict' && container.frozen && occ.write) {
                out.push({
                    severity: 'warning', code: 'frozen-write',
                    message: 'This dict is frozen: writing to it raises a FrozenWriteError.',
                    start: occ.start, end: occ.end
                });
            }
        }

        for (const site of file.calls) {
            if (site.checked) continue;
            const v = infer.valueOf(file, site.call.callee, 0);
            if (!v || v.k !== 'proc' || !v.sym.params) continue;
            const want = v.sym.params.length;
            const got = site.call.args.length;
            if (want === got) continue;
            out.push({
                severity: 'warning', code: 'arity',
                message: "proc '" + v.sym.name + "' takes " + want + ' argument' + (want === 1 ? '' : 's') + ', got ' + got + '. This raises a TypeError when it runs.',
                start: site.call.callee.start, end: site.call.rparen
            });
        }
        return out;
    }
}
