// AST of an OMG program, as the language server sees it. The shapes
// follow the tagged lists `compiler.omg` builds, with source offsets
// added to every node and every name.

export interface Span {
    start: number;
    end: number;
}

export interface Name extends Span {
    name: string;
}

export type Expr =
    | NumExpr | StrExpr | BoolExpr | IdExpr | ListExpr | DictExpr
    | IndexExpr | SliceExpr | DotExpr | CallExpr | UnaryExpr | BinExpr;

export interface NumExpr extends Span { type: 'num'; float: boolean; text: string; value?: bigint }
export interface StrExpr extends Span { type: 'str'; value: string }
export interface BoolExpr extends Span { type: 'bool'; value: boolean }
export interface IdExpr extends Span { type: 'id'; name: string }
export interface ListExpr extends Span { type: 'list'; elems: Expr[] }
export interface DictPair { key: Name; quoted: boolean; value: Expr }
export interface DictExpr extends Span { type: 'dict'; pairs: DictPair[] }
export interface IndexExpr extends Span { type: 'index'; obj: Expr; index: Expr }
export interface SliceExpr extends Span { type: 'slice'; obj: Expr; from: Expr; to: Expr | null }
export interface DotExpr extends Span { type: 'dot'; obj: Expr; attr: Name }
export interface CallExpr extends Span {
    type: 'call';
    callee: Expr;
    args: Expr[];
    lparen: number;
    /** Offset just past the `)`. */
    rparen: number;
    /** Compiler line of the `(`. */
    line: number;
}
export interface UnaryExpr extends Span { type: 'unary'; op: 'not' | 'neg' | 'plus' | 'bnot'; operand: Expr }
export interface BinExpr extends Span { type: 'bin'; op: string; left: Expr; right: Expr }

export type Stmt =
    | DeclStmt | AssignStmt | AttrAssignStmt | IndexAssignStmt | ExprStmt
    | EmitStmt | FactsStmt | ImportStmt | IfStmt | LoopStmt | ForStmt
    | BreakStmt | ContinueStmt | FuncStmt | ReturnStmt | TryStmt;

export interface StmtBase extends Span {
    /** Compiler line of the statement's first token. */
    line: number;
}

export interface Block extends Span {
    stmts: Stmt[];
    /** False when the closing `}` was never found. */
    closed: boolean;
}

export interface DeclStmt extends StmtBase { type: 'decl'; name: Name; init: Expr }
export interface AssignStmt extends StmtBase { type: 'assign'; name: Name; value: Expr }
export interface AttrAssignStmt extends StmtBase { type: 'attr_assign'; obj: Expr; attr: Name; value: Expr }
export interface IndexAssignStmt extends StmtBase { type: 'index_assign'; obj: Expr; index: Expr; value: Expr }
export interface ExprStmt extends StmtBase { type: 'expr'; expr: Expr }
export interface EmitStmt extends StmtBase { type: 'emit'; expr: Expr }
export interface FactsStmt extends StmtBase { type: 'facts'; expr: Expr }
export interface ImportStmt extends StmtBase { type: 'import'; path: string; pathSpan: Span; alias: Name }
export interface IfStmt extends StmtBase {
    type: 'if';
    cond: Expr;
    then: Block;
    /** An `elif` is an IfStmt here; a plain `else` is a Block. */
    otherwise: IfStmt | Block | null;
    isElif: boolean;
}
export interface LoopStmt extends StmtBase { type: 'loop'; cond: Expr; body: Block }
export interface ForStmt extends StmtBase { type: 'for'; name: Name; iter: Expr; body: Block }
export interface BreakStmt extends StmtBase { type: 'break' }
export interface ContinueStmt extends StmtBase { type: 'continue' }
export interface FuncStmt extends StmtBase { type: 'func'; name: Name; params: Name[]; body: Block; headEnd: number }
export interface ReturnStmt extends StmtBase { type: 'return'; expr: Expr }
export interface TryStmt extends StmtBase { type: 'try'; body: Block; excName: Name | null; handler: Block }

export function isBlock(n: IfStmt | Block): n is Block {
    return (n as Block).stmts !== undefined;
}
