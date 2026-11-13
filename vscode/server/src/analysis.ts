// Scope analysis of one OMG file.
//
// The walk follows `compile_stmt` / `compile_expr` in src/compiler.omg
// step for step, because OMG's name rules are the compiler's: two levels
// (file and proc), no block scope, a closure sees only what the proc
// around it had declared above, and a call to a builtin name is always
// the builtin. What the compiler rejects is reported here as an error in
// the compiler's words; what compiles but will fail when it runs is a
// warning.

import {
    CallExpr, DictExpr, Expr, FuncStmt, IdExpr, Name, Span, Stmt, isBlock
} from './ast';
import { builtin, isBuiltin, isInternalRaiser, raiser, rawOp, reservedGlobal } from './builtins';
import { FOREIGN_WORDS } from './language';
import { Comment, LexResult, SyntaxIssue, Token, tokenize } from './lexer';
import { parse } from './parser';

export type SymKind = 'proc' | 'var' | 'param' | 'import' | 'loopvar' | 'except';

export interface Ref extends Span {
    write: boolean;
    decl: boolean;
}

export interface Field {
    name: string;
    /** Where the key is written. */
    span: Span;
    value?: Expr;
    file: FileAnalysis;
}

export interface Sym {
    name: string;
    kind: SymKind;
    /** Declared at file level: visible everywhere in the file. */
    global: boolean;
    /** Span of the name where it is declared. */
    start: number;
    end: number;
    /** Span of the declaring statement. */
    declStart: number;
    declEnd: number;
    file: FileAnalysis;
    /** The proc this is local to. */
    owner?: Sym;
    node?: Stmt;
    params?: Name[];
    init?: Expr;
    /** For a loop variable: what it walks. */
    iter?: Expr;
    importPath?: string;
    doc?: string;
    refs: Ref[];
    reads: number;
    writes: number;
    /** Keys set with `name.key := value` after the declaration. */
    fields?: Map<string, Field>;
    /** A top-level statement of the file itself, so an importer can see it. */
    exported: boolean;
}

export type Target =
    | { t: 'sym'; sym: Sym }
    | { t: 'builtin'; name: string }
    | { t: 'raiser'; name: string }
    | { t: 'reserved'; name: string }
    | { t: 'raw'; name: string }
    | { t: 'member'; obj: Expr; name: string }
    | { t: 'key'; dict: DictExpr; name: string }
    | { t: 'unresolved'; name: string };

export interface Occurrence extends Span {
    target: Target;
    /** The name is being called. */
    call: boolean;
    decl: boolean;
    write: boolean;
}

export type Severity = 'error' | 'warning' | 'hint';

export interface Diag extends Span {
    severity: Severity;
    message: string;
    code: string;
    /** Greyed out rather than underlined. */
    unnecessary?: boolean;
    related?: { span: Span; message: string }[];
}

export interface ProcInfo {
    sym: Sym;
    node: FuncStmt;
    parent?: ProcInfo;
    /** Every name the proc declares at its own level. */
    locals: Map<string, Sym>;
}

export interface CallSite {
    call: CallExpr;
    /** The arity has been checked the way the compiler checks it. */
    checked: boolean;
}

const RAISE_ENDERS = new Set(['exit', 'exit_with_error']);

export class FileAnalysis {
    readonly lex: LexResult;
    readonly stmts: Stmt[];
    /** Lexer errors, then parser errors, in source order. */
    readonly syntaxErrors: SyntaxIssue[] = [];
    readonly diagnostics: Diag[] = [];
    readonly globals = new Map<string, Sym>();
    readonly exports = new Map<string, Sym>();
    readonly symbols: Sym[] = [];
    readonly occurrences: Occurrence[] = [];
    readonly idTargets = new Map<IdExpr, Target>();
    readonly procs: ProcInfo[] = [];
    readonly calls: CallSite[] = [];
    readonly imports: Sym[] = [];
    /** Uses the reserved low-level names: a file of the x86-64 runtime. */
    rawMode = false;

    private readonly procByNode = new Map<FuncStmt, ProcInfo>();

    constructor(readonly uri: string, readonly text: string) {
        this.lex = tokenize(text);
        const parsed = parse(this.lex);
        this.stmts = parsed.stmts;
        this.collectSyntaxErrors(parsed.errors);
        new Walker(this).run();
        this.occurrences.sort((a, b) => a.start - b.start);
    }

    get hasSyntaxError(): boolean {
        return this.syntaxErrors.length > 0;
    }

    private collectSyntaxErrors(parseErrors: SyntaxIssue[]): void {
        // The compiler lexes the whole file before it parses any of it,
        // so a lexer error is the one it reports, wherever it is. What
        // the parser makes of a garbled token stream is noise.
        const lexErrors = this.lex.errors;
        if (lexErrors.length > 0) {
            for (const e of lexErrors) this.syntaxErrors.push(e);
        } else if (parseErrors.length > 0) {
            this.syntaxErrors.push(parseErrors[0]);
        }
    }

    registerProc(info: ProcInfo): void {
        this.procs.push(info);
        this.procByNode.set(info.node, info);
    }

    procInfo(node: FuncStmt): ProcInfo | undefined {
        return this.procByNode.get(node);
    }

    /** The innermost proc whose body holds `offset`. */
    procAt(offset: number): ProcInfo | undefined {
        let best: ProcInfo | undefined;
        for (const p of this.procs) {
            if (offset >= p.node.start && offset <= p.node.end) {
                if (!best || p.node.start >= best.node.start) best = p;
            }
        }
        return best;
    }

    occurrenceAt(offset: number): Occurrence | undefined {
        const occ = this.occurrences;
        let lo = 0;
        let hi = occ.length - 1;
        while (lo <= hi) {
            const mid = (lo + hi) >> 1;
            const o = occ[mid];
            if (offset < o.start) hi = mid - 1;
            else if (offset > o.end) lo = mid + 1;
            else return o;
        }
        return undefined;
    }

    tokenAt(offset: number): Token | undefined {
        const toks = this.lex.tokens;
        let lo = 0;
        let hi = toks.length - 1;
        while (lo <= hi) {
            const mid = (lo + hi) >> 1;
            const t = toks[mid];
            if (offset < t.start) hi = mid - 1;
            else if (offset >= t.end) lo = mid + 1;
            else return t;
        }
        return undefined;
    }

    commentAt(offset: number): Comment | undefined {
        for (const c of this.lex.comments) {
            if (c.start >= offset) break;
            if (offset > c.end) continue;
            if (!c.block) return c;
            const closed = c.end - c.start >= 4 && this.text.startsWith('*/', c.end - 2);
            if (!closed || offset < c.end) return c;
        }
        return undefined;
    }

    /** The string literal the offset is inside of, quotes excluded. */
    stringAt(offset: number): Token | undefined {
        const i = this.tokenIndexBefore(offset);
        if (i < 0) return undefined;
        const t = this.lex.tokens[i];
        if (t.kind !== 'STR') return undefined;
        const closed = t.end - t.start >= 2 && this.text[t.end - 1] === '"';
        if (offset < t.end || (!closed && offset <= t.end)) return t;
        return undefined;
    }

    /** Index of the last token that starts before `offset`, or -1. */
    tokenIndexBefore(offset: number): number {
        const toks = this.lex.tokens;
        let lo = 0;
        let hi = toks.length - 1;
        let ans = -1;
        while (lo <= hi) {
            const mid = (lo + hi) >> 1;
            if (toks[mid].start < offset) { ans = mid; lo = mid + 1; } else { hi = mid - 1; }
        }
        return ans;
    }

    /**
     * What `name` means at `offset`, for code that did not parse (someone
     * is typing): locals of the procs around it, then the file's names.
     */
    lookup(name: string, offset: number): Target {
        for (let p = this.procAt(offset); p; p = p.parent) {
            const s = p.locals.get(name);
            if (s) return { t: 'sym', sym: s };
        }
        const g = this.globals.get(name);
        if (g) return { t: 'sym', sym: g };
        return classifyFree(name);
    }

    /** Every name that can be used at `offset`. */
    visibleSymbols(offset: number): Sym[] {
        const seen = new Set<string>();
        const out: Sym[] = [];
        for (let p = this.procAt(offset); p; p = p.parent) {
            for (const s of p.locals.values()) {
                if (!seen.has(s.name)) { seen.add(s.name); out.push(s); }
            }
        }
        for (const s of this.globals.values()) {
            if (!seen.has(s.name)) { seen.add(s.name); out.push(s); }
        }
        return out;
    }
}

export function classifyFree(name: string): Target {
    if (raiser(name) || isInternalRaiser(name)) return { t: 'raiser', name };
    if (isBuiltin(name)) return { t: 'builtin', name };
    if (reservedGlobal(name)) return { t: 'reserved', name };
    if (name.startsWith('__')) return { t: 'raw', name };
    return { t: 'unresolved', name };
}

interface Scope {
    /** Declared so far, in the order the compiler sees them. */
    declared: Map<string, Sym>;
    /** Everything this body declares at its own level, wherever it is. */
    all: Map<string, Sym>;
    info?: ProcInfo;
    loopDepth: number;
}

class Walker {
    private readonly f: FileAnalysis;
    private readonly scopes: Scope[] = [];
    /** Enclosing nested procs, innermost last, with the scope depth of each. */
    private readonly nested: { name: string; sym: Sym; depth: number }[] = [];
    /** First top-level `proc` statement of each name: the compiler's arity table. */
    private readonly topArity = new Map<string, number>();
    private readonly topAllocated = new Set<string>();
    private readonly lineOf: (offset: number) => number;

    constructor(f: FileAnalysis) {
        this.f = f;
        const starts = f.lex.lineStarts;
        this.lineOf = (offset: number): number => {
            let lo = 0;
            let hi = starts.length - 1;
            while (lo < hi) {
                const mid = (lo + hi + 1) >> 1;
                if (starts[mid] <= offset) lo = mid; else hi = mid - 1;
            }
            return lo + 1;
        };
    }

    run(): void {
        const f = this.f;
        const toks = f.lex.tokens;
        for (let i = 0; i + 1 < toks.length; i++) {
            const t = toks[i];
            if (t.kind === 'ID' && t.text.startsWith('__') && toks[i + 1].kind === 'LPAREN' && rawOp(t.text)) {
                f.rawMode = true;
                break;
            }
        }
        const top: Scope = { declared: new Map(), all: new Map(), loopDepth: 0 };
        this.scopes.push(top);
        this.predeclare(f.stmts, top, undefined, true);
        for (const s of f.stmts) {
            if (s.type === 'func' && !this.topArity.has(s.name.name)) {
                this.topArity.set(s.name.name, s.params.length);
            }
        }
        for (const [name, sym] of top.all) f.globals.set(name, sym);
        this.block(f.stmts);
        this.finish();
    }

    // ---- Declarations ---------------------------------------------------

    /**
     * Make a symbol for every name a body declares at its own level
     * (`cc_collect_declared`): allocs, procs, import aliases, loop
     * variables and except bindings, but nothing inside a nested proc.
     */
    private predeclare(stmts: Stmt[], scope: Scope, owner: ProcInfo | undefined, direct: boolean): void {
        for (const s of stmts) {
            switch (s.type) {
                case 'decl':
                    this.declareIn(scope, owner, s.name, 'var', s, direct).init ??= s.init;
                    break;
                case 'func': {
                    const sym = this.declareIn(scope, owner, s.name, 'proc', s, direct);
                    if (sym.node === s) sym.params = s.params;
                    break;
                }
                case 'import': {
                    const sym = this.declareIn(scope, owner, s.alias, 'import', s, false);
                    if (sym.node === s) {
                        sym.importPath = s.path;
                        this.f.imports.push(sym);
                    }
                    break;
                }
                case 'if':
                    this.predeclare(s.then.stmts, scope, owner, false);
                    for (let e = s.otherwise; e; ) {
                        if (isBlock(e)) {
                            this.predeclare(e.stmts, scope, owner, false);
                            break;
                        }
                        this.predeclare(e.then.stmts, scope, owner, false);
                        e = e.otherwise;
                    }
                    break;
                case 'loop':
                    this.predeclare(s.body.stmts, scope, owner, false);
                    break;
                case 'for': {
                    const sym = this.declareIn(scope, owner, s.name, 'loopvar', s, false);
                    if (sym.node === s) sym.iter = s.iter;
                    this.predeclare(s.body.stmts, scope, owner, false);
                    break;
                }
                case 'try':
                    this.predeclare(s.body.stmts, scope, owner, false);
                    if (s.excName) this.declareIn(scope, owner, s.excName, 'except', s, false);
                    this.predeclare(s.handler.stmts, scope, owner, false);
                    break;
                default:
                    break;
            }
        }
    }

    private declareIn(scope: Scope, owner: ProcInfo | undefined, name: Name, kind: SymKind, node: Stmt, direct: boolean): Sym {
        const existing = scope.all.get(name.name);
        if (existing) return existing;
        const f = this.f;
        const sym: Sym = {
            name: name.name, kind,
            global: owner === undefined,
            start: name.start, end: name.end,
            declStart: node.start, declEnd: node.end,
            file: f, owner: owner?.sym, node,
            refs: [], reads: 0, writes: 0,
            exported: owner === undefined && direct && (kind === 'var' || kind === 'proc')
        };
        if (kind === 'proc' || kind === 'var') sym.doc = docComment(f, node);
        scope.all.set(name.name, sym);
        f.symbols.push(sym);
        if (sym.exported) f.exports.set(sym.name, sym);
        return sym;
    }

    private newParam(owner: ProcInfo, p: Name, node: FuncStmt): Sym {
        const sym: Sym = {
            name: p.name, kind: 'param', global: false,
            start: p.start, end: p.end, declStart: p.start, declEnd: p.end,
            file: this.f, owner: owner.sym, node,
            refs: [], reads: 0, writes: 0, exported: false
        };
        this.f.symbols.push(sym);
        return sym;
    }

    // ---- Statements -----------------------------------------------------

    private get scope(): Scope {
        return this.scopes[this.scopes.length - 1];
    }

    private get inProc(): boolean {
        return this.scopes.length > 1;
    }

    private block(stmts: Stmt[]): void {
        let dead: Stmt | undefined;
        for (const s of stmts) {
            this.stmt(s);
            if (!dead && s !== stmts[stmts.length - 1] && this.endsFlow(s)) dead = s;
        }
        if (dead) {
            const i = stmts.indexOf(dead);
            const from = stmts[i + 1];
            const to = stmts[stmts.length - 1];
            this.f.diagnostics.push({
                severity: 'hint', code: 'unreachable', unnecessary: true,
                message: 'Unreachable code: nothing after `' + this.enderName(dead) + '` runs.',
                start: from.start, end: to.end
            });
        }
    }

    private endsFlow(s: Stmt): boolean {
        if (s.type === 'return' || s.type === 'break' || s.type === 'continue') return true;
        if (s.type === 'expr' && s.expr.type === 'call' && s.expr.callee.type === 'id') {
            const n = s.expr.callee.name;
            const t = this.f.idTargets.get(s.expr.callee);
            if (t && t.t === 'raiser') return true;
            if (t && t.t === 'builtin' && RAISE_ENDERS.has(n)) return true;
        }
        return false;
    }

    private enderName(s: Stmt): string {
        if (s.type === 'expr' && s.expr.type === 'call' && s.expr.callee.type === 'id') return s.expr.callee.name + '(...)';
        return s.type;
    }

    private stmt(s: Stmt): void {
        const f = this.f;
        switch (s.type) {
            case 'emit':
            case 'facts':
                this.expr(s.expr);
                break;
            case 'decl': {
                if (!this.inProc) {
                    if (this.topAllocated.has(s.name.name)) {
                        const first = this.scope.all.get(s.name.name);
                        f.diagnostics.push({
                            severity: 'error', code: 'duplicate-alloc',
                            message: "SyntaxError: '" + s.name.name + "' is already declared at the top level. Drop the `alloc` to assign to it.",
                            start: s.name.start, end: s.name.end,
                            related: first ? [{ span: first, message: 'First declared here.' }] : undefined
                        });
                    }
                    this.topAllocated.add(s.name.name);
                }
                this.expr(s.init);
                this.declare(s.name);
                break;
            }
            case 'assign': {
                this.expr(s.value);
                const target = this.resolve(s.name, false);
                this.note(s.name, target, { write: true });
                if (target.t !== 'sym') {
                    this.undeclaredStore(s.name, target);
                }
                break;
            }
            case 'attr_assign': {
                this.expr(s.obj);
                this.expr(s.value);
                this.f.occurrences.push({
                    start: s.attr.start, end: s.attr.end,
                    target: { t: 'member', obj: s.obj, name: s.attr.name },
                    call: false, decl: false, write: true
                });
                this.recordField(s.obj, s.attr.name, s.attr, s.value);
                break;
            }
            case 'index_assign':
                this.expr(s.obj);
                this.expr(s.index);
                this.expr(s.value);
                if (s.index.type === 'str' && /^[A-Za-z_][A-Za-z0-9_]*$/.test(s.index.value)) {
                    this.recordField(s.obj, s.index.value, { start: s.index.start + 1, end: s.index.end - 1 }, s.value);
                }
                break;
            case 'expr':
                this.expr(s.expr);
                this.noEffect(s.expr);
                break;
            case 'import':
                this.declare(s.alias);
                break;
            case 'if': {
                this.expr(s.cond);
                this.block(s.then.stmts);
                if (s.otherwise) {
                    if (isBlock(s.otherwise)) this.block(s.otherwise.stmts);
                    else this.stmt(s.otherwise);
                }
                break;
            }
            case 'loop':
                this.expr(s.cond);
                this.scope.loopDepth++;
                this.block(s.body.stmts);
                this.scope.loopDepth--;
                break;
            case 'for':
                this.expr(s.iter);
                this.declare(s.name);
                this.scope.loopDepth++;
                this.block(s.body.stmts);
                this.scope.loopDepth--;
                break;
            case 'try':
                this.block(s.body.stmts);
                if (s.excName) this.declare(s.excName);
                this.block(s.handler.stmts);
                break;
            case 'func':
                this.func(s);
                break;
            case 'return':
                if (!this.inProc) {
                    f.diagnostics.push({
                        severity: 'error', code: 'return-outside-proc',
                        message: "SyntaxError: 'return' outside of a proc. To stop the program, use `exit(0)`.",
                        start: s.start, end: s.start + 6
                    });
                }
                this.expr(s.expr);
                break;
            case 'break':
            case 'continue':
                if (this.scope.loopDepth === 0) {
                    f.diagnostics.push({
                        severity: 'error', code: s.type + '-outside-loop',
                        message: "SyntaxError: '" + s.type + "' outside of loop",
                        start: s.start, end: s.end
                    });
                }
                break;
        }
    }

    private func(s: FuncStmt): void {
        const f = this.f;
        const enclosing = this.scope;
        const nested = this.inProc;
        const sym = enclosing.all.get(s.name.name);
        const isFirst = sym !== undefined && sym.node === s;

        if (sym) {
            sym.refs.push({ start: s.name.start, end: s.name.end, write: true, decl: true });
            f.occurrences.push({
                start: s.name.start, end: s.name.end,
                target: { t: 'sym', sym }, call: false, decl: true, write: true
            });
            if (!isFirst) {
                f.diagnostics.push({
                    severity: 'warning', code: 'redefined',
                    message: "'" + s.name.name + "' is already defined in this " + (nested ? 'proc' : 'file') +
                        (sym.kind === 'proc' ? '. The later definition replaces the earlier one.' : ' as a variable.'),
                    start: s.name.start, end: s.name.end,
                    related: [{ span: sym, message: 'First defined here.' }]
                });
            }
        }
        if (!f.rawMode && (isBuiltin(s.name.name) || raiser(s.name.name))) {
            f.diagnostics.push({
                severity: 'warning', code: 'shadows-builtin',
                message: "'" + s.name.name + "' is a builtin. A call to " + s.name.name + '(...) always runs the builtin, never this proc. Pick another name.',
                start: s.name.start, end: s.name.end
            });
        }

        const parentInfo = enclosing.info;
        // A second definition of the same name still gets its own body
        // analysed, under a symbol of its own that nothing else sees.
        const procSym: Sym = isFirst && sym ? sym : {
            name: s.name.name, kind: 'proc', global: !nested,
            start: s.name.start, end: s.name.end, declStart: s.start, declEnd: s.end,
            file: f, owner: parentInfo?.sym, node: s, params: s.params,
            refs: [], reads: 1, writes: 0, exported: false
        };
        const info: ProcInfo = { sym: procSym, node: s, parent: parentInfo, locals: new Map() };
        f.registerProc(info);

        if (nested) {
            // Declared before the body, so the proc can call itself.
            if (sym) enclosing.declared.set(s.name.name, sym);
            this.nested.push({ name: s.name.name, sym: procSym, depth: this.scopes.length });
        }

        const scope: Scope = { declared: new Map(), all: new Map(), info, loopDepth: 0 };
        for (const p of s.params) {
            const ps = this.newParam(info, p, s);
            ps.refs.push({ start: p.start, end: p.end, write: true, decl: true });
            f.occurrences.push({ start: p.start, end: p.end, target: { t: 'sym', sym: ps }, call: false, decl: true, write: true });
            scope.declared.set(p.name, ps);
            scope.all.set(p.name, ps);
        }
        this.predeclare(s.body.stmts, scope, info, false);
        for (const [name, lsym] of scope.all) info.locals.set(name, lsym);

        this.scopes.push(scope);
        this.block(s.body.stmts);
        this.scopes.pop();
        if (nested) this.nested.pop();
    }

    /** `alloc`, `for`, `except` and `import` all bind a name in the current body. */
    private declare(name: Name): void {
        const scope = this.scope;
        const sym = scope.all.get(name.name);
        if (!sym) return;
        scope.declared.set(name.name, sym);
        const first = sym.start === name.start;
        sym.refs.push({ start: name.start, end: name.end, write: true, decl: true });
        if (!first) sym.writes++;
        this.f.occurrences.push({
            start: name.start, end: name.end,
            target: { t: 'sym', sym }, call: false, decl: true, write: true
        });
    }

    private recordField(obj: Expr, name: string, span: Span, value: Expr): void {
        if (obj.type !== 'id') return;
        const t = this.f.idTargets.get(obj);
        if (!t || t.t !== 'sym' || t.sym.file !== this.f) return;
        const sym = t.sym;
        if (sym.kind !== 'var') return;
        sym.fields ??= new Map();
        if (!sym.fields.has(name)) sym.fields.set(name, { name, span, value, file: this.f });
    }

    // ---- Names ----------------------------------------------------------

    /**
     * What a name means where the walk stands: the compiler's
     * `cc_resolve_load`, plus the cases it leaves to run time.
     */
    private resolve(name: Name, isCall: boolean): Target {
        const n = name.name;
        const scopes = this.scopes;
        for (let i = scopes.length - 1; i >= 1; i--) {
            const s = scopes[i].declared.get(n);
            if (s) return { t: 'sym', sym: s };
        }
        const top = scopes[0].all.get(n);
        // `cc_check_not_declared_later`: a nested proc reaching for a name
        // an enclosing proc only declares further down.
        if (scopes.length > 2 && !reservedGlobal(n) && !top) {
            for (let i = 1; i < scopes.length - 1; i++) {
                const later = scopes[i].all.get(n);
                if (later) {
                    this.f.diagnostics.push({
                        severity: 'error', code: 'captured-before-declared',
                        message: "SyntaxError: '" + n + "' is used in a nested proc before the enclosing proc declares it. Move its `alloc` above the nested proc.",
                        start: name.start, end: name.end,
                        related: [{ span: later, message: "'" + n + "' is declared here." }]
                    });
                    return { t: 'sym', sym: later };
                }
            }
        }
        if (top) {
            if (!this.inProc && !isCall && top.kind !== 'proc' && !scopes[0].declared.has(n)) {
                this.f.diagnostics.push({
                    severity: 'warning', code: 'used-before-alloc',
                    message: "'" + n + "' is used before its `alloc` on line " + this.lineOf(top.start) + ' has run.',
                    start: name.start, end: name.end
                });
            }
            return { t: 'sym', sym: top };
        }
        if (scopes.length > 1) {
            const own = scopes[scopes.length - 1].all.get(n);
            if (own) {
                this.f.diagnostics.push({
                    severity: 'warning', code: 'used-before-alloc',
                    message: "'" + n + "' is used before its `alloc` on line " + this.lineOf(own.start) + '. At this point the name does not exist yet.',
                    start: name.start, end: name.end
                });
                return { t: 'sym', sym: own };
            }
        }
        return classifyFree(n);
    }

    private note(name: Span, target: Target, how: { write?: boolean; call?: boolean }): void {
        const write = how.write === true;
        this.f.occurrences.push({
            start: name.start, end: name.end, target,
            call: how.call === true, decl: false, write
        });
        if (target.t === 'sym') {
            target.sym.refs.push({ start: name.start, end: name.end, write, decl: false });
            if (write) target.sym.writes++; else target.sym.reads++;
        }
    }

    private undeclaredStore(name: Name, target: Target): void {
        if (this.f.rawMode) return;
        let message = "'" + name.name + "' has not been declared. Write `alloc " + name.name + ' := ...` to create it.';
        if (target.t === 'builtin' || target.t === 'raiser') {
            message = "'" + name.name + "' is a builtin, not a variable. Declare your own with `alloc`.";
        } else if (target.t === 'reserved' || target.t === 'raw') {
            return;
        }
        this.f.diagnostics.push({
            severity: 'warning', code: 'undeclared-assignment', message,
            start: name.start, end: name.end
        });
    }

    private noEffect(e: Expr): void {
        if (e.type === 'call') return;
        if (e.type === 'id' && Object.prototype.hasOwnProperty.call(FOREIGN_WORDS, e.name)) {
            const t = this.f.idTargets.get(e);
            if (!t || t.t === 'unresolved') {
                // Replace the "not defined" warning with the useful one.
                const ds = this.f.diagnostics;
                for (let i = ds.length - 1; i >= 0; i--) {
                    if (ds[i].code === 'undefined-name' && ds[i].start === e.start) ds.splice(i, 1);
                }
                ds.push({
                    severity: 'warning', code: 'foreign-keyword',
                    message: '`' + e.name + '` is not an OMG keyword. ' + FOREIGN_WORDS[e.name],
                    start: e.start, end: e.end
                });
                return;
            }
        }
        this.f.diagnostics.push({
            severity: 'warning', code: 'no-effect',
            message: 'This statement does nothing: the value is worked out and thrown away. Did you mean `emit`, a call, or `:=`?',
            start: e.start, end: e.end
        });
    }

    // ---- Expressions ----------------------------------------------------

    private expr(e: Expr): void {
        const f = this.f;
        switch (e.type) {
            case 'num':
            case 'str':
            case 'bool':
                break;
            case 'id': {
                const target = this.resolve(e, false);
                f.idTargets.set(e, target);
                this.note(e, target, {});
                this.checkBareName(e, target);
                break;
            }
            case 'list':
                for (const x of e.elems) this.expr(x);
                break;
            case 'dict':
                for (const p of e.pairs) {
                    f.occurrences.push({
                        start: p.key.start, end: p.key.end,
                        target: { t: 'key', dict: e, name: p.key.name },
                        call: false, decl: true, write: false
                    });
                    this.expr(p.value);
                }
                break;
            case 'index':
                this.expr(e.obj);
                this.expr(e.index);
                break;
            case 'slice':
                this.expr(e.obj);
                this.expr(e.from);
                if (e.to) this.expr(e.to);
                break;
            case 'dot':
                this.expr(e.obj);
                f.occurrences.push({
                    start: e.attr.start, end: e.attr.end,
                    target: { t: 'member', obj: e.obj, name: e.attr.name },
                    call: false, decl: false, write: false
                });
                break;
            case 'unary':
                this.expr(e.operand);
                break;
            case 'bin':
                this.expr(e.left);
                this.expr(e.right);
                break;
            case 'call':
                this.call(e);
                break;
        }
    }

    private call(e: CallExpr): void {
        const f = this.f;
        const callee = e.callee;
        if (callee.type !== 'id') {
            this.expr(callee);
            if (callee.type === 'dot') {
                const occ = f.occurrences[f.occurrences.length - 1];
                if (occ && occ.start === callee.attr.start) occ.call = true;
            }
            for (const a of e.args) this.expr(a);
            f.calls.push({ call: e, checked: false });
            return;
        }

        const name = callee.name;
        const site: CallSite = { call: e, checked: false };
        f.calls.push(site);

        // The compiler's order: raise helpers, then builtins, then names
        // bound to a value, then top-level procs.
        if (raiser(name) || isInternalRaiser(name)) {
            const target: Target = { t: 'raiser', name };
            f.idTargets.set(callee, target);
            this.note(callee, target, { call: true });
            site.checked = true;
            const doc = raiser(name);
            if (doc && e.args.length > 1) {
                f.diagnostics.push({
                    severity: 'warning', code: 'builtin-arity',
                    message: name + '() takes one message; the other arguments are ignored.',
                    start: e.args[1].start, end: e.args[e.args.length - 1].end
                });
            }
            for (const a of e.args) this.expr(a);
            return;
        }
        if (isBuiltin(name) && !f.rawMode) {
            const target: Target = { t: 'builtin', name };
            f.idTargets.set(callee, target);
            this.note(callee, target, { call: true });
            site.checked = true;
            const doc = builtin(name)!;
            const got = e.args.length;
            if (got < doc.minArgs || got > doc.maxArgs) {
                const want = doc.minArgs === doc.maxArgs
                    ? String(doc.minArgs)
                    : doc.minArgs + ' or ' + doc.maxArgs;
                f.diagnostics.push({
                    severity: 'warning', code: 'builtin-arity',
                    message: doc.signatures.join(' / ') + ' takes ' + want + ' argument' + (want === '1' ? '' : 's') + ', got ' + got + '. This raises a TypeError when it runs.',
                    start: callee.start, end: e.rparen
                });
            }
            for (const a of e.args) this.expr(a);
            return;
        }

        const target = this.resolve(callee, true);
        f.idTargets.set(callee, target);
        this.note(callee, target, { call: true });

        if (target.t === 'sym') {
            const sym = target.sym;
            const valueBound = this.isValueBinding(name);
            if (valueBound) {
                // A call to an enclosing nested proc (recursion) is the
                // one closure call whose arity the compiler knows.
                const enc = this.enclosingNested(name);
                if (enc && enc.params && enc.params.length !== e.args.length) {
                    this.arityError(callee, e, name, enc.params.length);
                }
                if (enc) site.checked = true;
            } else if (sym.global) {
                if (sym.kind === 'proc') {
                    const want = this.topArity.get(name);
                    if (want !== undefined) {
                        site.checked = true;
                        if (want !== e.args.length) this.arityError(callee, e, name, want);
                    }
                } else if (!f.rawMode) {
                    site.checked = true;
                    f.diagnostics.push({
                        severity: 'warning', code: 'call-before-alloc',
                        message: "'" + name + "' is a variable declared further down (line " + this.lineOf(sym.start) +
                            '). A call written above the `alloc` looks for a proc of that name and fails with an UndefinedIdentError. Move the `alloc` above this code.',
                        start: callee.start, end: callee.end
                    });
                }
            }
        } else if (target.t === 'unresolved' && !f.rawMode) {
            site.checked = true;
            f.diagnostics.push({
                severity: 'warning', code: 'undefined-name',
                message: "No proc or variable named '" + name + "' in this file." + this.suggest(name, true),
                start: callee.start, end: callee.end
            });
        }
        for (const a of e.args) this.expr(a);
    }

    private arityError(callee: IdExpr, e: CallExpr, name: string, want: number): void {
        this.f.diagnostics.push({
            severity: 'error', code: 'arity',
            message: "SyntaxError: function '" + name + "' expects " + want + ' argument' + (want === 1 ? '' : 's') + ', got ' + e.args.length,
            start: callee.start, end: e.rparen
        });
    }

    /** `cc_is_value_binding`: declared so far, in any scope including the file's. */
    private isValueBinding(name: string): boolean {
        for (const s of this.scopes) {
            if (s.declared.has(name)) return true;
        }
        return false;
    }

    /** `cc_enclosing_nested_key`: an enclosing nested proc of that name, unless shadowed. */
    private enclosingNested(name: string): Sym | undefined {
        for (let i = this.nested.length - 1; i >= 0; i--) {
            const n = this.nested[i];
            if (n.name !== name) continue;
            for (let d = n.depth; d < this.scopes.length; d++) {
                if (this.scopes[d].declared.has(name)) return undefined;
            }
            return n.sym;
        }
        return undefined;
    }

    private checkBareName(e: IdExpr, target: Target): void {
        const f = this.f;
        if (f.rawMode) return;
        if (target.t === 'builtin' || target.t === 'raiser') {
            f.diagnostics.push({
                severity: 'warning', code: 'builtin-as-value',
                message: "'" + e.name + "' is a builtin and can only be called. To pass it around, wrap it in a proc.",
                start: e.start, end: e.end
            });
        } else if (target.t === 'unresolved') {
            f.diagnostics.push({
                severity: 'warning', code: 'undefined-name',
                message: "'" + e.name + "' is not defined in this file." + this.suggest(e.name, false),
                start: e.start, end: e.end
            });
        }
    }

    /** A near miss among the names in reach, for typos. */
    private suggest(name: string, wantCallable: boolean): string {
        if (Object.prototype.hasOwnProperty.call(FOREIGN_WORDS, name)) return ' ' + FOREIGN_WORDS[name];
        const candidates = new Set<string>();
        for (const s of this.scopes) for (const k of s.all.keys()) candidates.add(k);
        if (wantCallable) {
            for (const k of ['panic', 'raise']) candidates.add(k);
        }
        let best: string | undefined;
        let bestD = name.length <= 3 ? 1 : 2;
        const consider = (c: string): void => {
            if (c === name || Math.abs(c.length - name.length) > 2) return;
            const d = editDistance(name, c, bestD);
            if (d <= bestD && (best === undefined || d < bestD)) { best = c; bestD = d; }
        };
        for (const c of candidates) consider(c);
        if (best === undefined && wantCallable) {
            // Builtins only for calls, and only as a last resort.
            for (const c of BUILTIN_NAME_LIST) consider(c);
        }
        return best ? " Did you mean '" + best + "'?" : '';
    }

    // ---- Afterwards -----------------------------------------------------

    private finish(): void {
        const f = this.f;
        for (const sym of f.symbols) {
            if (sym.reads > 0) continue;
            if (sym.name.startsWith('_')) continue;
            if (sym.kind === 'var' && !sym.global) {
                f.diagnostics.push({
                    severity: 'hint', code: 'unused', unnecessary: true,
                    message: "'" + sym.name + "' is declared but never read.",
                    start: sym.start, end: sym.end
                });
            } else if (sym.kind === 'proc' && !sym.global) {
                f.diagnostics.push({
                    severity: 'hint', code: 'unused', unnecessary: true,
                    message: "The nested proc '" + sym.name + "' is never used.",
                    start: sym.start, end: sym.end
                });
            } else if (sym.kind === 'import') {
                f.diagnostics.push({
                    severity: 'hint', code: 'unused', unnecessary: true,
                    message: "The import '" + sym.name + "' is never used.",
                    start: sym.declStart, end: sym.declEnd
                });
            }
        }
        f.diagnostics.sort((a, b) => a.start - b.start);
    }
}

const BUILTIN_NAME_LIST: string[] = [];
export function registerBuiltinNames(names: string[]): void {
    BUILTIN_NAME_LIST.length = 0;
    for (const n of names) BUILTIN_NAME_LIST.push(n);
}

function editDistance(a: string, b: string, max: number): number {
    const m = a.length;
    const n = b.length;
    if (Math.abs(m - n) > max) return max + 1;
    let prev = new Array<number>(n + 1);
    let cur = new Array<number>(n + 1);
    for (let j = 0; j <= n; j++) prev[j] = j;
    for (let i = 1; i <= m; i++) {
        cur[0] = i;
        let rowMin = cur[0];
        for (let j = 1; j <= n; j++) {
            const cost = a.charCodeAt(i - 1) === b.charCodeAt(j - 1) ? 0 : 1;
            cur[j] = Math.min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + cost);
            if (i > 1 && j > 1 && a.charCodeAt(i - 1) === b.charCodeAt(j - 2) && a.charCodeAt(i - 2) === b.charCodeAt(j - 1)) {
                cur[j] = Math.min(cur[j], prev[j - 1]);
            }
            if (cur[j] < rowMin) rowMin = cur[j];
        }
        if (rowMin > max) return max + 1;
        [prev, cur] = [cur, prev];
    }
    return prev[n];
}

// ---- Doc comments -----------------------------------------------------------

/**
 * The comment written for a declaration: the `#` lines or the block
 * comment directly above it (no blank line between), or failing that a
 * comment at the end of its first line.
 */
export function docComment(f: FileAnalysis, node: Stmt): string | undefined {
    const { comments, lineStarts } = f.lex;
    if (comments.length === 0) return undefined;
    const text = f.text;
    const lineOf = (offset: number): number => {
        let lo = 0;
        let hi = lineStarts.length - 1;
        while (lo < hi) {
            const mid = (lo + hi + 1) >> 1;
            if (lineStarts[mid] <= offset) lo = mid; else hi = mid - 1;
        }
        return lo;
    };
    const ownLine = (c: Comment): boolean => text.slice(lineStarts[c.startLine], c.start).trim() === '';

    // Last comment that ends before the statement starts.
    let lo = 0;
    let hi = comments.length - 1;
    let idx = -1;
    while (lo <= hi) {
        const mid = (lo + hi) >> 1;
        if (comments[mid].end <= node.start) { idx = mid; lo = mid + 1; } else { hi = mid - 1; }
    }
    const stmtLine = lineOf(node.start);
    const picked: Comment[] = [];
    let expect = stmtLine - 1;
    for (let i = idx; i >= 0; i--) {
        const c = comments[i];
        if (c.endLine !== expect || !ownLine(c)) break;
        if (c.block) {
            // A block comment and the code after it on the same line do not count.
            if (picked.length === 0) picked.unshift(c);
            break;
        }
        picked.unshift(c);
        expect = c.startLine - 1;
    }
    if (picked.length > 0) {
        const cleaned = cleanDoc(picked.map((c) => text.slice(c.start, c.end)), picked[0].block);
        if (cleaned) return cleaned;
    }

    // `alloc limit := 10   # the most we will try`
    if (node.type === 'decl') {
        const line = lineOf(node.start);
        if (lineOf(node.end) === line) {
            for (let i = idx + 1; i < comments.length; i++) {
                const c = comments[i];
                if (c.startLine > line) break;
                if (c.startLine === line && c.start >= node.end && !c.block) {
                    return cleanDoc([text.slice(c.start, c.end)], false);
                }
            }
        }
    }
    return undefined;
}

function cleanDoc(parts: string[], block: boolean): string | undefined {
    let lines: string[];
    if (block) {
        let body = parts[0];
        body = body.replace(/^\/\*+/, '').replace(/\*+\/$/, '');
        lines = body.split(/\r?\n/).map((l) => l.replace(/^\s*\*(?!\/) ?/, ''));
        // A block written without leading stars keeps its own indentation.
        const indents = lines.filter((l) => l.trim() !== '').map((l) => /^\s*/.exec(l)![0].length);
        const cut = indents.length > 0 ? Math.min(...indents) : 0;
        lines = lines.map((l) => l.slice(Math.min(cut, l.length)));
    } else {
        lines = parts.map((l) => l.replace(/^#+ ?/, ''));
    }
    const kept: string[] = [];
    for (let raw of lines) {
        raw = raw.replace(/\s+$/, '');
        const t = raw.trim();
        // Rulers and section banners are decoration, not documentation.
        if (/^[=\-~*_#]{3,}$/.test(t)) continue;
        if (/^[=\-~]{2,}.*[=\-~]{2,}$/.test(t)) continue;
        let m = /^@param\s+(?:\{[^}]*\}\s*)?([A-Za-z_][A-Za-z0-9_]*)\s*-?\s*(.*)$/.exec(t);
        if (m) {
            kept.push('- `' + m[1] + '`' + (m[2] ? ': ' + m[2] : ''));
            continue;
        }
        m = /^@returns?\s+(?:\{[^}]*\}\s*)?(.*)$/.exec(t);
        if (m) {
            kept.push('', '**Returns** ' + m[1]);
            continue;
        }
        m = /^@(throws|raises)\s+(.*)$/.exec(t);
        if (m) {
            kept.push('', '**Raises** ' + m[2]);
            continue;
        }
        kept.push(raw);
    }
    while (kept.length > 0 && kept[0].trim() === '') kept.shift();
    while (kept.length > 0 && kept[kept.length - 1].trim() === '') kept.pop();
    if (kept.length === 0) return undefined;
    return kept.join('\n');
}
