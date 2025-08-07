use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use serde_json::Value;

#[derive(Clone)]
struct Instr {
    op: String,
    arg: Option<Arg>,
}

#[derive(Clone)]
enum Arg {
    Int(i64),
    Str(String),
    Builtin(String, usize),
}

struct FunctionEntry {
    name: String,
    params: Vec<String>,
    address: usize,
}

struct Compiler {
    code: Vec<Instr>,
    pending_funcs: Vec<(String, Vec<String>, Vec<Instr>)>,
    funcs: Vec<FunctionEntry>,
    break_stack: Vec<Vec<usize>>,
    builtins: HashSet<String>,
}

impl Compiler {
    fn new() -> Self {
        let builtins = ["chr", "ascii", "hex", "binary", "length", "read_file"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        Self {
            code: Vec::new(),
            pending_funcs: Vec::new(),
            funcs: Vec::new(),
            break_stack: Vec::new(),
            builtins,
        }
    }

    fn emit(&mut self, op: &str, arg: Option<Arg>) {
        self.code.push(Instr { op: op.to_string(), arg });
    }

    fn emit_placeholder(&mut self, op: &str) -> usize {
        let idx = self.code.len();
        self.code.push(Instr { op: op.to_string(), arg: None });
        idx
    }

    fn patch(&mut self, idx: usize, target: usize) {
        let op = self.code[idx].op.clone();
        self.code[idx] = Instr { op, arg: Some(Arg::Int(target as i64)) };
    }

    fn compile(&mut self, ast: &Vec<Value>) {
        self.compile_block(ast);
        self.emit("HALT", None);

        let mut final_code = self.code.clone();
        for (name, params, body) in self.pending_funcs.drain(..) {
            let addr = final_code.len();
            self.funcs.push(FunctionEntry { name: name.clone(), params: params.clone(), address: addr });
            for instr in body {
                match instr.arg {
                    Some(Arg::Int(i)) if instr.op == "JUMP" || instr.op == "JUMP_IF_FALSE" => {
                        final_code.push(Instr { op: instr.op, arg: Some(Arg::Int(i + addr as i64)) });
                    }
                    _ => final_code.push(instr),
                }
            }
        }
        self.code = final_code;
    }

    fn compile_block(&mut self, block: &Vec<Value>) {
        for stmt in block {
            self.compile_stmt(stmt);
        }
    }

    fn compile_stmt(&mut self, stmt: &Value) {
        let arr = stmt.as_array().expect("stmt array");
        let kind = arr[0].as_str().expect("kind str");
        match kind {
            "emit" => {
                self.compile_expr(&arr[1]);
                self.emit("EMIT", None);
            }
            "decl" | "assign" => {
                let name = arr[1].as_str().unwrap().to_string();
                self.compile_expr(&arr[2]);
                self.emit("STORE", Some(Arg::Str(name)));
            }
            "attr_assign" => {
                self.compile_expr(&arr[1]);
                self.compile_expr(&arr[3]);
                let attr = arr[2].as_str().unwrap().to_string();
                self.emit("STORE_ATTR", Some(Arg::Str(attr)));
            }
            "index_assign" => {
                self.compile_expr(&arr[1]);
                self.compile_expr(&arr[2]);
                self.compile_expr(&arr[3]);
                self.emit("STORE_INDEX", None);
            }
            "expr_stmt" => {
                self.compile_expr(&arr[1]);
                self.emit("POP", None);
            }
            "import" => {
                let path = arr[1].as_str().unwrap().to_string();
                let alias = arr[2].as_str().unwrap().to_string();
                self.emit("PUSH_STR", Some(Arg::Str(path)));
                self.emit("IMPORT", None);
                self.emit("STORE", Some(Arg::Str(alias)));
            }
            "facts" => {
                self.compile_expr(&arr[1]);
                self.emit("ASSERT", None);
            }
            "if" => {
                // Unroll nested if/elif chain
                let mut cond_blocks: Vec<(Value, Vec<Value>)> = Vec::new();
                let mut current = stmt.clone();
                let mut else_block: Option<Vec<Value>> = None;
                loop {
                    let carr = current.as_array().unwrap();
                    let cond = carr[1].clone();
                    let block_node = &carr[2];
                    let block = block_node.as_array().unwrap()[1].as_array().unwrap().clone();
                    cond_blocks.push((cond, block));
                    let tail = &carr[3];
                    if let Some(tarr) = tail.as_array() {
                        if tarr[0].as_str().unwrap() == "if" {
                            current = tail.clone();
                            continue;
                        } else if tarr[0].as_str().unwrap() == "block" {
                            else_block = Some(tarr[1].as_array().unwrap().clone());
                        }
                    }
                    break;
                }
                let mut end_jumps = Vec::new();
                for (cond, block) in cond_blocks {
                    self.compile_expr(&cond);
                    let jf = self.emit_placeholder("JUMP_IF_FALSE");
                    self.compile_block(&block);
                    end_jumps.push(self.emit_placeholder("JUMP"));
                    self.patch(jf, self.code.len());
                }
                if let Some(block) = else_block {
                    self.compile_block(&block);
                }
                for j in end_jumps {
                    self.patch(j, self.code.len());
                }
            }
            "loop" => {
                let start = self.code.len();
                self.compile_expr(&arr[1]);
                let jf = self.emit_placeholder("JUMP_IF_FALSE");
                let body = arr[2].as_array().unwrap()[1].as_array().unwrap().clone();
                self.break_stack.push(Vec::new());
                self.compile_block(&body);
                self.emit("JUMP", Some(Arg::Int(start as i64)));
                self.patch(jf, self.code.len());
                if let Some(brks) = self.break_stack.pop() {
                    for idx in brks {
                        self.patch(idx, self.code.len());
                    }
                }
            }
            "func_def" => {
                let name = arr[1].as_str().unwrap().to_string();
                let params = arr[2]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect::<Vec<_>>();
                let body = arr[3].as_array().unwrap()[1].as_array().unwrap().clone();
                let body_code = self.compile_function_body(&body);
                self.pending_funcs.push((name, params, body_code));
            }
            "return" => {
                let expr = &arr[1];
                if let Some(farr) = expr.as_array() {
                    if farr[0].as_str().unwrap() == "func_call" {
                        if let Some(func_arr) = farr[1].as_array() {
                            if func_arr[0].as_str().unwrap() == "ident" {
                                let name = func_arr[1].as_str().unwrap();
                                let args = farr[2].as_array().unwrap();
                                for a in args {
                                    self.compile_expr(a);
                                }
                                if self.builtins.contains(name) {
                                    self.emit("BUILTIN", Some(Arg::Builtin(name.to_string(), args.len())));
                                    self.emit("RET", None);
                                } else {
                                    self.emit("TCALL", Some(Arg::Str(name.to_string())));
                                }
                                return;
                            }
                        }
                    }
                }
                self.compile_expr(expr);
                self.emit("RET", None);
            }
            "break" => {
                if self.break_stack.is_empty() {
                    panic!("'break' used outside of loop");
                }
                let j = self.emit_placeholder("JUMP");
                self.break_stack.last_mut().unwrap().push(j);
            }
            "block" => {
                let stmts = arr[1].as_array().unwrap();
                self.compile_block(stmts);
            }
            _ => panic!("Unsupported statement: {:?}", stmt),
        }
    }

    fn compile_function_body(&mut self, body: &Vec<Value>) -> Vec<Instr> {
        let saved = std::mem::take(&mut self.code);
        self.compile_block(body);
        self.emit("RET", None);
        let body_code = std::mem::take(&mut self.code);
        self.code = saved;
        body_code
    }

    fn compile_expr(&mut self, node: &Value) {
        let arr = node.as_array().expect("expr array");
        let op = arr[0].as_str().unwrap();
        match op {
            "number" => {
                self.emit("PUSH_INT", Some(Arg::Int(arr[1].as_i64().unwrap())));
            }
            "string" => {
                self.emit("PUSH_STR", Some(Arg::Str(arr[1].as_str().unwrap().to_string())));
            }
            "bool" => {
                let v = if arr[1].as_bool().unwrap() { 1 } else { 0 };
                self.emit("PUSH_BOOL", Some(Arg::Int(v)));
            }
            "ident" => {
                self.emit("LOAD", Some(Arg::Str(arr[1].as_str().unwrap().to_string())));
            }
            "list" => {
                let elems = arr[1].as_array().unwrap();
                for e in elems {
                    self.compile_expr(e);
                }
                self.emit("BUILD_LIST", Some(Arg::Int(elems.len() as i64)));
            }
            "dict" => {
                let pairs = arr[1].as_array().unwrap();
                for p in pairs {
                    let key = p.as_array().unwrap()[0].as_str().unwrap().to_string();
                    let val = &p.as_array().unwrap()[1];
                    self.emit("PUSH_STR", Some(Arg::Str(key)));
                    self.compile_expr(val);
                }
                self.emit("BUILD_DICT", Some(Arg::Int(pairs.len() as i64)));
            }
            "index" => {
                self.compile_expr(&arr[1]);
                self.compile_expr(&arr[2]);
                self.emit("INDEX", None);
            }
            "slice" => {
                self.compile_expr(&arr[1]);
                self.compile_expr(&arr[2]);
                if arr[3].is_null() {
                    self.emit("PUSH_NONE", None);
                } else {
                    self.compile_expr(&arr[3]);
                }
                self.emit("SLICE", None);
            }
            "dot" => {
                self.compile_expr(&arr[1]);
                self.emit("ATTR", Some(Arg::Str(arr[2].as_str().unwrap().to_string())));
            }
            "func_call" => {
                let func_node = &arr[1];
                let args = arr[2].as_array().unwrap();
                if let Some(farr) = func_node.as_array() {
                    if farr[0].as_str().unwrap() == "ident" {
                        let name = farr[1].as_str().unwrap();
                        for a in args {
                            self.compile_expr(a);
                        }
                        if self.builtins.contains(name) {
                            self.emit("BUILTIN", Some(Arg::Builtin(name.to_string(), args.len())));
                        } else {
                            self.emit("CALL", Some(Arg::Str(name.to_string())));
                        }
                        return;
                    }
                }
                self.compile_expr(func_node);
                for a in args {
                    self.compile_expr(a);
                }
                self.emit("CALL_VALUE", Some(Arg::Int(args.len() as i64)));
            }
            "unary" => {
                let unary_op = arr[1].as_str().unwrap();
                self.compile_expr(&arr[2]);
                match unary_op {
                    "sub" => self.emit("NEG", None),
                    "not_bits" => self.emit("NOT", None),
                    "add" => {}
                    _ => panic!("Unknown unary op {}", unary_op),
                }
            }
            _ => {
                // binary operations encoded as op name
                let ops: HashMap<&'static str, &'static str> = [
                    ("add", "ADD"),
                    ("sub", "SUB"),
                    ("mul", "MUL"),
                    ("div", "DIV"),
                    ("mod", "MOD"),
                    ("eq", "EQ"),
                    ("ne", "NE"),
                    ("gt", "GT"),
                    ("lt", "LT"),
                    ("ge", "GE"),
                    ("le", "LE"),
                    ("and", "AND"),
                    ("or", "OR"),
                    ("and_bits", "BAND"),
                    ("or_bits", "BOR"),
                    ("xor_bits", "BXOR"),
                    ("shl", "SHL"),
                    ("shr", "SHR"),
                ].into_iter().collect();
                if let Some(opcode) = ops.get(op) {
                    self.compile_expr(&arr[1]);
                    self.compile_expr(&arr[2]);
                    self.emit(opcode, None);
                } else {
                    panic!("Unsupported expression node: {:?}", node);
                }
            }
        }
    }

    fn to_string(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        for f in &self.funcs {
            let params = f.params.join(" ");
            lines.push(format!("FUNC {} {} {} {}", f.name, f.params.len(), params, f.address));
        }
        for instr in &self.code {
            match &instr.arg {
                Some(Arg::Builtin(name, argc)) => {
                    lines.push(format!("BUILTIN {} {}", name, argc));
                }
                Some(Arg::Str(s)) if instr.op == "PUSH_STR" => {
                    lines.push(format!("PUSH_STR {}", serde_json::to_string(s).unwrap()));
                }
                Some(Arg::Str(s)) => {
                    lines.push(format!("{} {}", instr.op, s));
                }
                Some(Arg::Int(i)) => {
                    lines.push(format!("{} {}", instr.op, i));
                }
                None => lines.push(instr.op.clone()),
            }
        }
        lines.join("\n")
    }
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest.parent().expect("no parent");
    let source_path = repo_root.join("bootstrap/interpreter.omg");
    let out_dir = manifest.join("bytecode");
    fs::create_dir_all(&out_dir).expect("create bytecode dir");
    let out_bc = out_dir.join("interpreter.bc");

    // Parse source via Python to obtain AST JSON
    let output = Command::new("python")
        .arg("-c")
        .arg("import json,sys;from omglang.lexer import tokenize;from omglang.parser import Parser;path=sys.argv[1];src=open(path,encoding='utf-8').read();tok,tm=tokenize(src);ast=Parser(tok,tm,path).parse();json.dump(ast,sys.stdout)")
        .arg(&source_path)
        .current_dir(repo_root)
        .output()
        .expect("failed to run python parser");
    if !output.status.success() {
        panic!("python parser failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    let ast_json = String::from_utf8(output.stdout).expect("non utf8 ast");
    let ast_val: Value = serde_json::from_str(&ast_json).expect("invalid ast json");
    let ast_array = ast_val.as_array().expect("ast not array").clone();

    let mut compiler = Compiler::new();
    compiler.compile(&ast_array);
    let bc_str = compiler.to_string();

    fs::write(&out_bc, bc_str).expect("write bytecode");

    println!("cargo:rerun-if-changed={}", source_path.display());
    println!("cargo:rerun-if-changed={}/omglang/lexer.py", repo_root.display());
    println!("cargo:rerun-if-changed={}/omglang/parser", repo_root.display());
}
