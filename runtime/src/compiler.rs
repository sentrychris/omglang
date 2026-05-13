//! # OMGlang Bytecode Compiler
//!
//! Lowers an [`crate::ast::Node`] tree into a flat instruction stream + a
//! function table compatible with the existing VM.
//!
//! Equivalent to `omglang/compiler.py` from the reference implementation,
//! plus first-class **native import** support: instead of refusing files
//! with `import`, this compiler recursively compiles imported modules,
//! mangles their function names so they don't collide, and emits inline
//! initialisation that builds a frozen-namespace dict for each module.
//!
//! There is no longer a need for the OMG-implemented `bootstrap/src/interpreter.omg`
//! — running a `.omg` file goes straight through this compiler in-process.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::mem;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::ast::{BinOp, Node, UnaryOp};
use crate::bytecode::{Function, Instr, SourceMap};
use crate::error::{ErrorKind, RuntimeError};
use crate::lexer::tokenize;
use crate::parser::Parser;

/// Built-in function names known to the compiler. Calls to these become
/// `CallBuiltin` instructions instead of `Call`. Keep in sync with
/// `runtime::vm::builtins::call_builtin`.
fn builtin_names() -> &'static [&'static str] {
    &[
        "chr",
        "ascii",
        "hex",
        "binary",
        "length",
        "read_file",
        "freeze",
        "call_builtin",
        "file_open",
        "file_read",
        "file_write",
        "file_close",
        // Random-access file I/O. Used by tools/db (omgdb) for paged
        // storage; pairs with file_open's `rb+` / `wb+` modes.
        "file_seek",
        "file_tell",
        "file_exists",
        "is_dir",
        "read_dir",
        "make_dir",
        "string_bytes",
        // Numeric / math
        "int",
        "float",
        "floor",
        "ceil",
        "round",
        "abs",
        "sqrt",
        "pow",
        "log",
        "sin",
        "cos",
        "tan",
        // Used by `bootstrap/src/compiler.omg` to embed float literals as i64 bits.
        "float_bits",
        // Inverse pair, used by `bootstrap/src/vm.omg` to read float
        // and string literals back out of a `.omgb` byte stream.
        "bits_to_float",
        "bytes_to_string",
        // Dict-keys enumeration, used by the OMG-in-OMG VM to iterate a
        // closure's captured environment (and useful generally).
        "dict_keys",
        // list_repeat(item, count) — pre-allocate a list of `count`
        // copies of `item`. Lets pure-OMG code build byte vectors at
        // amortised O(1) per push via doubling; without it
        // `xs + [v]` is O(n) and `n` appends are O(n²).
        "list_repeat",
        // Print msg to stderr verbatim and exit 1. Used by `bootstrap/src/vm.omg`
        // to surface a hosted program's uncaught error without re-wrapping
        // it through `panic`'s "RuntimeError:" prefix.
        "exit_with_error",
        // Process control. Used by the OMG-native `omg` driver to
        // shell out to `cc` during `--build` and to propagate child
        // exit codes back to its own caller.
        "exit",
        "getpid",
        "subprocess",
        // I/O primitives. stdin_readline + print are used by the
        // OMG-native REPL; stdin_read[_bytes] make tools pipe-friendly
        // by slurping all of stdin to EOF (text or bytes).
        "stdin_readline",
        "stdin_read",
        "stdin_read_bytes",
        "print",
        // TCP networking. Six builtins mirror the file_* shape (open,
        // accept→stream, read, write, close). Bytes-in / bytes-out;
        // use bytes_to_string / string_bytes at the OMG layer for text.
        "tcp_listen",
        "tcp_accept",
        "tcp_connect",
        "tcp_read",
        "tcp_write",
        "tcp_close",
        // POSIX fork() for process-per-request concurrency. Returns 0
        // in child, child pid in parent. SIGCHLD is set to SIG_IGN so
        // children auto-reap.
        "fork",
    ]
}

/// Names that lower to a `Raise(kind)` instruction. These mirror the helper
/// names used by the original bootstrap interpreter and the Python
/// reference compiler.
fn raise_helpers() -> &'static [(&'static str, ErrorKind)] {
    &[
        ("panic", ErrorKind::Generic),
        ("raise", ErrorKind::Generic),
        ("_omg_vm_syntax_error_handle", ErrorKind::Syntax),
        ("_omg_vm_type_error_handle", ErrorKind::Type),
        (
            "_omg_vm_undef_ident_error_handle",
            ErrorKind::UndefinedIdent,
        ),
        ("_omg_vm_value_error_handle", ErrorKind::Value),
        (
            "_omg_vm_module_import_error_handle",
            ErrorKind::ModuleImport,
        ),
    ]
}

fn lookup_raise(name: &str) -> Option<ErrorKind> {
    raise_helpers()
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, k)| *k)
}

fn is_builtin(name: &str) -> bool {
    builtin_names().contains(&name)
}

/// One compiled program: instruction stream + function table + source map.
pub struct Program {
    pub code: Vec<Instr>,
    pub funcs: HashMap<String, Function>,
    pub src_map: SourceMap,
}

/// One pending function body waiting to be flushed after the main code.
/// `lines` is parallel to `code`; both get rebased and appended together
/// in `compile_program`.
struct PendingFunc {
    name: String,
    params: Rc<Vec<String>>,
    code: Vec<Instr>,
    lines: Vec<(u32, u32)>,
    source_file_idx: u32,
}

/// Compiler context. A single instance compiles the entry-point file plus
/// any modules it imports, sharing one global instruction stream and one
/// function table across them.
pub struct Compiler {
    code: Vec<Instr>,
    /// Parallel to `code`: per-instruction `(file_idx, line)`. Updated
    /// by every call to `emit` / `placeholder` from `current_file_idx`
    /// and `current_line`. The compile_stmt/compile_expr entry-points
    /// refresh `current_line` from the AST node before any emit fires.
    lines: Vec<(u32, u32)>,
    /// All source files ever loaded by this compile, in the order they
    /// were first seen. Entry-point is always index 0.
    src_files: Vec<String>,
    /// Path-to-index dedupe for `src_files`. Keys are
    /// `Path::display().to_string()` so they match what we store in
    /// `src_files` and what we'd derive from `current_file`.
    file_idx_of: HashMap<String, u32>,
    /// Index into `src_files` for the file currently being compiled.
    /// Swapped when entering / leaving an imported module body.
    current_file_idx: u32,
    /// Line of the AST node currently being compiled. The bottom of
    /// every recursive call into `compile_stmt`/`compile_expr` refreshes
    /// this so all subsequent `emit`s tag their instruction with the
    /// originating source line.
    current_line: u32,
    pending_funcs: Vec<PendingFunc>,
    funcs: HashMap<String, Function>,
    break_stack: Vec<Vec<usize>>,
    /// How many `SETUP_EXCEPT` blocks are currently open lexically. Used
    /// so `return` / `break` / tail calls can emit a matching number of
    /// `POP_BLOCK` instructions before transferring control — otherwise
    /// the runtime block-stack accumulates stale handlers, which a
    /// later `RAISE` in unrelated code would unwind into.
    try_depth: usize,
    /// `try_depth` value at the start of each currently-open loop, in
    /// parallel with `break_stack`. `break` uses this to compute how
    /// many `PopBlock`s are needed to escape any tries nested between
    /// the break statement and its target loop.
    loop_try_depth: Vec<usize>,
    /// Modules already loaded during this compile, keyed by canonical path.
    loaded_modules: HashMap<PathBuf, ModuleInfo>,
    /// Stack of paths currently being loaded — for cycle detection.
    loading_stack: HashSet<PathBuf>,
    /// Counter used to generate unique mangling prefixes.
    module_counter: usize,
    /// Active mangling prefix for the file currently being compiled. Empty
    /// string for the entry-point file.
    current_prefix: String,
    /// Current source-file path, used in error messages.
    current_file: PathBuf,
    /// Stack of name sets that are *locally* bound (parameters, allocs,
    /// import aliases, captured closures). Used to decide whether
    /// `f(x)` should compile to a direct `Call` or to `Load + CallValue`.
    /// The bottom-most frame represents the file's top-level locals.
    local_scopes: Vec<HashSet<String>>,
    /// **Mangled** names that have been alloc'd at top level. Used purely
    /// for the duplicate-`alloc` check; we can't reuse `local_scopes[0]`
    /// because that holds *unmangled* names (so two imported modules
    /// with `alloc i := ...` would falsely collide on the bare name).
    top_level_declared: HashSet<String>,
    /// Mangled top-level proc name → parameter count. Populated by a
    /// pre-pass before compiling any body, so direct `Call name` emits
    /// can verify that the call site's arg count matches the callee's
    /// declared arity. Without this check, the compiler would emit
    /// unbalanced bytecode (e.g. 2 pushes for a 0-param callee), the
    /// runtime's `Call` handler would pop only the declared count, and
    /// the extra operands would leak onto the stack as "ghost" values
    /// — silently consumed by later instructions, with weird and
    /// hard-to-trace consequences.
    proc_arity: HashMap<String, usize>,
}

#[derive(Clone)]
struct ModuleInfo {
    /// Mangling prefix used for this module (e.g. "__mod_3__").
    prefix: String,
    /// Names that are exported (top-level alloc + proc names, *unprefixed*).
    exports: Vec<(String, ExportKind)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExportKind {
    Func,
    Value,
}

impl Compiler {
    pub fn new(entry_file: impl Into<PathBuf>) -> Self {
        let entry = entry_file.into();
        let entry_display = entry.display().to_string();
        let mut file_idx_of = HashMap::new();
        file_idx_of.insert(entry_display.clone(), 0);
        Self {
            code: Vec::new(),
            lines: Vec::new(),
            src_files: vec![entry_display],
            file_idx_of,
            current_file_idx: 0,
            current_line: 0,
            pending_funcs: Vec::new(),
            funcs: HashMap::new(),
            break_stack: Vec::new(),
            try_depth: 0,
            loop_try_depth: Vec::new(),
            loaded_modules: HashMap::new(),
            loading_stack: HashSet::new(),
            module_counter: 0,
            current_prefix: String::new(),
            current_file: entry,
            local_scopes: vec![HashSet::new()],
            top_level_declared: HashSet::new(),
            proc_arity: HashMap::new(),
        }
    }

    /// Pre-pass: walk an AST's top-level statements and record every
    /// `proc` name's parameter count into `proc_arity`, mangled with
    /// `current_prefix`. Run before compiling the bodies so direct-
    /// `Call` sites can verify their argument count even on forward
    /// references (the standard case where a caller appears above its
    /// callee in source order).
    fn collect_proc_arity(&mut self, ast: &[Node]) {
        for stmt in ast {
            if let Node::FuncDef(name, params, _, _) = stmt {
                let mangled = self.mangle(name);
                self.proc_arity.insert(mangled, params.len());
            }
        }
    }

    /// Reject a direct `Call`/`TailCall` whose argument count doesn't match
    /// the callee's declared parameter count. Catches the operand-stack
    /// leak that arises when the compiler emits N pushes but the runtime
    /// `Call` handler pops only `func.params.len()` values — leftover
    /// arguments would sit on the operand stack and silently corrupt
    /// later instructions. Indirect (`CallValue`) and builtin calls are
    /// unaffected; their arities are known only at runtime / inside the
    /// builtin itself.
    fn check_direct_call_arity(
        &self,
        bare_name: &str,
        resolved: &str,
        got: usize,
    ) -> Result<(), RuntimeError> {
        if let Some(&expected) = self.proc_arity.get(resolved) {
            if got != expected {
                return Err(RuntimeError::SyntaxError(format!(
                    "function '{}' expects {} argument{}, got {} on line {} in {}",
                    bare_name,
                    expected,
                    if expected == 1 { "" } else { "s" },
                    got,
                    self.current_line,
                    self.current_file.display()
                )));
            }
        }
        Ok(())
    }

    /// Intern a source-file path into `src_files` and return its index.
    fn intern_file(&mut self, path: &Path) -> u32 {
        let key = path.display().to_string();
        if let Some(&idx) = self.file_idx_of.get(&key) {
            return idx;
        }
        let idx = self.src_files.len() as u32;
        self.src_files.push(key.clone());
        self.file_idx_of.insert(key, idx);
        idx
    }

    /// Refresh `current_line` from an AST node before its compilation
    /// emits any bytecode. Compiler-synthesised instructions (e.g. the
    /// implicit `PushNone+Ret` after a function body) inherit whatever
    /// the last refresh set, which is the closest enclosing source.
    fn enter_node(&mut self, node: &Node) {
        self.current_line = node.line() as u32;
    }

    pub fn declare_local(&mut self, name: &str) {
        if let Some(scope) = self.local_scopes.last_mut() {
            scope.insert(name.to_string());
        }
    }

    /// True when `name` is bound inside a function body (parameter, alloc,
    /// nested proc, except-binding). The bottom-most scope tracks
    /// *top-level* bindings, which are globals and so deliberately excluded.
    fn in_function_scope(&self, name: &str) -> bool {
        if self.local_scopes.len() <= 1 {
            return false;
        }
        self.local_scopes
            .iter()
            .skip(1)
            .any(|scope| scope.contains(name))
    }

    /// True when `name` has been *declared* anywhere in the current
    /// compilation, including the top-level scope. Used to decide whether
    /// a `name(args)` form should compile to a direct `Call` (top-level
    /// proc, fast path) or to `Load + CallValue` (any value binding,
    /// because the value may be a closure — e.g. `alloc add5 := make_adder(5)`).
    fn is_value_binding(&self, name: &str) -> bool {
        self.local_scopes.iter().any(|scope| scope.contains(name))
    }

    /// Reserved names auto-injected by the runtime as globals — they must
    /// never be prefixed by a module mangling, otherwise imports would lose
    /// access to them.
    fn is_reserved_global(name: &str) -> bool {
        matches!(name, "args" | "module_file" | "current_dir")
    }

    /// Resolve a name for a *load* (Ident, Call, etc.). Locals stay as-is;
    /// globals get the active module prefix unless they are reserved.
    fn resolve_load(&self, name: &str) -> String {
        if self.in_function_scope(name) {
            name.to_string()
        } else if Self::is_reserved_global(name) {
            name.to_string()
        } else {
            self.mangle(name)
        }
    }

    /// Resolve a name for a *store*.
    ///
    /// Reserved globals (`args`, `module_file`, `current_dir`) always pass
    /// through unprefixed. Otherwise:
    ///
    /// - Inside a function body, names declared locally (parameters / inner
    ///   `alloc` / except-bindings / nested procs) are stored unprefixed,
    ///   matching the runtime's local-env semantics.
    /// - Inside a function body, names *not* declared locally are treated
    ///   as references to the surrounding module's globals — those get
    ///   mangled so writes from imported modules land on the right slot.
    /// - At top-level, regular mangling applies (so imported modules don't
    ///   collide on top-level names).
    fn resolve_store(&self, name: &str) -> String {
        if Self::is_reserved_global(name) {
            return name.to_string();
        }
        if self.local_scopes.len() > 1 {
            if self.in_function_scope(name) {
                return name.to_string();
            }
            return self.mangle(name);
        }
        self.mangle(name)
    }

    /// Compile a complete program (entry-point AST already parsed) into a
    /// final [`Program`].
    pub fn compile_program(mut self, ast: Vec<Node>) -> Result<Program, RuntimeError> {
        self.collect_proc_arity(&ast);
        for stmt in ast {
            self.compile_stmt(&stmt)?;
        }
        self.emit(Instr::Halt);
        // Flush pending function bodies after the main code so PC layout is:
        //   [main code...HALT][func1 body][func2 body]...
        let mut final_code = self.code;
        let mut final_lines = self.lines;
        let pending = std::mem::take(&mut self.pending_funcs);
        for pf in pending {
            let addr = final_code.len();
            self.funcs.insert(
                pf.name,
                Function {
                    params: pf.params.as_ref().clone(),
                    address: addr,
                    source_file_idx: pf.source_file_idx,
                },
            );
            for instr in pf.code {
                final_code.push(rebase_jump(instr, addr));
            }
            final_lines.extend(pf.lines);
        }
        debug_assert_eq!(
            final_code.len(),
            final_lines.len(),
            "source map must be parallel to code"
        );
        Ok(Program {
            code: final_code,
            funcs: self.funcs,
            src_map: SourceMap {
                files: self.src_files,
                lines: final_lines,
            },
        })
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn emit(&mut self, instr: Instr) {
        self.code.push(instr);
        self.lines.push((self.current_file_idx, self.current_line));
    }

    /// Emit `n` `PopBlock` instructions. Used by `return` / `break` /
    /// tail-call sites to drain any open `SETUP_EXCEPT` blocks before
    /// transferring control out of their lexical scope, so a later
    /// `RAISE` doesn't unwind into a stale handler.
    fn emit_pop_blocks(&mut self, n: usize) {
        for _ in 0..n {
            self.emit(Instr::PopBlock);
        }
    }

    fn placeholder(&mut self, instr: Instr) -> usize {
        let idx = self.code.len();
        self.code.push(instr);
        self.lines.push((self.current_file_idx, self.current_line));
        idx
    }

    fn patch_jump(&mut self, idx: usize, target: usize) {
        self.code[idx] = match &self.code[idx] {
            Instr::Jump(_) => Instr::Jump(target),
            Instr::JumpIfFalse(_) => Instr::JumpIfFalse(target),
            Instr::SetupExcept(_) => Instr::SetupExcept(target),
            other => panic!("patch_jump on non-jump instr: {:?}", DebugInstr(other)),
        };
    }

    /// Apply the active module prefix to a top-level binding name.
    fn mangle(&self, name: &str) -> String {
        if self.current_prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}{}", self.current_prefix, name)
        }
    }

    // ------------------------------------------------------------------
    // Statements
    // ------------------------------------------------------------------

    fn compile_stmt(&mut self, stmt: &Node) -> Result<(), RuntimeError> {
        self.enter_node(stmt);
        match stmt {
            Node::Emit(expr, _) => {
                self.compile_expr(expr)?;
                self.emit(Instr::Emit);
            }
            Node::Decl(name, expr, line) => {
                // Top-level same-scope re-declaration is a compile-time
                // error: it's almost always a typo or accidental
                // copy/paste. We track top-level allocs by their
                // *mangled* name so two imported modules with `alloc i`
                // don't falsely collide. Inside a proc we don't enforce
                // this yet — OMG is proc-scoped (no block scoping), so
                // the "alloc-per-branch" idiom in parser-style code
                // would be forced into ugly hoists. Block scoping is the
                // right way to extend the rule into procs.
                if self.local_scopes.len() == 1 {
                    let mangled = self.resolve_store(name);
                    if !self.top_level_declared.insert(mangled) {
                        return Err(RuntimeError::SyntaxError(format!(
                            "'{}' is already declared at the top level on line {} in {}",
                            name,
                            line,
                            self.current_file.display()
                        )));
                    }
                }
                self.compile_expr(expr)?;
                // Declare *before* resolving the storage name so
                // `resolve_store` sees the new local in scope and emits the
                // unmangled name. (The expression has already been compiled
                // with the old scope state, so `alloc x := f(x)` still
                // resolves the RHS `x` against the surrounding scope.)
                self.declare_local(name);
                self.emit(Instr::StoreLocal(self.resolve_store(name)));
            }
            Node::Assign(name, expr, _) => {
                self.compile_expr(expr)?;
                self.emit(Instr::Store(self.resolve_store(name)));
            }
            Node::AttrAssign(target, attr, value, _) => {
                self.compile_expr(target)?;
                self.compile_expr(value)?;
                self.emit(Instr::StoreAttr(attr.clone()));
            }
            Node::IndexAssign(target, idx, value, _) => {
                self.compile_expr(target)?;
                self.compile_expr(idx)?;
                self.compile_expr(value)?;
                self.emit(Instr::StoreIndex);
            }
            Node::ExprStmt(expr, _) => {
                self.compile_expr(expr)?;
                self.emit(Instr::Pop);
            }
            Node::Import(path, alias, line) => {
                self.compile_import(path, alias, *line)?;
            }
            Node::Facts(expr, _) => {
                self.compile_expr(expr)?;
                self.emit(Instr::Assert);
            }
            Node::If(cond, then_block, tail, _) => {
                // Unroll nested elif chain to share the same end-jump array.
                let mut cases: Vec<(&Node, &Node)> = Vec::new();
                let mut else_block: Option<&Node> = None;
                let mut current_cond: &Node = cond;
                let mut current_then: &Node = then_block;
                let mut current_tail: &Option<Box<Node>> = tail;
                loop {
                    cases.push((current_cond, current_then));
                    match current_tail.as_deref() {
                        Some(Node::If(c, t, nt, _)) => {
                            current_cond = c;
                            current_then = t;
                            current_tail = nt;
                        }
                        Some(b @ Node::Block(_, _)) => {
                            else_block = Some(b);
                            break;
                        }
                        Some(other) => {
                            else_block = Some(other);
                            break;
                        }
                        None => break,
                    }
                }
                let mut end_jumps: Vec<usize> = Vec::new();
                for (c, b) in cases {
                    self.compile_expr(c)?;
                    let jf = self.placeholder(Instr::JumpIfFalse(0));
                    self.compile_block_node(b)?;
                    end_jumps.push(self.placeholder(Instr::Jump(0)));
                    let here = self.code.len();
                    self.patch_jump(jf, here);
                }
                if let Some(eb) = else_block {
                    self.compile_block_node(eb)?;
                }
                let here = self.code.len();
                for j in end_jumps {
                    self.patch_jump(j, here);
                }
            }
            Node::Loop(cond, body, _) => {
                let start = self.code.len();
                self.compile_expr(cond)?;
                let jf = self.placeholder(Instr::JumpIfFalse(0));
                self.break_stack.push(Vec::new());
                self.loop_try_depth.push(self.try_depth);
                self.compile_block_node(body)?;
                self.emit(Instr::Jump(start));
                let here = self.code.len();
                self.patch_jump(jf, here);
                let breaks = self.break_stack.pop().unwrap();
                self.loop_try_depth.pop();
                for b in breaks {
                    self.patch_jump(b, here);
                }
            }
            Node::Try(try_block, exc_name, except_block, _) => {
                let handler_idx = self.placeholder(Instr::SetupExcept(0));
                // Track the open SETUP_EXCEPT block so any `return` /
                // `break` inside the body emits a matching POP_BLOCK
                // before transferring control. The except body runs
                // *after* the runtime has unwound the block, so it
                // sees the original try_depth — adjust accordingly.
                self.try_depth += 1;
                self.compile_block_node(try_block)?;
                self.try_depth -= 1;
                self.emit(Instr::PopBlock);
                let end_jump = self.placeholder(Instr::Jump(0));
                let handler_pc = self.code.len();
                self.patch_jump(handler_idx, handler_pc);
                if let Some(name) = exc_name {
                    self.declare_local(name);
                    self.emit(Instr::StoreLocal(self.resolve_store(name)));
                } else {
                    self.emit(Instr::Pop);
                }
                self.compile_block_node(except_block)?;
                let end_pc = self.code.len();
                self.patch_jump(end_jump, end_pc);
            }
            Node::FuncDef(name, params, body, _) => {
                let (body_code, body_lines) = self.compile_function_body(params, body)?;
                let mangled = self.mangle(name);
                let source_file_idx = self.current_file_idx;
                self.pending_funcs.push(PendingFunc {
                    name: mangled.clone(),
                    params: params.clone(),
                    code: body_code,
                    lines: body_lines,
                    source_file_idx,
                });
                // Bind the function as a first-class value at the definition
                // site. At top level this stores `Closure { mangled, ∅ }` to
                // globals; inside a function body it captures the current
                // local environment so nested procs become real closures.
                self.emit(Instr::MakeFunc(mangled));
                // Inside another function, the proc lives in the local env,
                // so further references resolve via Load + CallValue. At
                // top level we leave it out of the local-scope tracker so
                // calls of the form `foo()` compile to fast direct `Call`s.
                if self.local_scopes.len() > 1 {
                    self.declare_local(name);
                }
            }
            Node::Return(expr, _) => {
                if let Node::FuncCall(callee, args, _) = expr.as_ref() {
                    if let Node::Ident(name, _) = callee.as_ref() {
                        if let Some(kind) = lookup_raise(name) {
                            // raise / panic are not tail-callable; lower to RAISE.
                            self.compile_raise_call(kind, args)?;
                            return Ok(());
                        }
                        // Tail-call optimisation only applies to direct
                        // calls of known top-level procs. If the name is a
                        // value binding (parameter, alloc, nested proc),
                        // fall through to the generic `Load + CallValue +
                        // Ret` path below so closures resolve correctly.
                        if !is_builtin(name) && !self.is_value_binding(name) {
                            let resolved = self.resolve_call_name(name);
                            self.check_direct_call_arity(name, &resolved, args.len())?;
                            for a in args {
                                self.compile_expr(a)?;
                            }
                            self.emit_pop_blocks(self.try_depth);
                            self.emit(Instr::TailCall(resolved));
                            return Ok(());
                        }
                    }
                }
                self.compile_expr(expr)?;
                self.emit_pop_blocks(self.try_depth);
                self.emit(Instr::Ret);
            }
            Node::Break(_) => {
                if self.break_stack.is_empty() {
                    return Err(RuntimeError::SyntaxError(format!(
                        "'break' outside of loop in {}",
                        self.current_file.display()
                    )));
                }
                // Pop any try blocks opened *between* the enclosing loop
                // and this break. `loop_try_depth.last()` records the
                // try_depth when the loop started; the current
                // try_depth tells us how many tries are nested inside.
                let saved = *self.loop_try_depth.last().unwrap();
                self.emit_pop_blocks(self.try_depth - saved);
                let j = self.placeholder(Instr::Jump(0));
                self.break_stack.last_mut().unwrap().push(j);
            }
            Node::Block(stmts, _) => {
                for s in stmts {
                    self.compile_stmt(s)?;
                }
            }
            other => {
                return Err(RuntimeError::SyntaxError(format!(
                    "Unsupported statement at line {} in {}: {:?}",
                    other.line(),
                    self.current_file.display(),
                    DebugNode(other)
                )));
            }
        }
        Ok(())
    }

    fn compile_block_node(&mut self, node: &Node) -> Result<(), RuntimeError> {
        match node {
            Node::Block(stmts, _) => {
                for s in stmts {
                    self.compile_stmt(s)?;
                }
                Ok(())
            }
            other => self.compile_stmt(other),
        }
    }

    fn compile_function_body(
        &mut self,
        params: &Rc<Vec<String>>,
        body: &Node,
    ) -> Result<(Vec<Instr>, Vec<(u32, u32)>), RuntimeError> {
        let saved_code = mem::take(&mut self.code);
        let saved_lines = mem::take(&mut self.lines);
        // Function bodies start with empty break_stack so a stray `break`
        // produces a syntax error, matching the Python compiler.
        let saved_break = mem::take(&mut self.break_stack);
        // Each function tracks its own try-block depth, independent of
        // the enclosing scope's. (Outer try/except blocks don't carry
        // through a function call boundary — the unwinder pops to the
        // enclosing block's env_depth, which would be the call frame.)
        let saved_try_depth = self.try_depth;
        let saved_loop_try_depth = mem::take(&mut self.loop_try_depth);
        self.try_depth = 0;
        // Push a new local scope seeded with the parameter names. They are
        // bound by the VM at call time; the compiler only needs to know
        // which identifiers are local (parameters / declarations) to choose
        // between Call and CallValue for `name(args)` forms.
        let mut new_scope: HashSet<String> = HashSet::new();
        for p in params.iter() {
            new_scope.insert(p.clone());
        }
        self.local_scopes.push(new_scope);
        self.compile_block_node(body)?;
        // Implicit return for procs that fall off the end. Without the
        // PushNone, Ret would pop whatever happened to be on top of the
        // operand stack — including values the *caller* pushed before
        // the call (e.g. `xs + [void_proc()]` would have `Ret` consume
        // `xs`).
        self.emit(Instr::PushNone);
        self.emit(Instr::Ret);
        self.local_scopes.pop();
        let func_code = mem::replace(&mut self.code, saved_code);
        let func_lines = mem::replace(&mut self.lines, saved_lines);
        self.break_stack = saved_break;
        self.try_depth = saved_try_depth;
        self.loop_try_depth = saved_loop_try_depth;
        Ok((func_code, func_lines))
    }

    fn compile_raise_call(
        &mut self,
        kind: ErrorKind,
        args: &[Node],
    ) -> Result<(), RuntimeError> {
        if let Some(first) = args.first() {
            self.compile_expr(first)?;
        } else {
            self.emit(Instr::PushStr(String::new()));
        }
        self.emit(Instr::Raise(kind));
        Ok(())
    }

    /// When the entry file (or an imported module) calls a user-defined
    /// function by bare name, the compiler resolves it against the active
    /// mangling prefix.
    fn resolve_call_name(&self, name: &str) -> String {
        // Imported modules use their own mangling prefix while compiling, so
        // calling a sibling proc within the same module gets the prefix
        // applied.  Since we don't know at this point whether `name` is a
        // local proc or a top-level proc from the entry file, the convention
        // is: top-level procs in *every* file are prefixed with their module
        // prefix, and we always add the prefix when emitting CALL.  Builtins
        // are special-cased before reaching this function.
        format!("{}{}", self.current_prefix, name)
    }

    // ------------------------------------------------------------------
    // Imports
    // ------------------------------------------------------------------

    fn compile_import_alias_store(&mut self, alias: &str) {
        // `import ... as name` introduces a brand-new binding; treat it as
        // an alloc so it doesn't accidentally rebind a same-named global.
        // Declare before resolve_store, same as Decl, so the alias stays
        // unmangled inside any enclosing function body.
        self.declare_local(alias);
        self.emit(Instr::StoreLocal(self.resolve_store(alias)));
    }

    fn compile_import(
        &mut self,
        path: &str,
        alias: &str,
        line: usize,
    ) -> Result<(), RuntimeError> {
        let resolved = self.resolve_module_path(path).map_err(|e| {
            RuntimeError::ModuleImportError(format!(
                "Module '{}' not found relative to '{}' on line {}: {}",
                path,
                self.current_file.display(),
                line,
                e
            ))
        })?;
        // Cycle detection.
        if self.loading_stack.contains(&resolved) {
            return Err(RuntimeError::ModuleImportError(format!(
                "Recursive import of '{}'",
                resolved.display()
            )));
        }
        // Cache.
        let info = if let Some(info) = self.loaded_modules.get(&resolved) {
            info.clone()
        } else {
            self.compile_module_inline(&resolved)?
        };
        // Bind the alias to a frozen namespace built at this point in the
        // bytecode. For each export:
        //   - functions:  PUSH_STR <mangled fn name>
        //   - values:     LOAD <mangled var name>
        // Then BUILD_DICT and freeze.
        for (name, kind) in &info.exports {
            self.emit(Instr::PushStr(name.clone()));
            match kind {
                ExportKind::Func => {
                    self.emit(Instr::PushStr(format!("{}{}", info.prefix, name)));
                }
                ExportKind::Value => {
                    // Reserved globals (`args`, `module_file`, `current_dir`)
                    // were stored unprefixed by the imported module's
                    // `Decl` — see `resolve_store`. We must Load them by the
                    // same name here.
                    let load_name = if Self::is_reserved_global(name) {
                        name.clone()
                    } else {
                        format!("{}{}", info.prefix, name)
                    };
                    self.emit(Instr::Load(load_name));
                }
            }
        }
        self.emit(Instr::BuildDict(info.exports.len()));
        self.emit(Instr::CallBuiltin("freeze".to_string(), 1));
        self.compile_import_alias_store(alias);
        Ok(())
    }

    fn resolve_module_path(&self, raw: &str) -> std::io::Result<PathBuf> {
        let raw = raw.replace('\\', "/");
        let p = Path::new(&raw);
        let base = self
            .current_file
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let candidate = if p.is_absolute() {
            p.to_path_buf()
        } else {
            base.join(p)
        };
        // Normalise (`./` and `..` collapse) without canonicalising so
        // the path stored in the source-file table matches what the
        // OMG-in-OMG compiler stores — required for triple-meta
        // byte-identical parity. Cycle-detection still works since
        // imports of the same logical path produce the same normalised
        // key. (A symlink redirecting two paths to the same file would
        // load it twice; not a concern in the corpus.)
        Ok(PathBuf::from(path_normalize(&candidate.to_string_lossy())))
    }

    fn compile_module_inline(
        &mut self,
        resolved: &Path,
    ) -> Result<ModuleInfo, RuntimeError> {
        let source = fs::read_to_string(resolved).map_err(|e| {
            RuntimeError::ModuleImportError(format!(
                "Cannot read '{}': {}",
                resolved.display(),
                e
            ))
        })?;
        // Lex + parse the imported file.
        let toks = tokenize(&source, &resolved.display().to_string())?;
        let ast = {
            let mut parser = Parser::new(&toks, resolved.display().to_string());
            parser.parse_program()?
        };
        // Determine exports from top-level decls / func_defs.
        let mut exports: Vec<(String, ExportKind)> = Vec::new();
        for stmt in &ast {
            match stmt {
                Node::Decl(name, _, _) => {
                    exports.push((name.clone(), ExportKind::Value));
                }
                Node::FuncDef(name, _, _, _) => {
                    exports.push((name.clone(), ExportKind::Func));
                }
                _ => {}
            }
        }
        // Allocate a unique mangling prefix for this module.
        self.module_counter += 1;
        let prefix = format!("__mod_{}__", self.module_counter);
        let info = ModuleInfo {
            prefix: prefix.clone(),
            exports,
        };
        // Insert into cache up front so cycles see a partially-compiled entry.
        self.loaded_modules
            .insert(resolved.to_path_buf(), info.clone());
        self.loading_stack.insert(resolved.to_path_buf());
        // Swap context: compile imported file's top-level statements inline at
        // the current import site, prefixed with this module's namespace.
        // The file index swap is what lets traceback frames inside this
        // module's body show the imported file's path, not the entry's.
        let saved_prefix = std::mem::replace(&mut self.current_prefix, prefix);
        let saved_file = std::mem::replace(&mut self.current_file, resolved.to_path_buf());
        let new_file_idx = self.intern_file(resolved);
        let saved_file_idx = self.current_file_idx;
        self.current_file_idx = new_file_idx;
        self.collect_proc_arity(&ast);
        for stmt in ast {
            self.compile_stmt(&stmt)?;
        }
        self.current_prefix = saved_prefix;
        self.current_file = saved_file;
        self.current_file_idx = saved_file_idx;
        self.loading_stack.remove(resolved);
        Ok(info)
    }

    // ------------------------------------------------------------------
    // Expressions
    // ------------------------------------------------------------------

    fn compile_expr(&mut self, expr: &Node) -> Result<(), RuntimeError> {
        self.enter_node(expr);
        match expr {
            Node::Number(v, _) => self.emit(Instr::PushInt(*v)),
            Node::Float(v, _) => self.emit(Instr::PushFloat(*v)),
            Node::Str(s, _) => self.emit(Instr::PushStr(s.clone())),
            Node::Bool(b, _) => self.emit(Instr::PushBool(*b)),
            Node::Ident(name, _) => self.emit(Instr::Load(self.resolve_load(name))),
            Node::List(elems, _) => {
                for e in elems {
                    self.compile_expr(e)?;
                }
                self.emit(Instr::BuildList(elems.len()));
            }
            Node::Dict(pairs, _) => {
                for (k, v) in pairs {
                    self.emit(Instr::PushStr(k.clone()));
                    self.compile_expr(v)?;
                }
                self.emit(Instr::BuildDict(pairs.len()));
            }
            Node::Index(base, idx, _) => {
                self.compile_expr(base)?;
                self.compile_expr(idx)?;
                self.emit(Instr::Index);
            }
            Node::Slice(base, start, end, _) => {
                self.compile_expr(base)?;
                self.compile_expr(start)?;
                if let Some(e) = end {
                    self.compile_expr(e)?;
                } else {
                    self.emit(Instr::PushNone);
                }
                self.emit(Instr::Slice);
            }
            Node::Dot(base, attr, _) => {
                self.compile_expr(base)?;
                self.emit(Instr::Attr(attr.clone()));
            }
            Node::FuncCall(callee, args, _) => {
                if let Node::Ident(name, _) = callee.as_ref() {
                    if let Some(kind) = lookup_raise(name) {
                        self.compile_raise_call(kind, args)?;
                        return Ok(());
                    }
                    if is_builtin(name) {
                        for a in args {
                            self.compile_expr(a)?;
                        }
                        self.emit(Instr::CallBuiltin(name.clone(), args.len()));
                        return Ok(());
                    }
                    if self.is_value_binding(name) {
                        // Any value binding — parameters, allocs, nested
                        // procs, or top-level allocs holding a closure —
                        // is invoked indirectly via Load + CallValue. Only
                        // top-level proc names use the direct `Call` fast
                        // path below.
                        self.emit(Instr::Load(self.resolve_load(name)));
                        for a in args {
                            self.compile_expr(a)?;
                        }
                        self.emit(Instr::CallValue(args.len()));
                        return Ok(());
                    }
                    // Top-level proc: emit a direct Call for the fast path.
                    let resolved = self.resolve_call_name(name);
                    self.check_direct_call_arity(name, &resolved, args.len())?;
                    for a in args {
                        self.compile_expr(a)?;
                    }
                    self.emit(Instr::Call(resolved));
                    return Ok(());
                }
                // Generic case: callee is an arbitrary expression (e.g.
                // `pair[0](x)` or `freeze(d).fn(x)`).
                self.compile_expr(callee)?;
                for a in args {
                    self.compile_expr(a)?;
                }
                self.emit(Instr::CallValue(args.len()));
            }
            Node::Unary(op, inner, _) => {
                self.compile_expr(inner)?;
                match op {
                    UnaryOp::Plus => {} // no-op
                    UnaryOp::Neg => self.emit(Instr::Neg),
                    UnaryOp::BNot => self.emit(Instr::Not),
                }
            }
            Node::Binary(BinOp::And, lhs, rhs, _) => {
                // Short-circuit: if lhs is falsy, the whole expression is
                // false; otherwise the result is bool(rhs). This matches the
                // Python reference interpreter (which also returns a bool).
                self.compile_expr(lhs)?;
                let jf = self.placeholder(Instr::JumpIfFalse(0));
                self.compile_expr(rhs)?;
                self.emit(Instr::PushBool(true));
                self.emit(Instr::And);
                let end_jump = self.placeholder(Instr::Jump(0));
                let on_false_pc = self.code.len();
                self.patch_jump(jf, on_false_pc);
                self.emit(Instr::PushBool(false));
                let end_pc = self.code.len();
                self.patch_jump(end_jump, end_pc);
            }
            Node::Binary(BinOp::Or, lhs, rhs, _) => {
                // Short-circuit: if lhs is truthy → true. Otherwise return
                // bool(rhs).
                self.compile_expr(lhs)?;
                let jf = self.placeholder(Instr::JumpIfFalse(0));
                // lhs was truthy → push true.
                self.emit(Instr::PushBool(true));
                let end_jump = self.placeholder(Instr::Jump(0));
                let on_false_pc = self.code.len();
                self.patch_jump(jf, on_false_pc);
                self.compile_expr(rhs)?;
                self.emit(Instr::PushBool(false));
                self.emit(Instr::Or);
                let end_pc = self.code.len();
                self.patch_jump(end_jump, end_pc);
            }
            Node::Binary(op, lhs, rhs, _) => {
                self.compile_expr(lhs)?;
                self.compile_expr(rhs)?;
                let instr = match op {
                    BinOp::Add => Instr::Add,
                    BinOp::Sub => Instr::Sub,
                    BinOp::Mul => Instr::Mul,
                    BinOp::Div => Instr::Div,
                    BinOp::FloorDiv => Instr::FloorDiv,
                    BinOp::Mod => Instr::Mod,
                    BinOp::BAnd => Instr::BAnd,
                    BinOp::BOr => Instr::BOr,
                    BinOp::BXor => Instr::BXor,
                    BinOp::Shl => Instr::Shl,
                    BinOp::Shr => Instr::Shr,
                    BinOp::Eq => Instr::Eq,
                    BinOp::Ne => Instr::Ne,
                    BinOp::Lt => Instr::Lt,
                    BinOp::Le => Instr::Le,
                    BinOp::Gt => Instr::Gt,
                    BinOp::Ge => Instr::Ge,
                    BinOp::And | BinOp::Or => unreachable!("handled above"),
                };
                self.emit(instr);
            }
            other => {
                return Err(RuntimeError::SyntaxError(format!(
                    "Unsupported expression at line {} in {}: {:?}",
                    other.line(),
                    self.current_file.display(),
                    DebugNode(other)
                )));
            }
        }
        Ok(())
    }
}

/// Collapse `./` and `..` segments in a path string. Mirrors the OMG
/// compiler's `path_normalize` (bootstrap/src/compiler.omg) byte-for-
/// byte so both frontends store the same path in the source-file
/// table. We can't use `fs::canonicalize` here because the OMG side
/// has no OS-level canonicalisation primitive — matching its weaker
/// normalisation is what keeps the triple-meta `.omgb` bytes equal.
///
/// Also called from main.rs's `absolute_normalised` helper to make
/// entry paths absolute before the compiler sees them.
pub fn path_normalize(p: &str) -> String {
    if p.is_empty() {
        return String::new();
    }
    let absolute = p.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for seg in p.split(|c| c == '/' || c == '\\') {
        if !seg.is_empty() {
            parts.push(seg);
        }
    }
    let mut stack: Vec<&str> = Vec::new();
    for seg in &parts {
        if *seg == "." {
            // drop
        } else if *seg == ".." {
            if let Some(&top) = stack.last() {
                if top != ".." {
                    stack.pop();
                    continue;
                }
            }
            // Only meaningful on relative paths.
            if !absolute {
                stack.push(*seg);
            }
        } else {
            stack.push(*seg);
        }
    }
    let body = stack.join("/");
    let out = if absolute {
        format!("/{}", body)
    } else {
        body
    };
    if out.is_empty() {
        ".".to_string()
    } else {
        out
    }
}

fn rebase_jump(instr: Instr, base: usize) -> Instr {
    match instr {
        Instr::Jump(t) => Instr::Jump(t + base),
        Instr::JumpIfFalse(t) => Instr::JumpIfFalse(t + base),
        Instr::SetupExcept(t) => Instr::SetupExcept(t + base),
        other => other,
    }
}

/// Compile a complete source string from a known file path, recursively
/// resolving `import` statements relative to that file.
pub fn compile_source(source: &str, file: impl AsRef<Path>) -> Result<Program, RuntimeError> {
    compile_source_with_globals(source, file, &[])
}

/// Compile while pretending the given names are already declared at top
/// level. Used by the REPL to tell the compiler about `alloc`s and `proc`s
/// from earlier turns, so that calls like `add5(10)` get lowered to
/// `Load + CallValue` (closure path) instead of `Call("add5")` (direct
/// function-table lookup, which would fail since `add5` is a value, not
/// a registered function).
pub fn compile_source_with_globals(
    source: &str,
    file: impl AsRef<Path>,
    known_globals: &[String],
) -> Result<Program, RuntimeError> {
    let path = file.as_ref().to_path_buf();
    let toks = tokenize(source, &path.display().to_string())?;
    let ast = {
        let mut p = Parser::new(&toks, path.display().to_string());
        p.parse_program()?
    };
    let mut compiler = Compiler::new(path);
    for name in known_globals {
        compiler.declare_local(name);
    }
    compiler.compile_program(ast)
}

// --- Debug formatting helpers (kept out of public surface) -----------------

struct DebugInstr<'a>(&'a Instr);
impl<'a> std::fmt::Debug for DebugInstr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Concise rendering — the variant name is enough for compiler errors.
        let name = match self.0 {
            Instr::Jump(_) => "Jump",
            Instr::JumpIfFalse(_) => "JumpIfFalse",
            Instr::SetupExcept(_) => "SetupExcept",
            _ => "instr",
        };
        write!(f, "{}", name)
    }
}

struct DebugNode<'a>(&'a Node);
impl<'a> std::fmt::Debug for DebugNode<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self.0 {
            Node::Number(..) => "Number",
            Node::Float(..) => "Float",
            Node::Str(..) => "Str",
            Node::Bool(..) => "Bool",
            Node::List(..) => "List",
            Node::Dict(..) => "Dict",
            Node::Ident(..) => "Ident",
            Node::Binary(..) => "Binary",
            Node::Unary(..) => "Unary",
            Node::Index(..) => "Index",
            Node::Slice(..) => "Slice",
            Node::Dot(..) => "Dot",
            Node::FuncCall(..) => "FuncCall",
            Node::Decl(..) => "Decl",
            Node::Assign(..) => "Assign",
            Node::AttrAssign(..) => "AttrAssign",
            Node::IndexAssign(..) => "IndexAssign",
            Node::Emit(..) => "Emit",
            Node::Facts(..) => "Facts",
            Node::Import(..) => "Import",
            Node::If(..) => "If",
            Node::Loop(..) => "Loop",
            Node::Break(..) => "Break",
            Node::FuncDef(..) => "FuncDef",
            Node::Return(..) => "Return",
            Node::ExprStmt(..) => "ExprStmt",
            Node::Block(..) => "Block",
            Node::Try(..) => "Try",
        };
        write!(f, "{}", name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile(src: &str) -> Program {
        let path = std::env::temp_dir().join("compile_test.omg");
        fs::write(&path, src).unwrap();
        let prog = compile_source(src, &path).unwrap();
        let _ = fs::remove_file(&path);
        prog
    }

    #[test]
    fn compiles_hello() {
        let prog = compile(";;;omg\nemit \"hi\"\n");
        assert!(matches!(prog.code.first(), Some(Instr::PushStr(_))));
    }

    #[test]
    fn compiles_function_call() {
        let prog = compile(";;;omg\nproc f(x) { return x + 1 }\nemit f(5)\n");
        assert!(prog.funcs.contains_key("f"));
    }
}
