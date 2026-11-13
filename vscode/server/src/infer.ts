// A small amount of static evaluation: enough to know that `m` is a
// module, that `make_stack()` returns a dict with `push` and `pop`, and
// that `add5` is the closure `add(x)`. It never guesses: when it cannot
// tell, it says nothing.

import { DictExpr, Expr, FuncStmt, Stmt, isBlock } from './ast';
import { FileAnalysis, Field, Occurrence, Sym, Target, classifyFree } from './analysis';
import {
    BuiltinDoc, GlobalDoc, RawOpDoc, ValueType, builtin, raiser, rawOp, reservedGlobal
} from './builtins';

/** What the workspace has to provide: the file behind an import. */
export interface ModuleSource {
    moduleOf(sym: Sym): FileAnalysis | undefined;
    /** In raw mode: a name defined in another file of the same unit. */
    unitLookup(file: FileAnalysis, name: string): Sym | undefined;
}

export type Value =
    | { k: 'proc'; sym: Sym }
    | { k: 'module'; file: FileAnalysis }
    | { k: 'dict'; fields: Map<string, Field>; frozen: boolean; open: boolean }
    | { k: 'type'; type: ValueType };

export type Resolved =
    | { k: 'sym'; sym: Sym }
    | { k: 'builtin'; doc: BuiltinDoc }
    | { k: 'raiser'; name: string; doc?: BuiltinDoc }
    | { k: 'reserved'; doc: GlobalDoc }
    | { k: 'raw'; name: string; doc?: RawOpDoc }
    | { k: 'field'; field: Field; frozen: boolean }
    | { k: 'missing-member'; container: Value; name: string };

const MAX_DEPTH = 10;

export class Inference {
    private readonly busy = new Set<Sym>();
    private readonly returnCache = new Map<Sym, Value | undefined>();

    constructor(private readonly src: ModuleSource) {}

    // ---- Names ----------------------------------------------------------

    resolveTarget(file: FileAnalysis, target: Target): Resolved | undefined {
        switch (target.t) {
            case 'sym':
                return { k: 'sym', sym: target.sym };
            case 'builtin': {
                const doc = builtin(target.name);
                return doc ? { k: 'builtin', doc } : undefined;
            }
            case 'raiser':
                return { k: 'raiser', name: target.name, doc: raiser(target.name) };
            case 'reserved': {
                const doc = reservedGlobal(target.name);
                return doc ? { k: 'reserved', doc } : undefined;
            }
            case 'raw': {
                const unit = this.src.unitLookup(file, target.name);
                if (unit) return { k: 'sym', sym: unit };
                return { k: 'raw', name: target.name, doc: rawOp(target.name) };
            }
            case 'key': {
                const pair = target.dict.pairs.find((p) => p.key.name === target.name);
                if (!pair) return undefined;
                return { k: 'field', field: { name: target.name, span: pair.key, value: pair.value, file }, frozen: false };
            }
            case 'member':
                return this.member(this.valueOf(file, target.obj, 0), target.name);
            case 'unresolved': {
                const unit = this.src.unitLookup(file, target.name);
                return unit ? { k: 'sym', sym: unit } : undefined;
            }
        }
    }

    resolve(file: FileAnalysis, occ: Occurrence): Resolved | undefined {
        return this.resolveTarget(file, occ.target);
    }

    /** `value.name`: a module's export or a dict's entry. */
    member(container: Value | undefined, name: string): Resolved | undefined {
        if (!container) return undefined;
        if (container.k === 'module') {
            const sym = container.file.exports.get(name);
            if (sym) return { k: 'sym', sym };
            return { k: 'missing-member', container, name };
        }
        if (container.k === 'dict') {
            const field = container.fields.get(name);
            if (field) return { k: 'field', field, frozen: container.frozen };
            return container.open ? undefined : { k: 'missing-member', container, name };
        }
        return undefined;
    }

    /**
     * Resolve a dotted chain typed so far (`a.b.c`), for places where the
     * code around the cursor does not parse.
     */
    chain(file: FileAnalysis, names: string[], offset: number): { resolved?: Resolved; value?: Value } {
        if (names.length === 0) return {};
        let resolved = this.resolveTarget(file, file.lookup(names[0], offset));
        let value = this.valueOfResolved(resolved);
        for (let i = 1; i < names.length; i++) {
            resolved = this.member(value, names[i]);
            value = this.valueOfResolved(resolved);
        }
        return { resolved, value };
    }

    // ---- Values ---------------------------------------------------------

    valueOfResolved(r: Resolved | undefined): Value | undefined {
        if (!r) return undefined;
        switch (r.k) {
            case 'sym':
                return this.valueOfSym(r.sym, 0);
            case 'field':
                return r.field.value ? this.valueOf(r.field.file, r.field.value, 0) : undefined;
            case 'reserved':
                return { k: 'type', type: r.doc.type };
            default:
                return undefined;
        }
    }

    valueOfSym(sym: Sym, depth: number): Value | undefined {
        if (depth > MAX_DEPTH || this.busy.has(sym)) return undefined;
        switch (sym.kind) {
            case 'proc':
                return { k: 'proc', sym };
            case 'import': {
                const file = this.src.moduleOf(sym);
                return file ? { k: 'module', file } : undefined;
            }
            case 'except':
                return { k: 'type', type: 'string' };
            case 'param':
                return undefined;
            case 'loopvar': {
                if (!sym.iter) return undefined;
                this.busy.add(sym);
                try {
                    const it = this.valueOf(sym.file, sym.iter, depth + 1);
                    if (it && it.k === 'type' && it.type === 'string') return { k: 'type', type: 'string' };
                    if (sym.iter.type === 'list' && sym.iter.elems.length > 0) {
                        const kinds = sym.iter.elems.map((e) => this.valueOf(sym.file, e, depth + 1));
                        const first = kinds[0];
                        if (first && first.k === 'type' && kinds.every((v) => v && v.k === 'type' && v.type === first.type)) return first;
                    }
                    return undefined;
                } finally {
                    this.busy.delete(sym);
                }
            }
            case 'var': {
                this.busy.add(sym);
                try {
                    let v = sym.init ? this.valueOf(sym.file, sym.init, depth + 1) : undefined;
                    if (sym.fields && sym.fields.size > 0) {
                        if (v && v.k === 'dict') {
                            const fields = new Map(v.fields);
                            for (const [k, fld] of sym.fields) if (!fields.has(k)) fields.set(k, fld);
                            v = { k: 'dict', fields, frozen: v.frozen, open: v.open };
                        } else if (!v) {
                            v = { k: 'dict', fields: new Map(sym.fields), frozen: false, open: true };
                        }
                    }
                    // A variable that is assigned again later may hold anything.
                    if (v && (v.k === 'type' || v.k === 'proc') && sym.writes > 0) return undefined;
                    return v;
                } finally {
                    this.busy.delete(sym);
                }
            }
        }
    }

    valueOf(file: FileAnalysis, e: Expr, depth: number): Value | undefined {
        if (depth > MAX_DEPTH) return undefined;
        switch (e.type) {
            case 'num':
                return { k: 'type', type: e.float ? 'float' : 'int' };
            case 'str':
                return { k: 'type', type: 'string' };
            case 'bool':
                return { k: 'type', type: 'bool' };
            case 'list':
                return { k: 'type', type: 'list' };
            case 'dict':
                return dictValue(file, e, false);
            case 'id': {
                const t = file.idTargets.get(e);
                if (!t) return undefined;
                if (t.t === 'sym') return this.valueOfSym(t.sym, depth + 1);
                if (t.t === 'reserved') {
                    const g = reservedGlobal(t.name);
                    return g ? { k: 'type', type: g.type } : undefined;
                }
                if (t.t === 'unresolved' || t.t === 'raw') {
                    const unit = this.src.unitLookup(file, t.name);
                    return unit ? this.valueOfSym(unit, depth + 1) : undefined;
                }
                return undefined;
            }
            case 'dot': {
                const r = this.member(this.valueOf(file, e.obj, depth + 1), e.attr.name);
                if (!r) return undefined;
                if (r.k === 'sym') return this.valueOfSym(r.sym, depth + 1);
                if (r.k === 'field' && r.field.value) return this.valueOf(r.field.file, r.field.value, depth + 1);
                return undefined;
            }
            case 'index': {
                if (e.index.type === 'str') {
                    const r = this.member(this.valueOf(file, e.obj, depth + 1), e.index.value);
                    if (r && r.k === 'sym') return this.valueOfSym(r.sym, depth + 1);
                    if (r && r.k === 'field' && r.field.value) return this.valueOf(r.field.file, r.field.value, depth + 1);
                    return undefined;
                }
                const o = this.valueOf(file, e.obj, depth + 1);
                if (o && o.k === 'type' && o.type === 'string') return { k: 'type', type: 'string' };
                return undefined;
            }
            case 'slice': {
                const o = this.valueOf(file, e.obj, depth + 1);
                if (o && o.k === 'type' && (o.type === 'string' || o.type === 'list')) return o;
                return undefined;
            }
            case 'unary': {
                if (e.op === 'not') return { k: 'type', type: 'bool' };
                if (e.op === 'bnot') return { k: 'type', type: 'int' };
                const o = this.valueOf(file, e.operand, depth + 1);
                return o && o.k === 'type' && (o.type === 'int' || o.type === 'float') ? o : undefined;
            }
            case 'bin':
                return this.binary(file, e.op, e.left, e.right, depth);
            case 'call': {
                if (e.callee.type === 'id') {
                    const t = file.idTargets.get(e.callee);
                    if (t && t.t === 'builtin') {
                        if (t.name === 'freeze' && e.args.length === 1) {
                            const inner = this.valueOf(file, e.args[0], depth + 1);
                            if (inner && inner.k === 'dict') return { ...inner, frozen: true };
                            return { k: 'type', type: 'frozen dict' };
                        }
                        const doc = builtin(t.name);
                        if (doc && doc.returnType && doc.returnType !== 'any' && doc.returnType !== 'never' && doc.returnType !== 'none') {
                            if (t.name === 'abs' && e.args.length === 1) {
                                const a = this.valueOf(file, e.args[0], depth + 1);
                                return a && a.k === 'type' && (a.type === 'int' || a.type === 'float') ? a : undefined;
                            }
                            if (doc.returnType === 'number') return undefined;
                            return { k: 'type', type: doc.returnType };
                        }
                        return undefined;
                    }
                }
                const callee = this.valueOf(file, e.callee, depth + 1);
                if (callee && callee.k === 'proc') return this.returnOf(callee.sym, depth + 1);
                return undefined;
            }
        }
    }

    private binary(file: FileAnalysis, op: string, l: Expr, r: Expr, depth: number): Value | undefined {
        switch (op) {
            case 'and': case 'or':
            case 'eq': case 'ne': case 'lt': case 'le': case 'gt': case 'ge':
                return { k: 'type', type: 'bool' };
            case 'div':
                return { k: 'type', type: 'float' };
            case 'band': case 'bor': case 'bxor': case 'shl': case 'shr':
                return { k: 'type', type: 'int' };
        }
        const a = this.valueOf(file, l, depth + 1);
        const b = this.valueOf(file, r, depth + 1);
        const ta = a && a.k === 'type' ? a.type : undefined;
        const tb = b && b.k === 'type' ? b.type : undefined;
        if (op === 'add') {
            if (ta === 'string' || tb === 'string') return { k: 'type', type: 'string' };
            if (ta === 'list' && tb === 'list') return { k: 'type', type: 'list' };
        }
        if (ta === 'int' && tb === 'int') return { k: 'type', type: 'int' };
        if ((ta === 'float' && (tb === 'int' || tb === 'float')) || (tb === 'float' && ta === 'int')) {
            return { k: 'type', type: 'float' };
        }
        return undefined;
    }

    /** What a proc hands back, when every `return` that says anything agrees. */
    returnOf(sym: Sym, depth: number): Value | undefined {
        if (this.returnCache.has(sym)) return this.returnCache.get(sym);
        if (depth > MAX_DEPTH || this.busy.has(sym) || !sym.node || sym.node.type !== 'func') return undefined;
        this.busy.add(sym);
        let result: Value | undefined;
        try {
            const returns: Expr[] = [];
            collectReturns((sym.node as FuncStmt).body.stmts, returns);
            let agreed = true;
            for (const e of returns) {
                const v = this.valueOf(sym.file, e, depth + 1);
                if (!v) { agreed = false; continue; }
                if (!result) {
                    result = v;
                } else if (!sameShape(result, v)) {
                    agreed = false;
                }
            }
            // A proc that sometimes returns a plain value we could not
            // name is still worth describing when the rest is an object.
            if (!agreed && result && result.k === 'type') result = undefined;
        } finally {
            this.busy.delete(sym);
        }
        this.returnCache.set(sym, result);
        return result;
    }
}

function sameShape(a: Value, b: Value): boolean {
    if (a.k !== b.k) return false;
    if (a.k === 'type' && b.k === 'type') return a.type === b.type;
    if (a.k === 'proc' && b.k === 'proc') return a.sym === b.sym;
    if (a.k === 'module' && b.k === 'module') return a.file === b.file;
    return true;
}

function dictValue(file: FileAnalysis, e: DictExpr, frozen: boolean): Value {
    const fields = new Map<string, Field>();
    for (const p of e.pairs) {
        if (!fields.has(p.key.name)) fields.set(p.key.name, { name: p.key.name, span: p.key, value: p.value, file });
    }
    return { k: 'dict', fields, frozen, open: false };
}

function collectReturns(stmts: Stmt[], out: Expr[]): void {
    for (const s of stmts) {
        switch (s.type) {
            case 'return':
                out.push(s.expr);
                break;
            case 'if':
                collectReturns(s.then.stmts, out);
                for (let e = s.otherwise; e; ) {
                    if (isBlock(e)) { collectReturns(e.stmts, out); break; }
                    collectReturns(e.then.stmts, out);
                    e = e.otherwise;
                }
                break;
            case 'loop':
            case 'for':
                collectReturns(s.body.stmts, out);
                break;
            case 'try':
                collectReturns(s.body.stmts, out);
                collectReturns(s.handler.stmts, out);
                break;
            default:
                break;
        }
    }
}

/** A short name for a value, for hovers: "int", "dict", "module". */
export function describeValue(v: Value | undefined): string | undefined {
    if (!v) return undefined;
    switch (v.k) {
        case 'type':
            return v.type;
        case 'proc':
            return 'proc';
        case 'module':
            return 'module';
        case 'dict':
            return v.frozen ? 'frozen dict' : 'dict';
    }
}

export { classifyFree };
