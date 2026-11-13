/*
 * omg_rt.h  →  inlined at the top of every .c emitted by
 *              src/backends/native-c.omg (`omg --build`, `omg --emit-c`).
 *
 * The OMG C runtime: value types, refcounting, list/dict/string
 * helpers, error handling, file + TCP builtins. Every transpiled
 * program has this header pasted in, so the resulting executable
 * depends only on libc (plus, on Windows, the system DLLs).
 *
 * Portability: everything that differs between POSIX (Linux, macOS,
 * the BSDs) and Windows sits behind `#ifdef _WIN32`, in the "Platform
 * layer" section below and in the handful of builtins that talk to the
 * operating system directly. The behaviour on Windows follows the Rust
 * runtime's Windows backend (rust/src/vm/sys.rs): what cannot exist
 * there (fork, pseudo-terminals) raises a catchable ValueError.
 * On Windows the supported C compilers are GCC and Clang (MinGW-w64).
 */
#ifdef _WIN32
/* C99 printf/scanf (%lld, %zu, two-digit exponents) from MinGW itself
 * rather than from the old msvcrt. Must precede every system header. */
#ifndef __USE_MINGW_ANSI_STDIO
#define __USE_MINGW_ANSI_STDIO 1
#endif
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
/* try/except is setjmp/longjmp. MinGW's default longjmp on 64-bit
 * Windows walks the stack with the system unwinder, which is meant for
 * C++ and SEH frames and is not reliable through generated code like
 * this (it can end the process silently). There is nothing to unwind
 * here, so ask for the plain register-restoring kind. */
#ifndef __USE_MINGW_SETJMP_NON_SEH
#define __USE_MINGW_SETJMP_NON_SEH 1
#endif
#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>
#include <shellapi.h>     /* CommandLineToArgvW */
#include <io.h>           /* _setmode, _read */
#include <fcntl.h>        /* _O_BINARY */
#include <wchar.h>
#endif
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <setjmp.h>
#if defined(_WIN32) && defined(__x86_64__) && defined(__SEH__)
/* Headers older than the switch above: same effect by hand. */
#undef setjmp
#define setjmp(BUF) _setjmp((BUF), NULL)
#endif
#include <stdarg.h>
#include <errno.h>
#include <time.h>
#include <sys/stat.h>
#include <sys/types.h>
#ifndef _WIN32
#include <sys/wait.h>
#include <dirent.h>
#include <unistd.h>
#include <sys/socket.h>
#include <poll.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <termios.h>
/* forkpty lives in a different header on each Unix family. */
#if defined(__APPLE__) || defined(__OpenBSD__) || defined(__NetBSD__)
#include <util.h>
#elif defined(__FreeBSD__) || defined(__DragonFly__)
#include <libutil.h>
#else
#include <pty.h>
#endif
#if defined(__APPLE__)
#include <mach-o/dyld.h>  /* _NSGetExecutablePath */
#endif
#include <sys/ioctl.h>    /* TIOCSWINSZ */
#include <fcntl.h>        /* fcntl */
#include <arpa/inet.h>
#include <netdb.h>
#include <signal.h>
#include <sys/resource.h> /* getrlimit / setrlimit for the stack guard */
#endif

typedef enum {
    OMG_INT,
    OMG_STR,
    OMG_BOOL,
    OMG_NONE,
    OMG_CLOSURE,
    OMG_LIST,
    OMG_DICT,
    OMG_FLOAT,
    /* Not an OMG value: the state of a variable slot that nothing has
     * been stored into yet. LOAD and STORE check for it and raise
     * UndefinedIdentError, the way the VMs do when a name is missing
     * from the environment. It never reaches the operand stack. */
    OMG_UNSET
} omg_tag;

/* Forward-declared so Value can hold pointers. */
struct OmgClosure;
struct OmgList;
struct OmgDict;

typedef struct {
    omg_tag tag;
    union {
        int64_t i;
        const char *s;
        int b;
        struct OmgClosure *c;
        struct OmgList *l;
        struct OmgDict *d;
        double f;
    } v;
} Value;

/* Every OMG proc compiles to a C function with this signature. Args are
 * passed inline rather than through a pointer-array so cc can sibling-
 * call-optimise tail-position calls without worrying about local-array
 * pointers escaping. `argc` is the actual number of meaningful args
 * (0..MAX_ARITY); higher slots are passed as omg_none() and ignored.
 *
 * The cap is generously sized — OMG procs in this repo use ≤9 params,
 * and 32 lets a future user write a struct-init-style helper without
 * hitting it. The cost is a handful of stack slots per call on x86_64
 * (args beyond 6 spill); sibling-CO still works because the args are
 * passed inline. If you raise it further, mirror the change in
 * native-c.omg (args_list + emit_function), compiler.omg's allowlist,
 * and the docs in docs/06-runtime.md. */
#define OMG_MAX_ARITY 32

typedef Value (*OmgFn)(Value *captured, int cap_count, int argc,
                       Value a0,  Value a1,  Value a2,  Value a3,
                       Value a4,  Value a5,  Value a6,  Value a7,
                       Value a8,  Value a9,  Value a10, Value a11,
                       Value a12, Value a13, Value a14, Value a15,
                       Value a16, Value a17, Value a18, Value a19,
                       Value a20, Value a21, Value a22, Value a23,
                       Value a24, Value a25, Value a26, Value a27,
                       Value a28, Value a29, Value a30, Value a31);

typedef struct OmgClosure {
    int rc;
    OmgFn fn;
    /* Bare source name (no `__mod_N__` mangling). Used as the frame
     * label when this closure is invoked indirectly via CALL_VALUE,
     * so a traceback shows `in tick` rather than `in __mod_2__tick`.
     * Points at a static string in the emitted C — never freed. */
    const char *name;
    Value *captured;     /* heap-allocated; freed when rc drops to 0 */
    int cap_count;
} OmgClosure;

static inline Value omg_int(int64_t i) {
    Value v;
    v.tag = OMG_INT;
    v.v.i = i;
    return v;
}

static inline Value omg_str(const char *s) {
    Value v;
    v.tag = OMG_STR;
    v.v.s = s;
    return v;
}

static inline Value omg_bool(int b) {
    Value v;
    v.tag = OMG_BOOL;
    v.v.b = b ? 1 : 0;
    return v;
}

static inline Value omg_float(double f) {
    Value v;
    v.tag = OMG_FLOAT;
    v.v.f = f;
    return v;
}

/* Reinterpret a 64-bit integer as a double — used by PUSH_FLOAT, which
 * carries the IEEE-754 bit pattern in the bytecode rather than a
 * literal numeric form. memcpy is the portable, strict-aliasing-safe
 * way to do this; cc -O2 inlines it to a register move. */
static inline Value omg_float_from_bits(int64_t bits) {
    double f;
    memcpy(&f, &bits, sizeof(double));
    return omg_float(f);
}

static inline Value omg_none(void) {
    Value v;
    v.tag = OMG_NONE;
    return v;
}

/* The initial contents of every variable slot (see OMG_UNSET). File-
 * level globals get the same state from OMG_UNSET_INIT. */
static inline Value omg_unset(void) {
    Value v;
    v.tag = OMG_UNSET;
    v.v.i = 0;
    return v;
}
#define OMG_UNSET_INIT { OMG_UNSET, { 0 } }

/* Build a closure value. `captured` may be NULL when cap_count == 0
 * (top-level procs that don't capture anything). `name` is the bare
 * source name used by traceback rendering. Refcounted: the caller
 * receives a closure with rc=1; transferring that reference into a
 * slot doesn't bump it, but copying it (LOAD, etc.) does. */
static inline Value omg_closure(OmgFn fn, const char *name, Value *captured, int cap_count) {
    OmgClosure *c = (OmgClosure *)malloc(sizeof(OmgClosure));
    c->rc = 1;
    c->fn = fn;
    c->name = name;
    c->captured = captured;
    c->cap_count = cap_count;
    Value v;
    v.tag = OMG_CLOSURE;
    v.v.c = c;
    return v;
}

/* Forward-declared so omg_truthy / omg_emit can poke at sizes. */
static int omg_list_len(struct OmgList *l);
static int omg_dict_len(struct OmgDict *d);

/* Forward declaration so omg_raise can stringify any Value. Defined
 * later, alongside the rest of the string utilities. */
static const char *omg_value_to_cstr(Value v, char *buf, size_t bufsize);

/* === Source map + call-frame stack ========================================
 * For Python-style tracebacks. The transpiler emits a `omg_src_files`
 * table per program and threads `omg_current_file_idx` /
 * `omg_current_line` through the code so any panic site knows where
 * it fired. The frame stack is maintained by CALL / CALL_VALUE / RET
 * around each user-proc dispatch — TCALL replaces the top frame in
 * place so tail-call optimisation doesn't lose the caller's name.
 */

/* Populated by the transpiler at the top of the emitted C. NULL when
 * a program has no source map (e.g. a hand-rolled C harness running
 * omg_rt.h directly). */
static const char *const *omg_src_files = NULL;
static int omg_src_files_n = 0;

static uint32_t omg_current_file_idx = 0;
static uint32_t omg_current_line = 0;

typedef struct OmgFrame {
    const char *name;        /* bare function name (no module prefix) */
    uint32_t call_file_idx;  /* file containing the call instruction */
    uint32_t call_line;      /* line of the call instruction */
} OmgFrame;

#define OMG_MAX_FRAMES 1024
static OmgFrame omg_frame_stack[OMG_MAX_FRAMES];
static int omg_frame_top = 0;
/* Calls deeper than OMG_MAX_FRAMES are counted here instead of being
 * recorded, so each pop still pairs with its own push and the recorded
 * frames are intact once the recursion unwinds. Only tracebacks care,
 * and they print the outermost OMG_MAX_FRAMES frames. */
static int omg_frame_dropped = 0;

static inline void omg_frame_push(const char *name, uint32_t fi, uint32_t ln) {
    if (omg_frame_top >= OMG_MAX_FRAMES) {
        omg_frame_dropped++;
        return;
    }
    omg_frame_stack[omg_frame_top].name = name;
    omg_frame_stack[omg_frame_top].call_file_idx = fi;
    omg_frame_stack[omg_frame_top].call_line = ln;
    omg_frame_top++;
}

static inline void omg_frame_pop(void) {
    if (omg_frame_dropped > 0) { omg_frame_dropped--; return; }
    if (omg_frame_top > 0) omg_frame_top--;
}

/* TCALL replaces the top frame's display name without touching the
 * call-site — the original caller is still the one we'll eventually
 * return to. Mirrors the Rust VM's TCO behaviour. */
static inline void omg_frame_set_top_name(const char *name) {
    if (omg_frame_top > 0) {
        omg_frame_stack[omg_frame_top - 1].name = name;
    }
}

static inline const char *omg_lookup_file(uint32_t fi) {
    if (!omg_src_files || fi >= (uint32_t)omg_src_files_n) return "<unknown>";
    return omg_src_files[fi];
}

/* If `name` is a `__mod_N__bare` mangled identifier, return a pointer
 * one past the trailing `__`. Otherwise return the input unchanged.
 * Used by CALL_VALUE's string-callee path so traceback frames show
 * `in foo` rather than `in __mod_3__foo`. The returned pointer aliases
 * `name`; the caller must not free it. */
static const char *omg_strip_mod_prefix_cstr(const char *name) {
    if (!name) return name;
    if (strncmp(name, "__mod_", 6) != 0) return name;
    const char *p = name + 6;
    /* Skip digits. */
    const char *start = p;
    while (*p >= '0' && *p <= '9') p++;
    if (p == start) return name;
    /* Need the closing `__`. */
    if (p[0] != '_' || p[1] != '_') return name;
    return p + 2;
}

/* Print a Python-style traceback to stderr. Identical layout to the
 * Rust runtime's `format_traceback` (rust/src/vm.rs) so AOT, native
 * interp, and Rust all share the same error UX. */
static void omg_emit_traceback(const char *fullmsg) {
    fprintf(stderr, "Traceback (most recent call last):\n");
    /* Top-level entry: where main called the outermost frame from. */
    if (omg_frame_top > 0) {
        const OmgFrame *first = &omg_frame_stack[0];
        fprintf(stderr, "  File \"%s\", line %u, in <top-level>\n",
                omg_lookup_file(first->call_file_idx),
                (unsigned)first->call_line);
    }
    /* Each intermediate frame's call site sits inside the previous
     * frame's function; name it accordingly. */
    for (int i = 1; i < omg_frame_top; i++) {
        const OmgFrame *f = &omg_frame_stack[i];
        fprintf(stderr, "  File \"%s\", line %u, in %s\n",
                omg_lookup_file(f->call_file_idx),
                (unsigned)f->call_line,
                omg_frame_stack[i - 1].name);
    }
    /* Error site: pc -> omg_current_*, function is whichever frame
     * we're currently inside (or top-level for zero-frame errors). */
    const char *cur_fn = (omg_frame_top > 0)
        ? omg_frame_stack[omg_frame_top - 1].name
        : "<top-level>";
    fprintf(stderr, "  File \"%s\", line %u, in %s\n",
            omg_lookup_file(omg_current_file_idx),
            (unsigned)omg_current_line,
            cur_fn);
    fprintf(stderr, "%s\n", fullmsg);
}

/* === Exception handling ===================================================
 * try/except is implemented with setjmp/longjmp: each SETUP_EXCEPT
 * heap-allocates an OmgBlock, captures the current operand-stack depth,
 * setjmp's into the block's jmp_buf, and pushes the block onto a global
 * block stack. RAISE / runtime panics longjmp to the most recent block.
 * POP_BLOCK pops on clean exit.
 *
 * Caveat: longjmp across C function boundaries unwinds the C stack,
 * which means refcounted locals in callers between the try and the raise
 * leak. The same is true of values that were on the operand stack above
 * `saved_sp` when the panic happened. Both are documented limitations
 * for the native path; the OMG VM itself doesn't suffer from them
 * because it has full control of every frame.
 */

typedef struct OmgBlock {
    int saved_sp;
    int saved_frame_top;   /* depth of omg_frame_stack at try-entry */
    int saved_frame_dropped; /* omg_frame_dropped at try-entry */
    jmp_buf jb;
} OmgBlock;

#define OMG_MAX_BLOCKS 256
static OmgBlock *omg_block_stack[OMG_MAX_BLOCKS];
static int omg_block_top = 0;

/* Heap-allocated string carrying the panic's "Display" form. The except
 * handler reads this off the stack — it's pushed there by the SETUP_EXCEPT
 * trampoline immediately after longjmp returns. */
static Value omg_pending_error;

static void omg_block_push(OmgBlock *b) {
    if (omg_block_top >= OMG_MAX_BLOCKS) {
        fprintf(stderr, "VmInvariant: try-block stack overflow\n");
        exit(1);
    }
    b->saved_frame_dropped = omg_frame_dropped;
    omg_block_stack[omg_block_top++] = b;
}

/* POP_BLOCK / handler-exit path pops the topmost block and returns the
 * pointer so the caller can free it. */
static OmgBlock *omg_block_take(void) {
    if (omg_block_top > 0) return omg_block_stack[--omg_block_top];
    return NULL;
}

/* Build "<prefix>: <msg>" on the heap. Leaked — exception messages are
 * rare enough that this isn't worth tracking. */
static const char *omg_format_error(const char *prefix, const char *msg) {
    size_t lp = strlen(prefix), lm = strlen(msg);
    char *buf = (char *)malloc(lp + 2 + lm + 1);
    if (!buf) { fprintf(stderr, "out of memory\n"); exit(1); }
    memcpy(buf, prefix, lp);
    buf[lp] = ':';
    buf[lp + 1] = ' ';
    memcpy(buf + lp + 2, msg, lm);
    buf[lp + 2 + lm] = 0;
    return buf;
}

/* Centralised "raise this error" routine. If a try block is active,
 * longjmp to its handler; otherwise print the formatted message to
 * stderr and exit with status 1 (mirroring the OMG VM's main-loop
 * failure path). All recoverable errors in this runtime go through
 * here, so they're catchable from OMG. */
static void omg_panic(const char *prefix, const char *msg) {
    const char *full = omg_format_error(prefix, msg);
    if (omg_block_top > 0) {
        omg_pending_error = omg_str(full);
        OmgBlock *b = omg_block_stack[--omg_block_top];
        /* Restore the frame stack to its depth at SETUP_EXCEPT so a
         * later uncaught panic doesn't include frames pushed inside
         * the now-unwound try body. */
        if (omg_frame_top > b->saved_frame_top) {
            omg_frame_top = b->saved_frame_top;
        }
        omg_frame_dropped = b->saved_frame_dropped;
        longjmp(b->jb, 1);
    }
    /* Uncaught — emit a traceback if we have any source map info to
     * work with, otherwise fall back to the bare "Kind: msg" line so
     * harness/test programs without source data stay readable. */
    if (omg_src_files != NULL) {
        omg_emit_traceback(full);
    } else {
        fprintf(stderr, "%s\n", full);
    }
    exit(1);
}

/* Convenience wrapper: format with printf-style args, then panic. */
static void omg_panicf(const char *prefix, const char *fmt, ...) {
    char buf[512];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(buf, sizeof(buf), fmt, ap);
    va_end(ap);
    omg_panic(prefix, buf);
}

/* === Native stack guard ====================================================
 * Every OMG call is a C call, so OMG recursion consumes the C stack.
 * Without a check, recursing too deep is a segfault with no message.
 * Each emitted function starts with OMG_STACK_CHECK(), which compares
 * its frame address against a limit computed once in main() and raises
 * a normal (catchable) RuntimeError when the stack is nearly used up.
 *
 * On Linux the main thread's stack grows on demand up to the soft
 * RLIMIT_STACK *currently* in force, so omg_stack_init first raises a
 * small soft limit to 64 MiB (the size of the thread the Rust VM runs
 * on) to allow deeper recursion. Elsewhere the size is fixed when the
 * process starts, so the limit is used as found.
 *
 * On Windows a thread's stack is reserved in one piece when the thread
 * is created, so the runtime asks the system where that reservation
 * ends. main() (at the end of the platform layer) runs the program on
 * a thread with a 64 MiB stack for the same reason as the Linux case. */
static uintptr_t omg_stack_limit = 0;

#ifdef _WIN32
static void omg_stack_init(void *base) {
    /* The region a local variable lives in is the thread's stack; its
     * AllocationBase is the lowest address of the whole reservation. */
    MEMORY_BASIC_INFORMATION mbi;
    uintptr_t b = (uintptr_t)base;
    if (VirtualQuery((LPCVOID)b, &mbi, sizeof(mbi)) == 0) { omg_stack_limit = 0; return; }
    uintptr_t low = (uintptr_t)mbi.AllocationBase;
    size_t size = b > low ? (size_t)(b - low) : 0;
    size_t reserve = size >= (size_t)4 * 1024 * 1024 ? (size_t)1024 * 1024 : size / 4;
    omg_stack_limit = low + reserve;
}
#else
static void omg_stack_init(void *base) {
    size_t size = (size_t)8 * 1024 * 1024;
    struct rlimit rl;
    if (getrlimit(RLIMIT_STACK, &rl) == 0) {
#ifdef __linux__
        rlim_t want = (rlim_t)64 * 1024 * 1024;
        if (rl.rlim_cur != RLIM_INFINITY && rl.rlim_cur < want) {
            struct rlimit raised = rl;
            raised.rlim_cur = (rl.rlim_max == RLIM_INFINITY || rl.rlim_max >= want) ? want : rl.rlim_max;
            if (setrlimit(RLIMIT_STACK, &raised) == 0) rl = raised;
        }
#endif
        if (rl.rlim_cur == RLIM_INFINITY) size = (size_t)256 * 1024 * 1024;
        else size = (size_t)rl.rlim_cur;
    }
    /* Keep room for the environment block above main's frame and for
     * the frames that format and report the error. */
    size_t reserve = size >= (size_t)4 * 1024 * 1024 ? (size_t)1024 * 1024 : size / 4;
    size -= reserve;
    uintptr_t b = (uintptr_t)base;
    omg_stack_limit = b > size ? b - size : 0;
}
#endif

static void omg_stack_overflow(void) {
    omg_panic("RuntimeError", "maximum recursion depth exceeded");
}

#define OMG_STACK_CHECK() \
    do { \
        if ((uintptr_t)__builtin_frame_address(0) < omg_stack_limit) omg_stack_overflow(); \
    } while (0)

static const char *omg_kind_prefix(int kind) {
    switch (kind) {
        case 0: return "RuntimeError";
        case 1: return "SyntaxError";
        case 2: return "TypeError";
        case 3: return "UndefinedIdentError";
        case 4: return "ValueError";
        case 5: return "ModuleImportError";
        default: return "RuntimeError";
    }
}

/* RAISE handler: stringify the popped value and panic with the
 * caller-supplied error-kind prefix. Doesn't return. */
static void omg_raise(int kind, Value msg) {
    char buf[64];
    const char *m = omg_value_to_cstr(msg, buf, sizeof(buf));
    omg_panic(omg_kind_prefix(kind), m);
}

/* === Reference counting ===================================================
 * Lists, dicts, and closures live on the heap and are managed by a simple
 * non-cycle-collecting refcount. Strings are *not* refcounted in this
 * phase — heap-allocated strings (concat / slice / chr / etc. results)
 * leak; literal strings live in static C storage and don't need
 * collection at all. Strings are a smaller leaker than lists/dicts and
 * can be folded in later.
 *
 * Codegen contract:
 *
 *   - Every "owning slot" (a local variable, a global, a list element,
 *     a dict value, a captured-env entry) holds one reference. Slots
 *     start initialised to omg_none() (rc-irrelevant) and `omg_assign`
 *     drops the old reference before installing a new one.
 *
 *   - The operand stack also holds one reference per slot. Pushing an
 *     existing value `inc`s; pushing a freshly-created one (rc=1
 *     already) doesn't. Popping-and-discarding `dec`s. Popping-and-
 *     transferring (into another owning slot) is just sp--; the new
 *     slot inherits the reference.
 *
 *   - Binary/unary ops dec their operands explicitly after computing
 *     the result.
 *
 *   - Function calls: caller pops args without dec'ing (transferred
 *     ownership); callee `inc`s on entry to store in v_param slots,
 *     `dec`s all locals/params/captured before returning.
 */

/* Forward declarations; bodies live after the OmgList/OmgDict/
 * OmgClosure struct layouts so the inc/dec helpers can poke at `rc`. */
static void omg_dec(Value v);
static void omg_inc(Value v);
static void omg_assign(Value *slot, Value v);

/* Truthiness rules mirror OMG: 0, 0.0, "", false, None, [], {} are
 * falsy. NaN is truthy (matches Python). Closures, non-empty
 * lists/dicts, and other non-zero numbers are truthy. */
static inline int omg_truthy(Value v) {
    switch (v.tag) {
        case OMG_INT:     return v.v.i != 0;
        case OMG_FLOAT:   return v.v.f != 0.0;
        case OMG_STR:     return v.v.s[0] != '\0';
        case OMG_BOOL:    return v.v.b;
        case OMG_NONE:    return 0;
        case OMG_CLOSURE: return 1;
        case OMG_LIST:    return omg_list_len(v.v.l) != 0;
        case OMG_DICT:    return omg_dict_len(v.v.d) != 0;
    }
    return 0;
}

/* Format a double the OMG VM way: shortest round-trippable
 * representation, with a guaranteed `.0` suffix for whole-valued
 * floats so they don't visually collapse into ints, and lowercase
 * `nan`/`inf`/`-inf` for special cases.
 *
 * C's printf doesn't ship a shortest-round-trip primitive, so we
 * walk precision from 1 to 17 digits, preferring a decimal form. If
 * `%.<p>g` produces scientific notation (which it does for round
 * numbers like 1000 at low precisions, exponent ≥ p), we skip that
 * precision and try the next; the first decimal form that round-trips
 * wins. If no decimal form ever round-trips (very large or tiny
 * values where decimal would be unwieldy), fall back to `%.17g`'s
 * scientific notation. */
static const char *omg_float_format(double f, char *buf, size_t bufsize) {
    if (isnan(f)) { snprintf(buf, bufsize, "nan"); return buf; }
    if (isinf(f)) { snprintf(buf, bufsize, "%s", f > 0 ? "inf" : "-inf"); return buf; }
    int settled = 0;
    for (int p = 1; p <= 17; p++) {
        snprintf(buf, bufsize, "%.*g", p, f);
        if (strchr(buf, 'e') || strchr(buf, 'E')) continue;  /* prefer decimal */
        double r;
        if (sscanf(buf, "%lf", &r) == 1 && r == f) { settled = 1; break; }
    }
    if (!settled) {
        snprintf(buf, bufsize, "%.17g", f);
    }
    if (!strchr(buf, '.') && !strchr(buf, 'e') && !strchr(buf, 'E')) {
        size_t len = strlen(buf);
        if (len + 2 < bufsize) {
            buf[len] = '.';
            buf[len + 1] = '0';
            buf[len + 2] = '\0';
        }
    }
    return buf;
}

/* Forward declarations for stringification of compound values. */
static void omg_print_value(Value v);

/* stdout is line-buffered everywhere, so that output and a later error
 * on stderr come out in the order they happened (and so a long-running
 * program's progress shows through a pipe), as with the Rust runtime.
 * POSIX stdio can do that itself (setvbuf in main). The Windows C
 * runtime has no line-buffered mode, so there each emit ends with a
 * flush. */
#ifdef _WIN32
#define OMG_EMIT_DONE() fflush(stdout)
#else
#define OMG_EMIT_DONE() ((void)0)
#endif

static void omg_emit(Value v) {
    char fbuf[64];
    switch (v.tag) {
        case OMG_INT:     printf("%lld\n", (long long)v.v.i); break;
        case OMG_FLOAT:   puts(omg_float_format(v.v.f, fbuf, sizeof(fbuf))); break;
        case OMG_STR:     puts(v.v.s); break;
        case OMG_BOOL:    puts(v.v.b ? "true" : "false"); break;
        case OMG_NONE:    putchar('\n'); break;
        case OMG_CLOSURE: printf("<proc>\n"); break;
        case OMG_LIST:
        case OMG_DICT:
            omg_print_value(v);
            putchar('\n');
            break;
    }
    OMG_EMIT_DONE();
}

/* === Integer arithmetic ====================================================
 * Phase 1.5: int+int only. String/list concat for ADD, float promotion,
 * and the rest of the arithmetic surface come in later phases.
 *
 * Division and modulo use floor semantics to match the OMG VM:
 * `(-7) / 2 == -4`, `(-7) % 2 == 1`. Plain C `/` and `%` truncate
 * toward zero, so we adjust when signs disagree and the remainder
 * is non-zero.
 */
/* `+` is polymorphic in OMG: string concat for any-string operand,
 * list concat for two-list, otherwise int math. The actual definition
 * is at the end of the runtime header (phase 5) — `omg_str_concat` and
 * `omg_list_concat` are both defined down there.
 * Forward-declared here so `omg_add` can use them without rearranging
 * everything. */
static Value omg_str_concat(Value a, Value b);

/* If either operand is a float, both are promoted to double. */
static inline int omg_is_float(Value v) { return v.tag == OMG_FLOAT; }

/* Coerce `v` to int64. Mirrors `Value::as_int` in rust/src/value.rs:
 * accept ints, bools, finite in-range floats (truncated toward zero),
 * and numeric strings (parsed). Reject lists, dicts, closures, none,
 * and non-numeric strings with a TypeError so a downstream arithmetic
 * op doesn't silently treat a heap pointer as an integer. */
static inline int64_t omg_as_int(Value v) {
    switch (v.tag) {
        case OMG_INT: return v.v.i;
        case OMG_BOOL: return v.v.b ? 1 : 0;
        case OMG_FLOAT: {
            double f = v.v.f;
            if (!isfinite(f)) {
                omg_panicf("ValueError",
                           "cannot convert non-finite float to int: %g", f);
            }
            if (f < (double)INT64_MIN || f > (double)INT64_MAX) {
                omg_panicf("ValueError",
                           "float %g is outside the i64 range", f);
            }
            return (int64_t)f;   /* truncation toward zero */
        }
        case OMG_STR: {
            const char *s = v.v.s;
            char *end = NULL;
            errno = 0;
            long long n = strtoll(s, &end, 10);
            if (s == end || *end != '\0' || errno == ERANGE) {
                omg_panicf("TypeError", "Invalid literal for int(): '%s'", s);
            }
            return (int64_t)n;
        }
        case OMG_LIST:
            omg_panic("TypeError",
                      "cannot convert list to int (use length() instead)");
        case OMG_DICT:
            omg_panic("TypeError", "cannot convert dict to int");
        case OMG_CLOSURE:
            omg_panic("TypeError", "cannot convert function to int");
        case OMG_NONE:
            omg_panic("TypeError", "cannot convert none to int");
    }
    return 0; /* unreachable */
}

/* Coerce `v` to double. Same shape as omg_as_int but produces a
 * double; numeric strings (with optional decimal/exponent) parse via
 * strtod. Used by `+ - * / %` and the math builtins. */
static inline double omg_as_double(Value v) {
    switch (v.tag) {
        case OMG_INT: return (double)v.v.i;
        case OMG_FLOAT: return v.v.f;
        case OMG_BOOL: return v.v.b ? 1.0 : 0.0;
        case OMG_STR: {
            const char *s = v.v.s;
            char *end = NULL;
            errno = 0;
            double d = strtod(s, &end);
            if (s == end || *end != '\0' || errno == ERANGE) {
                omg_panicf("TypeError",
                           "Invalid literal for float(): '%s'", s);
            }
            return d;
        }
        case OMG_LIST:
            omg_panic("TypeError",
                      "cannot convert list to float (use length() instead)");
        case OMG_DICT:
            omg_panic("TypeError", "cannot convert dict to float");
        case OMG_CLOSURE:
            omg_panic("TypeError", "cannot convert function to float");
        case OMG_NONE:
            omg_panic("TypeError", "cannot convert none to float");
    }
    return 0.0; /* unreachable */
}

/* Arithmetic ops route through omg_as_int / omg_as_double so numeric
 * strings (`"255" / 16` = 15) and bools coerce the same way they do
 * in the Rust runtime. Without this, native dereferenced raw .v.i on
 * non-int operands and silently produced pointer-arithmetic garbage. */

static inline Value omg_sub(Value a, Value b) {
    if (omg_is_float(a) || omg_is_float(b)) return omg_float(omg_as_double(a) - omg_as_double(b));
    return omg_int(omg_as_int(a) - omg_as_int(b));
}
static inline Value omg_mul(Value a, Value b) {
    if (omg_is_float(a) || omg_is_float(b)) return omg_float(omg_as_double(a) * omg_as_double(b));
    return omg_int(omg_as_int(a) * omg_as_int(b));
}

/* `/` is true division: always returns a float, matching Python 3 and
 * the Rust VM. Integer operands are widened to double. Use `//` when
 * you want integer (floor) division. */
static inline Value omg_div(Value a, Value b) {
    double bd = omg_as_double(b);
    if (bd == 0.0) omg_panic("ZeroDivisionError", "integer division or modulo by zero");
    return omg_float(omg_as_double(a) / bd);
}

/* Explicit floor division. int÷int stays int; any-float returns the
 * floor as a double (matches the OMG VM's `10.5 // 3 == 3.0`). */
static inline Value omg_floor_div(Value a, Value b) {
    if (omg_is_float(a) || omg_is_float(b)) {
        double bd = omg_as_double(b);
        if (bd == 0.0) omg_panic("ZeroDivisionError", "integer division or modulo by zero");
        return omg_float(floor(omg_as_double(a) / bd));
    }
    int64_t x = omg_as_int(a), y = omg_as_int(b);
    if (y == 0) omg_panic("ZeroDivisionError", "integer division or modulo by zero");
    int64_t q = x / y;
    if ((x % y != 0) && ((x < 0) != (y < 0))) q -= 1;
    return omg_int(q);
}

static inline Value omg_mod(Value a, Value b) {
    if (omg_is_float(a) || omg_is_float(b)) {
        double bd = omg_as_double(b);
        if (bd == 0.0) omg_panic("ZeroDivisionError", "integer division or modulo by zero");
        double ad = omg_as_double(a);
        /* Python-style floor modulo: result has the sign of the divisor. */
        return omg_float(ad - floor(ad / bd) * bd);
    }
    int64_t x = omg_as_int(a), y = omg_as_int(b);
    if (y == 0) omg_panic("ZeroDivisionError", "integer division or modulo by zero");
    int64_t r = x % y;
    if (r != 0 && ((r < 0) != (y < 0))) r += y;
    return omg_int(r);
}

static inline Value omg_neg(Value a) {
    if (a.tag == OMG_FLOAT) return omg_float(-a.v.f);
    return omg_int(-omg_as_int(a));
}

/* === Comparisons ==========================================================
 * `eq`/`ne` are typed-structural: 5 == "5" is false. Ordered comparisons
 * accept str-vs-str (lex compare) or int-vs-int (numeric); other type
 * combinations are an error in OMG. Floats and cross-type numeric
 * comparisons come in a later phase.
 */
/* Forward declarations for structural compare of collections. */
static int omg_list_eq(struct OmgList *a, struct OmgList *b);
static int omg_dict_eq(struct OmgDict *a, struct OmgDict *b);

static int omg_values_equal(Value a, Value b) {
    /* Cross-type numeric: int ↔ float compare by value. Mirrors the
     * OMG VM, where `5 == 5.0` is true. */
    if (a.tag == OMG_INT && b.tag == OMG_FLOAT) return ((double)a.v.i) == b.v.f;
    if (a.tag == OMG_FLOAT && b.tag == OMG_INT) return a.v.f == ((double)b.v.i);
    if (a.tag != b.tag) return 0;
    switch (a.tag) {
        case OMG_INT:     return a.v.i == b.v.i;
        case OMG_FLOAT:   return a.v.f == b.v.f;
        case OMG_STR:     return strcmp(a.v.s, b.v.s) == 0;
        case OMG_BOOL:    return a.v.b == b.v.b;
        case OMG_NONE:    return 1;
        case OMG_CLOSURE: return a.v.c == b.v.c;
        case OMG_LIST:    return omg_list_eq(a.v.l, b.v.l);
        case OMG_DICT:    return omg_dict_eq(a.v.d, b.v.d);
    }
    return 0;
}

static inline Value omg_eq(Value a, Value b) { return omg_bool(omg_values_equal(a, b)); }
static inline Value omg_ne(Value a, Value b) { return omg_bool(!omg_values_equal(a, b)); }

static int omg_compare(Value a, Value b) {
    if (a.tag == OMG_STR && b.tag == OMG_STR) return strcmp(a.v.s, b.v.s);
    /* Numeric comparisons: anything that can be cast to a double goes
     * through a single double compare; an int operand promotes to
     * double for the test, matching the OMG VM's promote-on-float
     * rule for ordered comparisons. */
    if ((a.tag == OMG_INT || a.tag == OMG_FLOAT) &&
        (b.tag == OMG_INT || b.tag == OMG_FLOAT)) {
        double ad = a.tag == OMG_INT ? (double)a.v.i : a.v.f;
        double bd = b.tag == OMG_INT ? (double)b.v.i : b.v.f;
        return (ad > bd) - (ad < bd);
    }
    omg_panic("TypeError", "cannot order-compare these values");
    return 0; /* unreachable */
}

static inline Value omg_lt(Value a, Value b) { return omg_bool(omg_compare(a, b) <  0); }
static inline Value omg_le(Value a, Value b) { return omg_bool(omg_compare(a, b) <= 0); }
static inline Value omg_gt(Value a, Value b) { return omg_bool(omg_compare(a, b) >  0); }
static inline Value omg_ge(Value a, Value b) { return omg_bool(omg_compare(a, b) >= 0); }

/* === Bitwise / shift / logical ===========================================
 * Bitwise ops are integer-only: floats are explicitly rejected (silent
 * truncation would be a footgun, matching Rust). Strings and bools
 * coerce via omg_as_int the same way arithmetic does. */

static inline void omg_reject_float(Value v, const char *op) {
    if (omg_is_float(v)) {
        omg_panicf("TypeError", "operator '%s' is not defined for floats", op);
    }
}

static inline Value omg_band(Value a, Value b) {
    omg_reject_float(a, "&"); omg_reject_float(b, "&");
    return omg_int(omg_as_int(a) & omg_as_int(b));
}
static inline Value omg_bor (Value a, Value b) {
    omg_reject_float(a, "|"); omg_reject_float(b, "|");
    return omg_int(omg_as_int(a) | omg_as_int(b));
}
static inline Value omg_bxor(Value a, Value b) {
    omg_reject_float(a, "^"); omg_reject_float(b, "^");
    return omg_int(omg_as_int(a) ^ omg_as_int(b));
}

static inline Value omg_shl(Value a, Value b) {
    omg_reject_float(a, "<<"); omg_reject_float(b, "<<");
    int64_t ai = omg_as_int(a), bi = omg_as_int(b);
    if (bi < 0 || bi >= 64) {
        omg_panicf("ValueError", "shift count out of range: %lld", (long long)bi);
    }
    return omg_int(ai << bi);
}
static inline Value omg_shr(Value a, Value b) {
    omg_reject_float(a, ">>"); omg_reject_float(b, ">>");
    int64_t ai = omg_as_int(a), bi = omg_as_int(b);
    if (bi < 0 || bi >= 64) {
        omg_panicf("ValueError", "shift count out of range: %lld", (long long)bi);
    }
    return omg_int(ai >> bi);
}

static inline Value omg_bnot(Value a) {
    omg_reject_float(a, "~");
    return omg_int(~omg_as_int(a));
}

/* `and`/`or` are eager at this level — the OMG compiler emits explicit
 * JumpIfFalse for short-circuit semantics; the bytecode AND/OR ops just
 * combine the two pre-evaluated operands. */
static inline Value omg_and(Value a, Value b) { return omg_bool(omg_truthy(a) && omg_truthy(b)); }
static inline Value omg_or (Value a, Value b) { return omg_bool(omg_truthy(a) || omg_truthy(b)); }

/* === Strings ==============================================================
 * Phase 4: real string concat / index / slice. Heap-allocated strings
 * are leaked deliberately — phase 5+ will introduce refcounting along
 * with proper list/dict GC. UTF-8-aware where it matters: indexing and
 * slicing operate on Unicode code points (matching the OMG VM), not raw
 * bytes; ASCII strings work as you'd expect.
 */

/* Forward declaration. */
static char *omg_compound_to_cstr(Value v);

/* Stringify any Value; result lives in `buf` (for short forms) or in a
 * dedicated string's storage. Caller treats result as read-only. For
 * lists/dicts we leak a fresh malloc'd buffer, since they don't fit in
 * a fixed-size scratch. */
static const char *omg_value_to_cstr(Value v, char *buf, size_t bufsize) {
    switch (v.tag) {
        case OMG_STR:     return v.v.s;
        case OMG_INT:
            snprintf(buf, bufsize, "%lld", (long long)v.v.i);
            return buf;
        case OMG_FLOAT:   return omg_float_format(v.v.f, buf, bufsize);
        case OMG_BOOL:    return v.v.b ? "true" : "false";
        case OMG_NONE:    return "";
        case OMG_CLOSURE: return "<proc>";
        case OMG_LIST:
        case OMG_DICT:    return omg_compound_to_cstr(v);
    }
    return "";
}

/* Allocate a fresh char buffer (caller leaks it). */
static char *omg_str_alloc(size_t n) {
    char *p = (char *)malloc(n);
    if (!p) { fprintf(stderr, "out of memory\n"); exit(1); }
    return p;
}

static Value omg_str_concat(Value a, Value b) {
    char abuf[64], bbuf[64];
    const char *as = omg_value_to_cstr(a, abuf, sizeof(abuf));
    const char *bs = omg_value_to_cstr(b, bbuf, sizeof(bbuf));
    size_t la = strlen(as), lb = strlen(bs);
    char *buf = omg_str_alloc(la + lb + 1);
    memcpy(buf, as, la);
    memcpy(buf + la, bs, lb);
    buf[la + lb] = 0;
    return omg_str(buf);
}

/* Walk past one UTF-8 code point starting at `s`. Returns the byte
 * length of that code point (1..4), or 0 at end-of-string. */
static int omg_utf8_advance(const char *s) {
    unsigned char b = (unsigned char)*s;
    if (b == 0)         return 0;
    if (b < 0x80)       return 1;
    if ((b & 0xE0) == 0xC0) return 2;
    if ((b & 0xF0) == 0xE0) return 3;
    if ((b & 0xF8) == 0xF0) return 4;
    return 1;            /* malformed; advance one byte */
}

/* Count code points in a UTF-8 string. Matches `length(s)` semantics. */
static size_t omg_str_char_count(const char *s) {
    size_t n = 0;
    while (*s) {
        s += omg_utf8_advance(s);
        n++;
    }
    return n;
}

/* Compute byte offset of the `idx`-th code point. Negative `idx` counts
 * from the end. Returns -1 if out of range. */
static int omg_str_byte_offset(const char *s, int64_t idx) {
    int64_t total = (int64_t)omg_str_char_count(s);
    if (idx < 0) idx += total;
    if (idx < 0 || idx > total) return -1;
    int off = 0;
    int64_t i = 0;
    while (i < idx && s[off]) {
        off += omg_utf8_advance(s + off);
        i++;
    }
    return off;
}

static Value omg_str_index(Value base, int64_t idx) {
    int off = omg_str_byte_offset(base.v.s, idx);
    if (off < 0) {
        omg_panicf("IndexError", "index %lld out of range for length %zu",
                   (long long)idx, omg_str_char_count(base.v.s));
    }
    int len = omg_utf8_advance(base.v.s + off);
    char *buf = omg_str_alloc(len + 1);
    memcpy(buf, base.v.s + off, len);
    buf[len] = 0;
    return omg_str(buf);
}

/* Slice s[start:end]. start/end may be OMG_NONE meaning "default"
 * (0 / length). Out-of-range bounds clamp into [0, len], matching
 * Python and the OMG VM. */
static Value omg_str_slice(Value base, Value start_v, Value end_v) {
    const char *s = base.v.s;
    int64_t total = (int64_t)omg_str_char_count(s);
    int64_t start = start_v.tag == OMG_NONE ? 0     : start_v.v.i;
    int64_t end   = end_v.tag   == OMG_NONE ? total : end_v.v.i;
    if (start < 0) start += total;
    if (end   < 0) end   += total;
    if (start < 0)     start = 0;
    if (end   < 0)     end   = 0;
    if (start > total) start = total;
    if (end   > total) end   = total;
    if (end   < start) end   = start;
    int start_off = omg_str_byte_offset(s, start);
    int end_off   = omg_str_byte_offset(s, end);
    int len = end_off - start_off;
    char *buf = omg_str_alloc(len + 1);
    memcpy(buf, s + start_off, len);
    buf[len] = 0;
    return omg_str(buf);
}

/* `chr(i)` — single-character string for byte value `i` (Latin-1).
 * Codepoints 0x80..0xFF expand to 2 UTF-8 bytes (matching the OMG VM,
 * which uses Rust's `char::to_string` for the same input). */
static Value omg_chr(Value vi) {
    int64_t i = vi.v.i & 0xFF;
    if (i < 0x80) {
        char *buf = omg_str_alloc(2);
        buf[0] = (char)i;
        buf[1] = 0;
        return omg_str(buf);
    }
    char *buf = omg_str_alloc(3);
    buf[0] = (char)(0xC0 | (i >> 6));
    buf[1] = (char)(0x80 | (i & 0x3F));
    buf[2] = 0;
    return omg_str(buf);
}

/* `ascii(c)` — codepoint of a single-character string. Decodes UTF-8. */
static Value omg_ascii(Value vs) {
    const unsigned char *s = (const unsigned char *)vs.v.s;
    if (s[0] == 0) {
        omg_panic("TypeError", "ascii() expects a single character");
    }
    if (s[0] < 0x80) return omg_int(s[0]);
    if ((s[0] & 0xE0) == 0xC0) return omg_int(((s[0] & 0x1F) << 6) | (s[1] & 0x3F));
    if ((s[0] & 0xF0) == 0xE0) return omg_int(((s[0] & 0x0F) << 12) | ((s[1] & 0x3F) << 6) | (s[2] & 0x3F));
    if ((s[0] & 0xF8) == 0xF0) return omg_int(((s[0] & 0x07) << 18) | ((s[1] & 0x3F) << 12) | ((s[2] & 0x3F) << 6) | (s[3] & 0x3F));
    return omg_int(s[0]);
}

/* === Lists ===============================================================
 * Phase 5: dynamic Value array. Heap-allocated, leaked. List `+` allocates
 * a fresh list (matches OMG VM — never mutates either operand). All other
 * list operations mutate in place where the OMG VM does (e.g. STORE_INDEX). */

typedef struct OmgList {
    int rc;
    int len;
    int cap;
    Value *items;
} OmgList;

static OmgList *omg_list_alloc(int cap) {
    OmgList *l = (OmgList *)malloc(sizeof(OmgList));
    if (!l) { fprintf(stderr, "out of memory\n"); exit(1); }
    l->rc = 1;
    l->len = 0;
    l->cap = cap < 0 ? 0 : cap;
    l->items = l->cap > 0 ? (Value *)malloc(l->cap * sizeof(Value)) : NULL;
    if (l->cap > 0 && !l->items) { fprintf(stderr, "out of memory\n"); exit(1); }
    return l;
}

static int omg_list_len(OmgList *l) { return l ? l->len : 0; }

static void omg_list_push(OmgList *l, Value v) {
    if (l->len == l->cap) {
        int newcap = l->cap < 4 ? 4 : l->cap * 2;
        Value *items = (Value *)realloc(l->items, newcap * sizeof(Value));
        if (!items) { fprintf(stderr, "out of memory\n"); exit(1); }
        l->items = items;
        l->cap = newcap;
    }
    l->items[l->len++] = v;
}

static Value omg_list_new(void) {
    Value v;
    v.tag = OMG_LIST;
    v.v.l = omg_list_alloc(0);
    return v;
}

/* Build a list from `n` Values popped off the operand stack. Caller
 * passes them in source order (so items[0] is the first item). */
static Value omg_list_build(Value *items, int n) {
    Value v;
    v.tag = OMG_LIST;
    v.v.l = omg_list_alloc(n);
    for (int i = 0; i < n; i++) omg_list_push(v.v.l, items[i]);
    return v;
}

/* list_repeat(item, count) — allocate a fresh list of length `count`
 * with every slot holding the same value. The OMG-side bytecode
 * writer uses this to pre-allocate its byte buffer in O(n) rather
 * than appending one byte at a time (which is O(n²)). Each
 * inc-ref of the item is counted as a separate reference because the
 * caller's `item` value will be dec'd by the standard transfer
 * protocol on builtin return. */
static Value omg_list_repeat(Value item, Value count) {
    if (count.tag != OMG_INT) {
        omg_panic("TypeError", "list_repeat() count must be an int");
    }
    if (count.v.i < 0) {
        omg_panicf("ValueError",
                   "list_repeat() count must be non-negative, got %lld",
                   (long long)count.v.i);
    }
    int n = (int)count.v.i;
    Value v;
    v.tag = OMG_LIST;
    v.v.l = omg_list_alloc(n);
    for (int i = 0; i < n; i++) {
        omg_inc(item);
        omg_list_push(v.v.l, item);
    }
    return v;
}

/* append(list, item) — push `item` onto the end of `list` in place, in
 * amortised constant time. The builtin protocol borrows its arguments,
 * so the list takes its own reference to the item. */
static Value omg_builtin_append(Value list, Value item) {
    if (list.tag != OMG_LIST) {
        omg_panic("TypeError", "append() expects (list, any)");
    }
    omg_inc(item);
    omg_list_push(list.v.l, item);
    return omg_none();
}

/* By-reference closure cells. A "cell" is a 1-element OMG_LIST that
 * lets a parent and its closures share the same storage slot for a
 * captured local. Mirrors `Rc<RefCell<Value>>` in the Rust runtime
 * and `[v]` in the OMG VM / native-js path. omg_cell_new takes
 * ownership of `v` (the cell becomes its sole owner). */
static Value omg_cell_new(Value v) {
    Value cell;
    cell.tag = OMG_LIST;
    cell.v.l = omg_list_alloc(1);
    omg_list_push(cell.v.l, v);
    return cell;
}

static Value omg_cell_get(Value cell) {
    /* Same semantics as omg_index(cell, omg_int(0)): returns a
     * fresh ref to the contents (caller will push to stack). */
    Value v = cell.v.l->items[0];
    omg_inc(v);
    return v;
}

static void omg_cell_set(Value cell, Value v) {
    /* Transfers `v` into the cell, dec'ing the previous occupant.
     * Doesn't touch the cell's own rc — the caller still owns it. */
    omg_assign(&cell.v.l->items[0], v);
}

/* A proc entered with the wrong number of arguments. Only a call
 * through a value can get here (direct calls are checked by the
 * compiler), and that call site has just pushed a traceback frame for
 * the callee; drop it so the error is reported at the call, as on the
 * VMs. */
static void omg_arity_error(const char *name, int expected, int got) {
    omg_frame_pop();
    omg_panicf("TypeError", "function '%s' expects %d arguments, got %d",
               name, expected, got);
}

/* A LOAD or STORE of a name that nothing has been stored into yet. */
static void omg_undefined(const char *name) {
    omg_panic("UndefinedIdentError", name);
}

/* LOAD / STORE through a variable slot that holds a cell. The slot is
 * OMG_UNSET (not a cell) until its `alloc` has run, or when a closure
 * captured the name before the enclosing proc had declared it. */
static Value omg_cell_load(Value cell, const char *name) {
    if (cell.tag != OMG_LIST) omg_undefined(name);
    return omg_cell_get(cell);
}

static void omg_cell_store(Value cell, const char *name, Value v) {
    if (cell.tag != OMG_LIST) omg_undefined(name);
    omg_cell_set(cell, v);
}

/* The closure a running proc is, rebuilt from the pieces its C function
 * was called with. A nested proc that refers to itself (recursion, or
 * handing itself to someone else) gets this bound to its own name on
 * entry; the VMs do the same thing in the frame they build for a
 * closure call. The new closure shares the captured cells but owns its
 * own array, so there is no reference cycle. */
static Value omg_closure_self(OmgFn fn, const char *name, Value *captured, int cap_count) {
    Value *cap = NULL;
    if (cap_count > 0) {
        cap = (Value *)malloc((size_t)cap_count * sizeof(Value));
        if (!cap) { fprintf(stderr, "out of memory\n"); exit(1); }
        for (int i = 0; i < cap_count; i++) {
            cap[i] = captured[i];
            omg_inc(cap[i]);
        }
    }
    return omg_closure(fn, name, cap, cap_count);
}

static int64_t omg_normalise_index(int64_t idx, int len, const char *opname) {
    int64_t real = idx < 0 ? idx + len : idx;
    if (real < 0 || real >= len) {
        (void)opname;
        omg_panicf("IndexError", "index %lld out of range for length %d",
                   (long long)idx, len);
    }
    return real;
}

static Value omg_list_index(OmgList *l, int64_t idx) {
    int64_t real = omg_normalise_index(idx, l->len, "list");
    /* Caller is going to take ownership (push to stack). The list
     * still owns its slot, so inc to give the caller a fresh ref. */
    Value v = l->items[real];
    omg_inc(v);
    return v;
}

static void omg_list_set_index(OmgList *l, int64_t idx, Value v) {
    int64_t real = omg_normalise_index(idx, l->len, "list");
    /* Transferred ownership: dec the old occupant before overwriting. */
    omg_assign(&l->items[real], v);
}

static Value omg_list_concat(OmgList *a, OmgList *b) {
    Value v;
    v.tag = OMG_LIST;
    v.v.l = omg_list_alloc(a->len + b->len);
    /* The result list takes its own references to each element; both
     * source lists keep theirs (a + b never mutates either operand). */
    for (int i = 0; i < a->len; i++) {
        omg_inc(a->items[i]);
        omg_list_push(v.v.l, a->items[i]);
    }
    for (int i = 0; i < b->len; i++) {
        omg_inc(b->items[i]);
        omg_list_push(v.v.l, b->items[i]);
    }
    return v;
}

static int omg_list_eq(OmgList *a, OmgList *b) {
    if (a == b) return 1;
    if (!a || !b) return 0;
    if (a->len != b->len) return 0;
    for (int i = 0; i < a->len; i++) {
        if (!omg_values_equal(a->items[i], b->items[i])) return 0;
    }
    return 1;
}

/* === Dicts ===============================================================
 * Phase 5: parallel keys[]/vals[] arrays, linear lookup. Plenty fast for
 * the OMG corpus (dicts tend to be small) and trivially correct. The
 * `frozen` flag is set by `freeze()` and forbids further mutation. */

typedef struct OmgDict {
    int rc;
    int len;
    int cap;
    char **keys;
    uint32_t *hashes;   /* FNV-1a of each key — strcmp gated by hash match */
    Value *vals;
    int frozen;
} OmgDict;

/* FNV-1a 32-bit. Hot path: vm.omg's local-env lookup runs this on
 * variable names per LOAD; keep it inlineable and branch-free. */
static inline uint32_t omg_str_hash(const char *s) {
    uint32_t h = 2166136261u;
    while (*s) {
        h ^= (unsigned char)*s++;
        h *= 16777619u;
    }
    return h;
}

static OmgDict *omg_dict_alloc(void) {
    OmgDict *d = (OmgDict *)malloc(sizeof(OmgDict));
    if (!d) { fprintf(stderr, "out of memory\n"); exit(1); }
    d->rc = 1;
    d->len = 0;
    d->cap = 0;
    d->keys = NULL;
    d->hashes = NULL;
    d->vals = NULL;
    d->frozen = 0;
    return d;
}

static int omg_dict_len(OmgDict *d) { return d ? d->len : 0; }

/* === Refcount helpers (definitions) =====================================
 * Forward-declared near the top; defined here, after every heap struct
 * (OmgClosure / OmgList / OmgDict) is fully visible.
 */
static void omg_inc(Value v) {
    switch (v.tag) {
        case OMG_LIST:    v.v.l->rc++; break;
        case OMG_DICT:    v.v.d->rc++; break;
        case OMG_CLOSURE: v.v.c->rc++; break;
        default: break;
    }
}

/* Decrement rc for refcounted heap values; on zero, recursively
 * release nested references and free the storage. No-op for
 * unrefcounted tags (int/bool/none/str). */
static void omg_dec(Value v) {
    switch (v.tag) {
        case OMG_LIST: {
            OmgList *l = v.v.l;
            if (--l->rc == 0) {
                for (int i = 0; i < l->len; i++) omg_dec(l->items[i]);
                free(l->items);
                free(l);
            }
            break;
        }
        case OMG_DICT: {
            OmgDict *d = v.v.d;
            if (--d->rc == 0) {
                for (int i = 0; i < d->len; i++) {
                    free(d->keys[i]);
                    omg_dec(d->vals[i]);
                }
                free(d->keys);
                free(d->hashes);
                free(d->vals);
                free(d);
            }
            break;
        }
        case OMG_CLOSURE: {
            OmgClosure *c = v.v.c;
            if (--c->rc == 0) {
                for (int i = 0; i < c->cap_count; i++) omg_dec(c->captured[i]);
                free(c->captured);
                free(c);
            }
            break;
        }
        default: break;
    }
}

/* Assign `v` into `*slot`, dec'ing the old occupant. The new value is
 * transferred — caller has already given up its reference. Used for
 * every STORE/STORE_LOCAL/list-elem/dict-value/captured-slot write. */
static void omg_assign(Value *slot, Value v) {
    Value old = *slot;
    *slot = v;
    omg_dec(old);
}

/* Hash-gated find: comparing 4 bytes of hash before strcmp filters out
 * 99%+ of mismatches in one branch, so larger dicts (procs with many
 * locals, big global tables) no longer pay full strcmp per slot. */
static int omg_dict_find(OmgDict *d, const char *key) {
    uint32_t h = omg_str_hash(key);
    for (int i = 0; i < d->len; i++) {
        if (d->hashes[i] == h && strcmp(d->keys[i], key) == 0) return i;
    }
    return -1;
}

/* Variant taking the precomputed hash. omg_dict_set already hashed the
 * key for the find, so the same value can be reused for the eventual
 * insert without re-hashing. */
static int omg_dict_find_h(OmgDict *d, const char *key, uint32_t h) {
    for (int i = 0; i < d->len; i++) {
        if (d->hashes[i] == h && strcmp(d->keys[i], key) == 0) return i;
    }
    return -1;
}

static void omg_dict_grow(OmgDict *d) {
    if (d->len < d->cap) return;
    int newcap = d->cap < 4 ? 4 : d->cap * 2;
    char **nk = (char **)realloc(d->keys, newcap * sizeof(char *));
    uint32_t *nh = (uint32_t *)realloc(d->hashes, newcap * sizeof(uint32_t));
    Value *nv = (Value *)realloc(d->vals, newcap * sizeof(Value));
    if (!nk || !nh || !nv) { fprintf(stderr, "out of memory\n"); exit(1); }
    d->keys = nk;
    d->hashes = nh;
    d->vals = nv;
    d->cap = newcap;
}

static void omg_dict_set(OmgDict *d, const char *key, Value v) {
    uint32_t h = omg_str_hash(key);
    int idx = omg_dict_find_h(d, key, h);
    if (idx >= 0) {
        if (d->frozen) {
            omg_panic("FrozenWriteError", "Imported modules are read-only");
        }
        /* Transferred ownership: dec the old value, install the new one. */
        omg_assign(&d->vals[idx], v);
        return;
    }
    if (d->frozen) {
        omg_panic("FrozenWriteError", "Imported modules are read-only");
    }
    omg_dict_grow(d);
    /* keys are owned: copy so caller's storage can be temporary. */
    size_t klen = strlen(key);
    char *kc = (char *)malloc(klen + 1);
    if (!kc) { fprintf(stderr, "out of memory\n"); exit(1); }
    memcpy(kc, key, klen + 1);
    d->keys[d->len] = kc;
    d->hashes[d->len] = h;
    d->vals[d->len] = v;  /* transferred — no inc */
    d->len++;
}

static Value omg_dict_get(OmgDict *d, const char *key) {
    int idx = omg_dict_find(d, key);
    if (idx < 0) {
        omg_panicf("KeyError", "\"Key '%s' not found\"", key);
    }
    /* Caller takes ownership; dict's slot keeps its own. */
    Value v = d->vals[idx];
    omg_inc(v);
    return v;
}

static Value omg_dict_freeze(Value v) {
    if (v.tag != OMG_DICT) {
        omg_panic("TypeError", "freeze() expects a dict");
    }
    OmgDict *src = v.v.d;
    OmgDict *dst = omg_dict_alloc();
    for (int i = 0; i < src->len; i++) {
        /* dict_set takes ownership of the value, so inc src's so it
         * stays alive after freeze() returns. */
        omg_inc(src->vals[i]);
        omg_dict_set(dst, src->keys[i], src->vals[i]);
    }
    dst->frozen = 1;
    Value out;
    out.tag = OMG_DICT;
    out.v.d = dst;
    return out;
}

static int omg_dict_eq(OmgDict *a, OmgDict *b) {
    if (a == b) return 1;
    if (!a || !b) return 0;
    if (a->len != b->len) return 0;
    for (int i = 0; i < a->len; i++) {
        int j = omg_dict_find(b, a->keys[i]);
        if (j < 0) return 0;
        if (!omg_values_equal(a->vals[i], b->vals[j])) return 0;
    }
    return 1;
}

/* dict_keys(d) — returns a list of the dict's keys as strings.
 * Mirrors the runtime builtin.
 *
 * Each returned string is a *fresh heap copy* of the dict's key, not
 * an alias. The dict frees its `keys[i]` storage when the dict's
 * refcount drops to zero (see OMG_DICT in omg_dec); if we returned
 * aliases the list's OMG_STR pointers would dangle as soon as the
 * dict went out of scope — and OMG_STR has no refcount, so list-held
 * strings can outlive their source. The cost is one strdup per key
 * per call, which leaks per the rest of omg_rt.h's string story
 * (strings are immutable, malloc'd, never explicitly freed). */
/* has_key(d, k) -> bool. Non-throwing key probe; the OMG-on-OMG VM's
 * lookup hot path used to call `try { d[k] }` per access. The key is
 * stringified (matching OMG's d[i] dict-index behaviour for int keys),
 * and non-dict containers cleanly return false (so `is_vm_none` /
 * `is_vm_closure` can probe arbitrary values without a try/except
 * wrapper). */
static Value omg_has_key(Value v, Value k) {
    Value r;
    r.tag = OMG_BOOL;
    if (v.tag != OMG_DICT) {
        r.v.b = 0;
        return r;
    }
    char kbuf[64];
    const char *ks = omg_value_to_cstr(k, kbuf, sizeof(kbuf));
    r.v.b = (omg_dict_find(v.v.d, ks) >= 0) ? 1 : 0;
    return r;
}

static Value omg_dict_keys(Value v) {
    if (v.tag != OMG_DICT) {
        omg_panic("TypeError", "dict_keys() expects a dict");
    }
    OmgDict *d = v.v.d;
    Value out;
    out.tag = OMG_LIST;
    out.v.l = omg_list_alloc(d->len);
    for (int i = 0; i < d->len; i++) {
        size_t klen = strlen(d->keys[i]);
        char *kc = (char *)malloc(klen + 1);
        if (!kc) { fprintf(stderr, "out of memory\n"); exit(1); }
        memcpy(kc, d->keys[i], klen + 1);
        Value s;
        s.tag = OMG_STR;
        s.v.s = kc;
        omg_list_push(out.v.l, s);
    }
    return out;
}

/* === Print compound values ============================================== */

/* Containers on the path from the value being printed down to the one
 * currently being visited. A container found on its own path is a
 * cycle and prints as `[...]` / `{...}` instead of recursing until the
 * stack runs out; one that merely appears twice (`[a, a]`) is not on
 * the path the second time and prints in full. Mirrors the `seen` set
 * in the Rust VM's Value::to_string. */
#define OMG_PRINT_MAX_DEPTH 256
static const void *omg_print_path[OMG_PRINT_MAX_DEPTH];
static int omg_print_depth = 0;

static int omg_print_enter(const void *p) {
    for (int i = 0; i < omg_print_depth; i++) {
        if (omg_print_path[i] == p) return 0;
    }
    /* Absurdly deep nesting is cut off the same way a cycle is. */
    if (omg_print_depth >= OMG_PRINT_MAX_DEPTH) return 0;
    omg_print_path[omg_print_depth++] = p;
    return 1;
}

/* Where a printed value goes: a stdio stream, or (fp == NULL) a heap
 * buffer that grows as needed. One writer for both keeps `emit xs` and
 * `"" + xs` identical, and needs nothing beyond standard C (the old
 * version used POSIX open_memstream, which Windows does not have). */
typedef struct {
    FILE *fp;
    char *p;
    size_t len, cap;
} OmgOut;

static void omg_out_write(OmgOut *o, const char *s, size_t n) {
    if (o->fp) { fwrite(s, 1, n, o->fp); return; }
    if (o->len + n + 1 > o->cap) {
        size_t cap = o->cap ? o->cap : 64;
        while (o->len + n + 1 > cap) cap *= 2;
        char *np = (char *)realloc(o->p, cap);
        if (!np) { fprintf(stderr, "out of memory\n"); exit(1); }
        o->p = np;
        o->cap = cap;
    }
    memcpy(o->p + o->len, s, n);
    o->len += n;
    o->p[o->len] = 0;
}

static void omg_out_puts(OmgOut *o, const char *s) {
    omg_out_write(o, s, strlen(s));
}

static void omg_out_value(OmgOut *out, Value v) {
    char buf[64];
    switch (v.tag) {
        case OMG_INT:
            snprintf(buf, sizeof(buf), "%lld", (long long)v.v.i);
            omg_out_puts(out, buf);
            break;
        case OMG_FLOAT:   omg_out_puts(out, omg_float_format(v.v.f, buf, sizeof(buf))); break;
        case OMG_STR:     omg_out_puts(out, v.v.s); break;
        case OMG_BOOL:    omg_out_puts(out, v.v.b ? "true" : "false"); break;
        case OMG_NONE:    break;
        case OMG_CLOSURE: omg_out_puts(out, "<proc>"); break;
        case OMG_LIST: {
            if (!omg_print_enter(v.v.l)) { omg_out_puts(out, "[...]"); break; }
            omg_out_write(out, "[", 1);
            for (int i = 0; i < v.v.l->len; i++) {
                if (i) omg_out_write(out, ", ", 2);
                omg_out_value(out, v.v.l->items[i]);
            }
            omg_out_write(out, "]", 1);
            omg_print_depth--;
            break;
        }
        case OMG_DICT: {
            if (!omg_print_enter(v.v.d)) { omg_out_puts(out, "{...}"); break; }
            omg_out_write(out, "{", 1);
            for (int i = 0; i < v.v.d->len; i++) {
                if (i) omg_out_write(out, ", ", 2);
                omg_out_puts(out, v.v.d->keys[i]);
                omg_out_write(out, ": ", 2);
                omg_out_value(out, v.v.d->vals[i]);
            }
            omg_out_write(out, "}", 1);
            omg_print_depth--;
            break;
        }
    }
}

static void omg_fprint_value(FILE *out, Value v) {
    OmgOut o = {out, NULL, 0, 0};
    omg_out_value(&o, v);
}

static void omg_print_value(Value v) {
    omg_fprint_value(stdout, v);
}

/* Stringify a list/dict for use in a string-concat context. Allocates a
 * fresh malloc'd buffer; leaked along with the rest of phase-5 storage. */
static char *omg_compound_to_cstr(Value v) {
    OmgOut o = {NULL, NULL, 0, 0};
    omg_out_write(&o, "", 0);   /* allocate, so the result is never NULL */
    omg_out_value(&o, v);
    return o.p;
}

/* === Polymorphic operators (extended) ==================================== */

/* Update INDEX dispatch (was string-only in phase 4). */
#undef omg_index
static Value omg_index(Value base, Value idx) {
    if (base.tag == OMG_STR && idx.tag == OMG_INT) return omg_str_index(base, idx.v.i);
    if (base.tag == OMG_LIST && idx.tag == OMG_INT) return omg_list_index(base.v.l, idx.v.i);
    if (base.tag == OMG_DICT) {
        char buf[64];
        const char *k = omg_value_to_cstr(idx, buf, sizeof(buf));
        return omg_dict_get(base.v.d, k);
    }
    {
        /* Same wording as the Rust VM ("<value> is not indexable"). */
        char vbuf[64];
        omg_panicf("TypeError", "%s is not indexable", omg_value_to_cstr(base, vbuf, sizeof(vbuf)));
    }
    return omg_none(); /* unreachable */
}

/* Update SLICE dispatch (was string-only in phase 4). */
static Value omg_list_slice(OmgList *l, Value start_v, Value end_v) {
    int total = l->len;
    int64_t start = start_v.tag == OMG_NONE ? 0     : start_v.v.i;
    int64_t end   = end_v.tag   == OMG_NONE ? total : end_v.v.i;
    if (start < 0) start += total;
    if (end   < 0) end   += total;
    if (start < 0)     start = 0;
    if (end   < 0)     end   = 0;
    if (start > total) start = total;
    if (end   > total) end   = total;
    if (end   < start) end   = start;
    Value out;
    out.tag = OMG_LIST;
    out.v.l = omg_list_alloc((int)(end - start));
    /* Slice doesn't mutate `l`. The new list takes its own refs. */
    for (int64_t i = start; i < end; i++) {
        omg_inc(l->items[i]);
        omg_list_push(out.v.l, l->items[i]);
    }
    return out;
}

#undef omg_slice
static Value omg_slice(Value base, Value start, Value end) {
    if (base.tag == OMG_STR)  return omg_str_slice(base, start, end);
    if (base.tag == OMG_LIST) return omg_list_slice(base.v.l, start, end);
    omg_panic("TypeError", "cannot slice this value");
    return omg_none(); /* unreachable */
}

/* STORE_INDEX dispatch — list[i] := v or dict[k] := v. */
static void omg_store_index(Value base, Value idx, Value v) {
    if (base.tag == OMG_LIST && idx.tag == OMG_INT) {
        omg_list_set_index(base.v.l, idx.v.i, v);
        return;
    }
    if (base.tag == OMG_DICT) {
        char buf[64];
        const char *k = omg_value_to_cstr(idx, buf, sizeof(buf));
        omg_dict_set(base.v.d, k, v);
        return;
    }
    omg_panic("TypeError", "cannot index-assign this value");
}

/* ATTR / STORE_ATTR — only meaningful on dicts. */
static Value omg_attr(Value base, const char *name) {
    if (base.tag == OMG_DICT) return omg_dict_get(base.v.d, name);
    omg_panicf("TypeError", "%s has no attribute '%s'",
               base.tag == OMG_STR ? "string" : "value", name);
    return omg_none(); /* unreachable */
}

static void omg_store_attr(Value base, const char *name, Value v) {
    if (base.tag == OMG_DICT) {
        omg_dict_set(base.v.d, name, v);
        return;
    }
    omg_panicf("TypeError", "cannot set attribute '%s' on this value", name);
}

/* `length(x)` — extended for lists. (Dicts are deliberately *not*
 * supported, matching the OMG runtime's behaviour: `length` rejects
 * dicts so users have to be explicit. Use `length(dict_keys(d))` if
 * you actually want a dict's size.) */
#undef omg_length
static Value omg_length(Value v) {
    switch (v.tag) {
        case OMG_STR:  return omg_int((int64_t)omg_str_char_count(v.v.s));
        case OMG_LIST: return omg_int((int64_t)v.v.l->len);
        case OMG_DICT: return omg_int((int64_t)omg_dict_len(v.v.d));
        default: break;
    }
    omg_panic("TypeError", "length() expects list, string or dict (type mismatch)");
    return omg_none(); /* unreachable */
}

/* Extend `+`: list + list → fresh list. (String concat already handled.) */
#undef omg_add
static inline Value omg_add(Value a, Value b) {
    if (a.tag == OMG_STR  || b.tag == OMG_STR)  return omg_str_concat(a, b);
    if (a.tag == OMG_LIST && b.tag == OMG_LIST) return omg_list_concat(a.v.l, b.v.l);
    if (omg_is_float(a) || omg_is_float(b)) return omg_float(omg_as_double(a) + omg_as_double(b));
    return omg_int(a.v.i + b.v.i);
}

/* === Numeric / math builtins ============================================= */

/* int(x) — truncate float toward zero, parse string, pass-through int. */
static Value omg_builtin_int(Value v) {
    switch (v.tag) {
        case OMG_INT:  return v;
        case OMG_BOOL: return omg_int(v.v.b ? 1 : 0);
        case OMG_FLOAT: {
            if (!isfinite(v.v.f)) {
                omg_panic("ValueError", "cannot convert non-finite float to int");
            }
            return omg_int((int64_t)v.v.f);
        }
        case OMG_STR: {
            char *end;
            long long val = strtoll(v.v.s, &end, 10);
            if (*end != '\0') {
                omg_panicf("TypeError", "Invalid literal for int(): '%s'", v.v.s);
            }
            return omg_int((int64_t)val);
        }
        case OMG_NONE: return omg_int(0);
        default:
            omg_panic("TypeError", "cannot convert to int");
            return omg_none(); /* unreachable */
    }
}

/* float(x) — widen int, parse string, pass-through float. */
static Value omg_builtin_float(Value v) {
    switch (v.tag) {
        case OMG_FLOAT: return v;
        case OMG_INT:   return omg_float((double)v.v.i);
        case OMG_BOOL:  return omg_float(v.v.b ? 1.0 : 0.0);
        case OMG_STR: {
            char *end;
            double val = strtod(v.v.s, &end);
            if (*end != '\0') {
                omg_panicf("TypeError", "Invalid literal for float(): '%s'", v.v.s);
            }
            return omg_float(val);
        }
        case OMG_NONE: return omg_float(0.0);
        default:
            omg_panic("TypeError", "cannot convert to float");
            return omg_none(); /* unreachable */
    }
}

/* floor / ceil / round return int. round uses banker's rounding (ties
 * to even) to match Python 3 / the OMG VM. */
static Value omg_builtin_floor(Value v) {
    if (v.tag == OMG_INT) return v;
    if (v.tag == OMG_FLOAT) return omg_int((int64_t)floor(v.v.f));
    omg_panic("TypeError", "floor() expects one number"); return omg_none();
}
static Value omg_builtin_ceil(Value v) {
    if (v.tag == OMG_INT) return v;
    if (v.tag == OMG_FLOAT) return omg_int((int64_t)ceil(v.v.f));
    omg_panic("TypeError", "ceil() expects one number"); return omg_none();
}
static Value omg_builtin_round(Value v) {
    if (v.tag == OMG_INT) return v;
    if (v.tag == OMG_FLOAT) {
        double f = v.v.f;
        double r = round(f);                    /* half away from zero */
        double down = trunc(f);
        /* A tie is a fractional part of *exactly* one half, which is
         * exactly representable, so no tolerance: with one,
         * round(1.4999999999) came out as 2. */
        if (fabs(f - down) == 0.5) {
            double up = f >= 0.0 ? down + 1.0 : down - 1.0;
            r = (fmod(down, 2.0) == 0.0) ? down : up;
        }
        return omg_int((int64_t)r);
    }
    omg_panic("TypeError", "round() expects one number"); return omg_none();
}
static Value omg_builtin_abs(Value v) {
    if (v.tag == OMG_INT)   return omg_int(v.v.i < 0 ? -v.v.i : v.v.i);
    if (v.tag == OMG_FLOAT) return omg_float(fabs(v.v.f));
    omg_panic("TypeError", "abs() expects one number"); return omg_none();
}

/* Math kit — always return float. */
static Value omg_builtin_sqrt(Value v) {
    double x = omg_as_double(v);
    if (x < 0.0) omg_panic("ValueError", "sqrt() of a negative number");
    return omg_float(sqrt(x));
}
static Value omg_builtin_pow(Value a, Value b) {
    /* int**non_negative_int returns int (overflow-checked); else float. */
    if (a.tag == OMG_INT && b.tag == OMG_INT && b.v.i >= 0 && b.v.i <= 62) {
        int64_t base = a.v.i, exp = b.v.i, r = 1;
        for (int64_t k = 0; k < exp; k++) r *= base;  /* may overflow silently — phase 6 simplifies */
        return omg_int(r);
    }
    return omg_float(pow(omg_as_double(a), omg_as_double(b)));
}
static Value omg_builtin_log(Value v) {
    double x = omg_as_double(v);
    if (x <= 0.0) omg_panic("ValueError", "log() requires a positive number");
    return omg_float(log(x));
}
static Value omg_builtin_sin(Value v) { return omg_float(sin(omg_as_double(v))); }
static Value omg_builtin_cos(Value v) { return omg_float(cos(omg_as_double(v))); }
static Value omg_builtin_tan(Value v) { return omg_float(tan(omg_as_double(v))); }

/* === Long-tail conversion builtins ======================================= */

/* hex(i) -> "ff" / "-1" (matches Rust's `{:x}`). */
static Value omg_builtin_hex(Value v) {
    if (v.tag != OMG_INT) omg_panic("TypeError", "hex() expects one integer (arity mismatch)");
    char buf[32];
    if (v.v.i < 0) {
        snprintf(buf, sizeof(buf), "-%llx", (long long)(-v.v.i));
    } else {
        snprintf(buf, sizeof(buf), "%llx", (long long)v.v.i);
    }
    char *out = omg_str_alloc(strlen(buf) + 1);
    memcpy(out, buf, strlen(buf) + 1);
    return omg_str(out);
}

/* binary(n[, width]) -> binary string. With width, mask & zero-pad. */
static Value omg_binary_format(int64_t n, int has_width, int64_t width) {
    if (has_width) {
        if (width <= 0) omg_panic("ValueError", "binary() width must be positive");
        if (width > 4096) omg_panicf("ValueError", "binary() width %lld is too large (max 4096)", (long long)width);
        /* Two's-complement bits of n, truncated to `width` and
         * zero-padded. Values are 64-bit: shifting by 64 or more is
         * undefined in C, so bits past 63 are written as zeros. */
        uint64_t masked = (uint64_t)n;
        if (width < 64) masked &= (((uint64_t)1 << width) - 1);
        char *buf = omg_str_alloc((size_t)width + 1);
        for (int64_t i = 0; i < width; i++) {
            buf[width - 1 - i] = (i < 64 && ((masked >> i) & 1)) ? '1' : '0';
        }
        buf[width] = 0;
        return omg_str(buf);
    }
    /* No width: print as Rust's `{:b}` (no leading zeros, but `-` for negatives). */
    int negative = n < 0;
    uint64_t u = negative ? (uint64_t)(-n) : (uint64_t)n;
    if (u == 0) {
        char *buf = omg_str_alloc(2); buf[0] = '0'; buf[1] = 0;
        return omg_str(buf);
    }
    char tmp[65];
    int len = 0;
    while (u > 0) { tmp[len++] = (u & 1) ? '1' : '0'; u >>= 1; }
    char *buf = omg_str_alloc((size_t)len + (negative ? 2 : 1));
    int o = 0;
    if (negative) buf[o++] = '-';
    for (int i = 0; i < len; i++) buf[o++] = tmp[len - 1 - i];
    buf[o] = 0;
    return omg_str(buf);
}

static Value omg_builtin_binary1(Value v) {
    if (v.tag != OMG_INT) omg_panic("TypeError", "binary() expects one or two integers (arity mismatch)");
    return omg_binary_format(v.v.i, 0, 0);
}
static Value omg_builtin_binary2(Value n, Value w) {
    if (n.tag != OMG_INT || w.tag != OMG_INT) {
        omg_panic("TypeError", "binary() expects one or two integers (arity mismatch)");
    }
    return omg_binary_format(n.v.i, 1, w.v.i);
}

/* string_bytes(s) -> list of UTF-8 byte values. */
static Value omg_builtin_string_bytes(Value v) {
    if (v.tag != OMG_STR) omg_panic("TypeError", "string_bytes() expects a string");
    size_t n = strlen(v.v.s);
    Value out;
    out.tag = OMG_LIST;
    out.v.l = omg_list_alloc((int)n);
    for (size_t i = 0; i < n; i++) {
        omg_list_push(out.v.l, omg_int((unsigned char)v.v.s[i]));
    }
    return out;
}

/* bytes_to_string([bytes]) -> string. */
static Value omg_builtin_bytes_to_string(Value v) {
    if (v.tag != OMG_LIST) omg_panic("TypeError", "bytes_to_string() expects a list of bytes");
    OmgList *l = v.v.l;
    char *buf = omg_str_alloc((size_t)l->len + 1);
    for (int i = 0; i < l->len; i++) {
        if (l->items[i].tag != OMG_INT || l->items[i].v.i < 0 || l->items[i].v.i > 255) {
            omg_panic("TypeError", "bytes_to_string() expects a list of bytes (0-255)");
        }
        buf[i] = (char)(unsigned char)l->items[i].v.i;
    }
    buf[l->len] = 0;
    return omg_str(buf);
}

/* float_bits("3.14") -> i64 of IEEE-754 bits. */
static Value omg_builtin_float_bits(Value v) {
    if (v.tag != OMG_STR) omg_panic("TypeError", "float_bits() expects a numeric string");
    char *end;
    double f = strtod(v.v.s, &end);
    /* Skip trailing whitespace as a courtesy (matches Rust's str::parse). */
    while (*end == ' ' || *end == '\t' || *end == '\n' || *end == '\r') end++;
    if (*end != '\0') {
        omg_panicf("ValueError", "float_bits(): invalid literal '%s'", v.v.s);
    }
    int64_t bits;
    memcpy(&bits, &f, sizeof(double));
    return omg_int(bits);
}

/* bits_to_float(i64) -> f64. */
static Value omg_builtin_bits_to_float(Value v) {
    if (v.tag != OMG_INT) omg_panic("TypeError", "bits_to_float() expects an integer");
    return omg_float_from_bits(v.v.i);
}

/* === panic / raise builtins ============================================== */

static Value omg_builtin_panic(Value v) {
    if (v.tag != OMG_STR) omg_panic("TypeError", "panic() expects a string (type mismatch)");
    omg_panic("RuntimeError", v.v.s);
    return omg_none(); /* unreachable */
}

static Value omg_builtin_raise(Value v) {
    if (v.tag != OMG_STR) omg_panic("TypeError", "raise() expects a string (type mismatch)");
    omg_panic("RuntimeError", v.v.s);
    return omg_none(); /* unreachable */
}

/* Print msg to stderr verbatim (no kind prefix) and exit 1. Used by
 * vm.omg to surface an already-formatted error string at top level
 * without `panic`'s "RuntimeError:" wrapper. */
static Value omg_builtin_exit_with_error(Value v) {
    if (v.tag != OMG_STR) omg_panic("TypeError", "exit_with_error() expects a string");
    fflush(stdout);
    fprintf(stderr, "%s\n", v.v.s);
    fflush(stderr);
    exit(1);
    return omg_none(); /* unreachable */
}

/* exit(code): exit the current process with the given status code.
 * Used by the OMG-native `omg` driver to propagate child
 * exit codes from subprocess() back up to the shell. */
static Value omg_builtin_exit(Value v) {
    if (v.tag != OMG_INT) omg_panic("TypeError", "exit() expects an integer");
    fflush(stdout);
    fflush(stderr);
    exit((int)v.v.i);
    return omg_none(); /* unreachable */
}

/* === Platform layer =======================================================
 * The operating-system calls the builtins below are written against.
 * Each helper has a POSIX and a Windows body and reports failure through
 * its return value (and errno where that makes sense), never by raising,
 * so the builtins keep one copy of their argument checks and messages.
 *
 * On Windows every path and argument crosses the boundary as UTF-8 on
 * the OMG side and UTF-16 on the system side, files are always opened
 * in binary mode (no "\n" <-> "\r\n" translation, as in the Rust
 * runtime), and paths are returned the way the system spells them.
 */
#ifdef _WIN32
/* UTF-8 -> UTF-16. Caller frees. NULL on invalid input / out of memory. */
static wchar_t *omg_os_wide(const char *s) {
    int n = MultiByteToWideChar(CP_UTF8, 0, s, -1, NULL, 0);
    if (n <= 0) return NULL;
    wchar_t *w = (wchar_t *)malloc((size_t)n * sizeof(wchar_t));
    if (!w) return NULL;
    if (MultiByteToWideChar(CP_UTF8, 0, s, -1, w, n) <= 0) { free(w); return NULL; }
    return w;
}

/* UTF-16 -> UTF-8. Caller owns the result. NULL on failure. */
static char *omg_os_utf8(const wchar_t *w) {
    int n = WideCharToMultiByte(CP_UTF8, 0, w, -1, NULL, 0, NULL, NULL);
    if (n <= 0) return NULL;
    char *s = (char *)malloc((size_t)n);
    if (!s) return NULL;
    if (WideCharToMultiByte(CP_UTF8, 0, w, -1, s, n, NULL, NULL) <= 0) { free(s); return NULL; }
    return s;
}

/* Attributes of `path`, or INVALID_FILE_ATTRIBUTES. */
static DWORD omg_os_attrs(const char *path) {
    wchar_t *w = omg_os_wide(path);
    if (!w) return INVALID_FILE_ATTRIBUTES;
    DWORD a = GetFileAttributesW(w);
    free(w);
    return a;
}
#endif

/* The process's working directory as a heap string; "." if unknown. */
static const char *omg_os_getcwd(void) {
#ifdef _WIN32
    DWORD n = GetCurrentDirectoryW(0, NULL);
    if (n > 0) {
        wchar_t *w = (wchar_t *)malloc((size_t)n * sizeof(wchar_t));
        if (w && GetCurrentDirectoryW(n, w) > 0) {
            char *s = omg_os_utf8(w);
            free(w);
            if (s) return s;
        } else {
            free(w);
        }
    }
    return ".";
#else
    char *cwdbuf = (char *)malloc(4096);
    if (cwdbuf && getcwd(cwdbuf, 4096)) return cwdbuf;
    free(cwdbuf);
    return ".";
#endif
}

/* fopen. `mode` is one of r, w, a, rb, wb, ab, rb+, wb+. */
static FILE *omg_os_fopen(const char *path, const char *mode) {
#ifdef _WIN32
    /* Always binary: text mode would rewrite line endings. */
    const wchar_t *wm = L"rb";
    if (mode[0] == 'w') wm = strchr(mode, '+') ? L"wb+" : L"wb";
    else if (mode[0] == 'a') wm = L"ab";
    else if (strchr(mode, '+')) wm = L"rb+";
    wchar_t *w = omg_os_wide(path);
    if (!w) { errno = EINVAL; return NULL; }
    FILE *fp = _wfopen(w, wm);
    free(w);
    return fp;
#else
    return fopen(path, mode);
#endif
}

static int omg_os_exists(const char *path) {
#ifdef _WIN32
    return omg_os_attrs(path) != INVALID_FILE_ATTRIBUTES;
#else
    struct stat st;
    return stat(path, &st) == 0;
#endif
}

static int omg_os_is_dir(const char *path) {
#ifdef _WIN32
    DWORD a = omg_os_attrs(path);
    return a != INVALID_FILE_ATTRIBUTES && (a & FILE_ATTRIBUTE_DIRECTORY) != 0;
#else
    struct stat st;
    if (stat(path, &st) != 0) return 0;
    return (st.st_mode & S_IFMT) == S_IFDIR;
#endif
}

/* Size in bytes and time of the last write (ms since 1970) of `path`.
 * 0 on success; -1 with errno set otherwise. */
static int omg_os_stat(const char *path, int64_t *size, int64_t *mtime_ms) {
#ifdef _WIN32
    wchar_t *w = omg_os_wide(path);
    if (!w) { errno = EINVAL; return -1; }
    WIN32_FILE_ATTRIBUTE_DATA d;
    BOOL ok = GetFileAttributesExW(w, GetFileExInfoStandard, &d);
    free(w);
    if (!ok) { errno = ENOENT; return -1; }
    *size = (int64_t)(((uint64_t)d.nFileSizeHigh << 32) | (uint64_t)d.nFileSizeLow);
    uint64_t t = ((uint64_t)d.ftLastWriteTime.dwHighDateTime << 32) | (uint64_t)d.ftLastWriteTime.dwLowDateTime;
    *mtime_ms = (int64_t)((t - 116444736000000000ULL) / 10000ULL);
    return 0;
#else
    struct stat st;
    if (stat(path, &st) != 0) return -1;
    *size = (int64_t)st.st_size;
#ifdef __APPLE__
    *mtime_ms = (int64_t)st.st_mtimespec.tv_sec * 1000 + (int64_t)(st.st_mtimespec.tv_nsec / 1000000);
#else
    *mtime_ms = (int64_t)st.st_mtim.tv_sec * 1000 + (int64_t)(st.st_mtim.tv_nsec / 1000000);
#endif
    return 0;
#endif
}

/* Create one directory. 0 on success or if it is already there. */
static int omg_os_mkdir(const char *path) {
#ifdef _WIN32
    wchar_t *w = omg_os_wide(path);
    if (!w) { errno = EINVAL; return -1; }
    int rc = _wmkdir(w);
    free(w);
#else
    int rc = mkdir(path, 0755);
#endif
    if (rc == 0 || errno == EEXIST) return 0;
    /* "C:" or "/" as the first prefix of a longer path: fine if it is
     * a directory already, whatever errno says. */
    int saved = errno;
    if (omg_os_is_dir(path)) return 0;
    errno = saved;
    return -1;
}

/* 64-bit seek / tell (`long` is 32 bits on Windows). */
static int omg_os_seek(FILE *fp, int64_t off) {
#ifdef _WIN32
    return _fseeki64(fp, off, SEEK_SET);
#else
    return fseek(fp, (long)off, SEEK_SET);
#endif
}

static int64_t omg_os_tell(FILE *fp) {
#ifdef _WIN32
    return (int64_t)_ftelli64(fp);
#else
    return (int64_t)ftell(fp);
#endif
}

/* Milliseconds since the UNIX epoch; 0 if the clock cannot be read. */
static int64_t omg_os_time_ms(void) {
#ifdef _WIN32
    FILETIME ft;
    GetSystemTimeAsFileTime(&ft);
    uint64_t t = ((uint64_t)ft.dwHighDateTime << 32) | (uint64_t)ft.dwLowDateTime;
    /* FILETIME counts 100 ns ticks from 1601-01-01. */
    return (int64_t)((t - 116444736000000000ULL) / 10000ULL);
#else
    struct timespec ts;
    if (clock_gettime(CLOCK_REALTIME, &ts) != 0) return 0;
    return (int64_t)ts.tv_sec * 1000 + (int64_t)(ts.tv_nsec / 1000000);
#endif
}

static void omg_os_sleep_ms(int64_t n) {
#ifdef _WIN32
    while (n > 0) {
        DWORD step = n > 0x7fffffff ? 0x7fffffff : (DWORD)n;
        Sleep(step);
        n -= step;
    }
#else
    struct timespec ts;
    ts.tv_sec = (time_t)(n / 1000);
    ts.tv_nsec = (long)((n % 1000) * 1000000);
    nanosleep(&ts, NULL);
#endif
}

/* getpid(): return the process ID. Used by the OMG-native driver to
 * generate unique tempfile paths (e.g. /tmp/omg-<pid>.omgb) without
 * needing a mktemp builtin. */
static Value omg_builtin_getpid(void) {
#ifdef _WIN32
    return omg_int((int64_t)GetCurrentProcessId());
#else
    return omg_int((int64_t)getpid());
#endif
}

/* executable_path(): absolute path of the currently running binary, so
 * the toolchain can locate sibling files (omg_rt.h) regardless of how
 * it was invoked. Returns `false` on platforms where we don't have a
 * way to look this up — callers fall back to dirname(args[0]).
 *
 * Linux reads /proc/self/exe; macOS asks dyld (_NSGetExecutablePath);
 * Windows asks the loader (GetModuleFileNameW).
 * BSDs vary and aren't implemented: the fallback path keeps the
 * toolchain working there when invoked via an absolute or relative
 * path. */
static Value omg_builtin_executable_path(void) {
#if defined(_WIN32)
    wchar_t wbuf[32768];
    DWORD n = GetModuleFileNameW(NULL, wbuf, (DWORD)(sizeof(wbuf) / sizeof(wbuf[0])));
    if (n == 0 || n >= sizeof(wbuf) / sizeof(wbuf[0])) return omg_bool(0);
    char *out = omg_os_utf8(wbuf);
    if (!out) return omg_bool(0);
    return omg_str(out);
#elif defined(__linux__)
    char buf[4096];
    ssize_t n = readlink("/proc/self/exe", buf, sizeof(buf) - 1);
    if (n < 0) return omg_bool(0);
    buf[n] = 0;
    char *out = (char *)malloc((size_t)n + 1);
    if (!out) { fprintf(stderr, "out of memory\n"); exit(1); }
    memcpy(out, buf, (size_t)n + 1);
    return omg_str(out);
#elif defined(__APPLE__)
    char buf[4096];
    uint32_t size = (uint32_t)sizeof(buf);
    if (_NSGetExecutablePath(buf, &size) != 0) return omg_bool(0);
    /* Resolve symlinks and `..` so dirname() of the result is the real
     * directory holding omg_rt.h. Fall back to the raw path. */
    char resolved[4096];
    const char *src = realpath(buf, resolved) ? resolved : buf;
    size_t n = strlen(src);
    char *out = (char *)malloc(n + 1);
    if (!out) { fprintf(stderr, "out of memory\n"); exit(1); }
    memcpy(out, src, n + 1);
    return omg_str(out);
#else
    return omg_bool(0);
#endif
}

/* stdin_readline(): read one line from stdin (no trailing newline).
 * Returns `false` on EOF (same convention as read_file). Used by the
 * OMG-native REPL. */
static Value omg_builtin_stdin_readline(void) {
    /* Grow buffer dynamically. fgetc loop is simple and portable. */
    size_t cap = 256, len = 0;
    char *buf = (char *)malloc(cap);
    if (!buf) { fprintf(stderr, "out of memory\n"); exit(1); }
    int c;
    int saw_anything = 0;
    while ((c = fgetc(stdin)) != EOF) {
        saw_anything = 1;
        if (c == '\n') break;
        if (len + 1 >= cap) {
            cap *= 2;
            char *nb = (char *)realloc(buf, cap);
            if (!nb) { free(buf); fprintf(stderr, "out of memory\n"); exit(1); }
            buf = nb;
        }
        buf[len++] = (char)c;
    }
    if (!saw_anything) {
        free(buf);
        return omg_bool(0);
    }
    /* Strip trailing \r if present (Windows line endings). */
    if (len > 0 && buf[len - 1] == '\r') len--;
    buf[len] = 0;
    return omg_str(buf);
}

/* stdin_read(): slurp all of stdin to EOF as a UTF-8 string. The
 * pipe-friendly counterpart to read_file() — `cat input | tool` works
 * once the tool calls stdin_read(). Returns the empty string when stdin
 * is already at EOF (no input piped in).
 *
 * Buffer ownership matches stdin_readline / omg_str: the heap
 * allocation is handed to the Value and not freed (OMG_STR is a bare
 * `const char *` with no refcount). */
static Value omg_builtin_stdin_read(void) {
    size_t cap = 4096, len = 0;
    char *buf = (char *)malloc(cap);
    if (!buf) { fprintf(stderr, "out of memory\n"); exit(1); }
    int c;
    while ((c = fgetc(stdin)) != EOF) {
        if (len + 1 >= cap) {
            cap *= 2;
            char *nb = (char *)realloc(buf, cap);
            if (!nb) { free(buf); fprintf(stderr, "out of memory\n"); exit(1); }
            buf = nb;
        }
        buf[len++] = (char)c;
    }
    buf[len] = 0;
    return omg_str(buf);
}

/* stdin_read_bytes(): slurp all of stdin to EOF as a list of byte
 * values (0-255). Pipe-friendly counterpart to file_open(path, "rb") +
 * file_read(). Empty list on no input. */
static Value omg_builtin_stdin_read_bytes(void) {
    Value v;
    v.tag = OMG_LIST;
    v.v.l = omg_list_alloc(0);
    int c;
    while ((c = fgetc(stdin)) != EOF) {
        omg_list_push(v.v.l, omg_int((int64_t)(unsigned char)c));
    }
    return v;
}

/* print(s): like emit, but no trailing newline. Used for REPL prompts. */
static Value omg_builtin_print(Value v) {
    if (v.tag != OMG_STR) omg_panic("TypeError", "print() expects a string");
    fputs(v.v.s, stdout);
    fflush(stdout);
    return omg_none();
}

/* === Real-time terminal I/O ============================================
 * time_ms / sleep_ms / stdin_set_raw / stdin_read_key form the platform
 * primitives needed by interactive terminal programs (see
 * examples/games/snake.omg). They mirror the Rust runtime's builtins of the
 * same names.
 */

/* time_ms(): milliseconds since the UNIX epoch. Suitable for elapsed
 * time / frame pacing; not a real monotonic clock — system time changes
 * are visible. */
static Value omg_builtin_time_ms(void) {
    return omg_int(omg_os_time_ms());
}

/* sleep_ms(n): pause this process for n ms. Negative / zero is a no-op. */
static Value omg_builtin_sleep_ms(Value v) {
    if (v.tag != OMG_INT) omg_panic("TypeError", "sleep_ms() expects an int");
    int64_t n = v.v.i;
    if (n <= 0) return omg_none();
    omg_os_sleep_ms(n);
    return omg_none();
}

#ifdef _WIN32
/* --- Windows console ------------------------------------------------------
 * The same two builtins on the Windows console. This is the C twin of
 * the Windows backend in rust/src/vm/sys.rs: raw mode is three
 * console-mode flags, keys arrive as input records, and the bytes handed
 * to the program are the UTF-8 / VT sequences a Unix terminal would
 * have sent, so the games and the editor need no Windows code. */
#ifndef ENABLE_VIRTUAL_TERMINAL_INPUT
#define ENABLE_VIRTUAL_TERMINAL_INPUT 0x0200
#endif
#ifndef ENABLE_VIRTUAL_TERMINAL_PROCESSING
#define ENABLE_VIRTUAL_TERMINAL_PROCESSING 0x0004
#endif

#define OMG_CON_QUEUE 1024
static struct {
    int have_saved;        /* saved_mode is valid: raw mode is active   */
    DWORD saved_mode;      /* input mode before the first set_raw(true) */
    int vt_input;          /* the console itself sends VT sequences     */
    int atexit_set;
    WCHAR high_surrogate;  /* first half of a UTF-16 pair, or 0         */
    unsigned char pending[OMG_CON_QUEUE];  /* bytes decoded, not yet read */
    int head, len;
} omg_con;

static void omg_con_push(unsigned char b) {
    if (omg_con.len < OMG_CON_QUEUE) {
        omg_con.pending[(omg_con.head + omg_con.len) % OMG_CON_QUEUE] = b;
        omg_con.len++;
    }
}

static int omg_con_pop(void) {
    if (omg_con.len == 0) return -1;
    int b = omg_con.pending[omg_con.head];
    omg_con.head = (omg_con.head + 1) % OMG_CON_QUEUE;
    omg_con.len--;
    return b;
}

/* Queue one UTF-16 unit as UTF-8, pairing surrogates. */
static void omg_con_push_utf16(WCHAR unit) {
    uint32_t c;
    if (unit >= 0xD800 && unit <= 0xDBFF) { omg_con.high_surrogate = unit; return; }
    if (unit >= 0xDC00 && unit <= 0xDFFF) {
        if (omg_con.high_surrogate == 0) return;
        c = 0x10000 + (((uint32_t)omg_con.high_surrogate - 0xD800) << 10) + ((uint32_t)unit - 0xDC00);
    } else {
        c = unit;
    }
    omg_con.high_surrogate = 0;
    if (c < 0x80) {
        omg_con_push((unsigned char)c);
    } else if (c < 0x800) {
        omg_con_push((unsigned char)(0xC0 | (c >> 6)));
        omg_con_push((unsigned char)(0x80 | (c & 0x3F)));
    } else if (c < 0x10000) {
        omg_con_push((unsigned char)(0xE0 | (c >> 12)));
        omg_con_push((unsigned char)(0x80 | ((c >> 6) & 0x3F)));
        omg_con_push((unsigned char)(0x80 | (c & 0x3F)));
    } else {
        omg_con_push((unsigned char)(0xF0 | (c >> 18)));
        omg_con_push((unsigned char)(0x80 | ((c >> 12) & 0x3F)));
        omg_con_push((unsigned char)(0x80 | ((c >> 6) & 0x3F)));
        omg_con_push((unsigned char)(0x80 | (c & 0x3F)));
    }
}

/* The escape sequence a VT terminal sends for a navigation key. Only
 * used when the console refused ENABLE_VIRTUAL_TERMINAL_INPUT. */
static const char *omg_con_vk_to_vt(WORD vk) {
    switch (vk) {
        case 0x26: return "\x1b[A";   /* VK_UP */
        case 0x28: return "\x1b[B";   /* VK_DOWN */
        case 0x27: return "\x1b[C";   /* VK_RIGHT */
        case 0x25: return "\x1b[D";   /* VK_LEFT */
        case 0x24: return "\x1b[H";   /* VK_HOME */
        case 0x23: return "\x1b[F";   /* VK_END */
        case 0x2D: return "\x1b[2~";  /* VK_INSERT */
        case 0x2E: return "\x1b[3~";  /* VK_DELETE */
        case 0x21: return "\x1b[5~";  /* VK_PRIOR (Page Up) */
        case 0x22: return "\x1b[6~";  /* VK_NEXT (Page Down) */
        default:   return NULL;
    }
}

/* Put the console back the way it was. Registered with atexit() the
 * first time raw mode is switched on, so a program that exits (or dies
 * with an uncaught error) in raw mode does not leave the shell broken. */
static void omg_con_restore(void) {
    if (!omg_con.have_saved) return;
    HANDLE h = GetStdHandle(STD_INPUT_HANDLE);
    if (h != NULL && h != INVALID_HANDLE_VALUE) SetConsoleMode(h, omg_con.saved_mode);
    omg_con.have_saved = 0;
}

static Value omg_builtin_stdin_set_raw(Value v) {
    if (v.tag != OMG_BOOL) omg_panic("TypeError", "stdin_set_raw() expects a bool");
    HANDLE h = GetStdHandle(STD_INPUT_HANDLE);
    DWORD mode = 0;
    if (h == NULL || h == INVALID_HANDLE_VALUE || !GetConsoleMode(h, &mode)) {
        omg_panic("ValueError", "stdin_set_raw: stdin is not a console");
    }
    const DWORD cooked = ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT;
    if (v.v.b) {
        if (!omg_con.have_saved) {
            omg_con.saved_mode = mode;
            omg_con.have_saved = 1;
            if (!omg_con.atexit_set) { atexit(omg_con_restore); omg_con.atexit_set = 1; }
        }
        /* LINE_INPUT ~ ICANON, ECHO_INPUT ~ ECHO, PROCESSED_INPUT ~ ISIG
         * (Ctrl-C arrives as byte 0x03 instead of ending the program). */
        DWORD raw = mode & ~cooked;
        if (SetConsoleMode(h, raw | ENABLE_VIRTUAL_TERMINAL_INPUT)) omg_con.vt_input = 1;
        else if (SetConsoleMode(h, raw)) omg_con.vt_input = 0;
        else omg_panic("ValueError", "stdin_set_raw: SetConsoleMode failed");
    } else {
        DWORD restore = omg_con.have_saved
            ? omg_con.saved_mode
            : ((mode | cooked) & ~(DWORD)ENABLE_VIRTUAL_TERMINAL_INPUT);
        omg_con.have_saved = 0;
        omg_con.vt_input = 0;
        omg_con.head = 0;
        omg_con.len = 0;
        omg_con.high_surrogate = 0;
        if (!SetConsoleMode(h, restore)) omg_panic("ValueError", "stdin_set_raw: SetConsoleMode failed");
    }
    return omg_none();
}

/* One byte of keyboard input, or -1 when no key is waiting. */
static int omg_con_read_byte(void) {
    int b = omg_con_pop();
    if (b >= 0) return b;
    HANDLE h = GetStdHandle(STD_INPUT_HANDLE);
    DWORD mode = 0;
    if (h == NULL || h == INVALID_HANDLE_VALUE) return -1;
    if (!GetConsoleMode(h, &mode)) {
        /* Redirected stdin (a pipe or a file) has no "no key yet"
         * state: behave like a blocking one-byte read, which is what
         * read(0) does for a pipe on Unix. */
        unsigned char c;
        return _read(0, &c, 1) == 1 ? (int)c : -1;
    }
    /* Drain queued console events until one yields a byte. Mouse, focus
     * and resize events, key releases and bare modifier presses are
     * consumed and skipped; left queued, the count would never reach 0. */
    for (;;) {
        DWORD count = 0, got = 0;
        INPUT_RECORD rec;
        if (!GetNumberOfConsoleInputEvents(h, &count) || count == 0) return -1;
        if (!ReadConsoleInputW(h, &rec, 1, &got) || got == 0) return -1;
        if (rec.EventType != KEY_EVENT || !rec.Event.KeyEvent.bKeyDown) continue;
        WCHAR unit = rec.Event.KeyEvent.uChar.UnicodeChar;
        int repeat = rec.Event.KeyEvent.wRepeatCount > 0 ? rec.Event.KeyEvent.wRepeatCount : 1;
        if (unit != 0) {
            for (int i = 0; i < repeat; i++) omg_con_push_utf16(unit);
        } else if (!omg_con.vt_input) {
            const char *seq = omg_con_vk_to_vt(rec.Event.KeyEvent.wVirtualKeyCode);
            if (seq) {
                for (int i = 0; i < repeat; i++) {
                    for (const char *q = seq; *q; q++) omg_con_push((unsigned char)*q);
                }
            }
        }
        b = omg_con_pop();
        if (b >= 0) return b;
    }
}

/* stdin_read_key(): non-blocking single-byte read; see the POSIX
 * version below for the contract. */
static Value omg_builtin_stdin_read_key(void) {
    int b = omg_con_read_byte();
    if (b < 0) return omg_bool(0);
    char *out = (char *)malloc(2);
    if (!out) { fprintf(stderr, "out of memory\n"); exit(1); }
    out[0] = (char)b;
    out[1] = 0;
    return omg_str(out);
}
#else
/* stdin_set_raw(on): toggle cbreak / no-echo on the controlling TTY.
 * In raw mode reads return immediately (VMIN=VTIME=0) so they pair with
 * stdin_read_key(). Caller is responsible for restoring cooked mode
 * before exit, otherwise the user's shell prompt looks weird. */
static Value omg_builtin_stdin_set_raw(Value v) {
    if (v.tag != OMG_BOOL) omg_panic("TypeError", "stdin_set_raw() expects a bool");
    struct termios t;
    if (tcgetattr(0, &t) != 0) omg_panic("ValueError", "stdin_set_raw: tcgetattr failed");
    if (v.v.b) {
        /* ICANON / ECHO: standard raw-mode (line buffering / echo off).
         * ISIG: keep Ctrl-C / Ctrl-Z / Ctrl-\ from generating signals
         * — TUI apps want those as plain bytes (e.g. ^Z for undo).
         * IXON: keep Ctrl-S / Ctrl-Q from triggering XON/XOFF flow
         * control, which would otherwise freeze terminal output. */
        t.c_lflag &= ~(ICANON | ECHO | ISIG);
        t.c_iflag &= ~IXON;
        t.c_cc[VMIN] = 0;
        t.c_cc[VTIME] = 0;
    } else {
        t.c_lflag |= ICANON | ECHO | ISIG;
        t.c_iflag |= IXON;
    }
    if (tcsetattr(0, TCSANOW, &t) != 0) omg_panic("ValueError", "stdin_set_raw: tcsetattr failed");
    return omg_none();
}

/* stdin_read_key(): non-blocking single-byte read. Returns a one-char
 * OMG_STR if a key is available, else OMG_BOOL false. Requires
 * stdin_set_raw(true) — in cooked mode the kernel buffers until newline
 * and read() still blocks. */
static Value omg_builtin_stdin_read_key(void) {
    unsigned char buf;
    ssize_t n = read(0, &buf, 1);
    if (n != 1) return omg_bool(0);
    char *out = (char *)malloc(2);
    if (!out) { fprintf(stderr, "out of memory\n"); exit(1); }
    out[0] = (char)buf;
    out[1] = 0;
    return omg_str(out);
}

#endif /* _WIN32 */

/* === Pseudo-terminal primitives ========================================
 *
 * Five tiny shims around forkpty / read / write / close / TIOCSWINSZ
 * so OMG programs can embed an interactive child (a shell, less,
 * vim, etc.). The master fd is always set non-blocking — pty_read
 * polls cheaply and returns "" when no data is available, so the
 * caller can multiplex with stdin in a single loop.
 *
 * The whole pty package is mirrored in rust/src/vm/builtins.rs;
 * keep them in sync.
 */

#ifdef _WIN32
/* Windows has no forkpty. Same answers as the Rust runtime gives there:
 * spawn and write raise, read reports end-of-file, close and resize do
 * nothing. A program can probe with try / except. */
static Value omg_builtin_pty_spawn(Value v) {
    if (v.tag != OMG_LIST) omg_panic("TypeError", "pty_spawn() expects a list");
    omg_panic("ValueError", "pty_spawn(): pseudo-terminals are not supported on Windows");
    return omg_none();
}

static Value omg_builtin_pty_read(Value fd_v) {
    if (fd_v.tag != OMG_INT) omg_panic("TypeError", "pty_read() expects an int fd");
    return omg_bool(0);
}

static Value omg_builtin_pty_write(Value fd_v, Value s_v) {
    if (fd_v.tag != OMG_INT) omg_panic("TypeError", "pty_write() fd must be int");
    if (s_v.tag != OMG_STR)  omg_panic("TypeError", "pty_write() data must be string");
    omg_panic("ValueError", "pty_write(): pseudo-terminals are not supported on Windows");
    return omg_none();
}

static Value omg_builtin_pty_close(Value fd_v) {
    if (fd_v.tag != OMG_INT) omg_panic("TypeError", "pty_close() expects an int fd");
    return omg_none();
}

static Value omg_builtin_pty_resize(Value fd_v, Value rows_v, Value cols_v) {
    if (fd_v.tag != OMG_INT)   omg_panic("TypeError", "pty_resize() fd must be int");
    if (rows_v.tag != OMG_INT) omg_panic("TypeError", "pty_resize() rows must be int");
    if (cols_v.tag != OMG_INT) omg_panic("TypeError", "pty_resize() cols must be int");
    return omg_none();
}
#else
static Value omg_builtin_pty_spawn(Value v) {
    if (v.tag != OMG_LIST) omg_panic("TypeError", "pty_spawn() expects a list");
    OmgList *l = v.v.l;
    if (l->len == 0) omg_panic("ValueError", "pty_spawn() needs at least the command");
    char **argv = (char **)malloc((l->len + 1) * sizeof(char *));
    if (!argv) { fprintf(stderr, "out of memory\n"); exit(1); }
    for (int i = 0; i < l->len; i++) {
        if (l->items[i].tag != OMG_STR) {
            free(argv);
            omg_panic("TypeError", "pty_spawn() argv must be all strings");
        }
        argv[i] = (char *)l->items[i].v.s;
    }
    argv[l->len] = NULL;

    int master;
    pid_t pid = forkpty(&master, NULL, NULL, NULL);
    if (pid < 0) {
        free(argv);
        omg_panic("ValueError", "pty_spawn(): forkpty failed");
    }
    if (pid == 0) {
        /* Child. Set TERM, exec, and _exit on failure so we don't
         * unwind back into the OMG runtime. */
        setenv("TERM", "xterm-256color", 1);
        execvp(argv[0], argv);
        _exit(127);
    }
    /* Parent: master fd non-blocking so pty_read() can poll. */
    int flags = fcntl(master, F_GETFL, 0);
    fcntl(master, F_SETFL, flags | O_NONBLOCK);
    free(argv);
    return omg_int((int64_t)master);
}

/* Non-blocking read from the master fd. Three return shapes:
 *   - OMG_STR with bytes: data delivered
 *   - OMG_STR empty:      no data right now, try again later
 *   - OMG_BOOL false:     EOF / error (child closed) */
static Value omg_builtin_pty_read(Value fd_v) {
    if (fd_v.tag != OMG_INT) omg_panic("TypeError", "pty_read() expects an int fd");
    char buf[4096];
    ssize_t n = read((int)fd_v.v.i, buf, sizeof(buf));
    if (n > 0) {
        char *s = (char *)malloc(n + 1);
        if (!s) { fprintf(stderr, "out of memory\n"); exit(1); }
        memcpy(s, buf, n);
        s[n] = 0;
        return omg_str(s);
    }
    if (n == 0) {
        return omg_bool(0);
    }
    if (errno == EAGAIN || errno == EWOULDBLOCK) {
        char *empty = (char *)malloc(1);
        if (!empty) { fprintf(stderr, "out of memory\n"); exit(1); }
        empty[0] = 0;
        return omg_str(empty);
    }
    return omg_bool(0);
}

/* Write bytes to the master fd. Returns the number of bytes written
 * (may be short if the pty buffer is full). */
static Value omg_builtin_pty_write(Value fd_v, Value s_v) {
    if (fd_v.tag != OMG_INT) omg_panic("TypeError", "pty_write() fd must be int");
    if (s_v.tag != OMG_STR)  omg_panic("TypeError", "pty_write() data must be string");
    ssize_t n = write((int)fd_v.v.i, s_v.v.s, strlen(s_v.v.s));
    return omg_int((int64_t)n);
}

static Value omg_builtin_pty_close(Value fd_v) {
    if (fd_v.tag != OMG_INT) omg_panic("TypeError", "pty_close() expects an int fd");
    close((int)fd_v.v.i);
    return omg_none();
}

/* TIOCSWINSZ + SIGWINCH: the child re-reads dimensions and reflows. */
static Value omg_builtin_pty_resize(Value fd_v, Value rows_v, Value cols_v) {
    if (fd_v.tag != OMG_INT)   omg_panic("TypeError", "pty_resize() fd must be int");
    if (rows_v.tag != OMG_INT) omg_panic("TypeError", "pty_resize() rows must be int");
    if (cols_v.tag != OMG_INT) omg_panic("TypeError", "pty_resize() cols must be int");
    struct winsize ws;
    ws.ws_row = (unsigned short)rows_v.v.i;
    ws.ws_col = (unsigned short)cols_v.v.i;
    ws.ws_xpixel = 0;
    ws.ws_ypixel = 0;
    ioctl((int)fd_v.v.i, TIOCSWINSZ, &ws);
    return omg_none();
}
#endif /* _WIN32 */

/* subprocess(argv): run a program and wait for it. argv is a list of
 * strings; argv[0] is the program (looked up on PATH), the rest are its
 * arguments. stdin/stdout/stderr are inherited. Returns the child's
 * exit code.
 *
 * Mirrors std::process::Command::status() in the Rust runtime so the
 * OMG-native `omg` driver can start `cc` the same way. POSIX: fork +
 * execvp + waitpid. Windows: CreateProcess. */
#ifdef _WIN32
/* Growable byte buffer for building the command line. */
typedef struct { char *p; size_t len, cap; } OmgCmdBuf;

static void omg_cmd_put(OmgCmdBuf *b, char c) {
    if (b->len + 1 >= b->cap) {
        b->cap = b->cap ? b->cap * 2 : 256;
        char *np = (char *)realloc(b->p, b->cap);
        if (!np) { fprintf(stderr, "out of memory\n"); exit(1); }
        b->p = np;
    }
    b->p[b->len++] = c;
    b->p[b->len] = 0;
}

/* Append one argument, quoted so that the child's C runtime (and
 * CommandLineToArgvW) parse it back to exactly `arg`: backslashes are
 * literal unless they precede a double quote. */
static void omg_cmd_arg(OmgCmdBuf *b, const char *arg) {
    if (b->len > 0) omg_cmd_put(b, ' ');
    if (arg[0] != 0 && strpbrk(arg, " \t\n\v\"") == NULL) {
        for (const char *q = arg; *q; q++) omg_cmd_put(b, *q);
        return;
    }
    omg_cmd_put(b, '"');
    for (const char *q = arg;; q++) {
        size_t slashes = 0;
        while (*q == '\\') { q++; slashes++; }
        if (*q == 0) {
            for (size_t i = 0; i < slashes * 2; i++) omg_cmd_put(b, '\\');
            break;
        }
        if (*q == '"') {
            for (size_t i = 0; i < slashes * 2 + 1; i++) omg_cmd_put(b, '\\');
        } else {
            for (size_t i = 0; i < slashes; i++) omg_cmd_put(b, '\\');
        }
        omg_cmd_put(b, *q);
    }
    omg_cmd_put(b, '"');
}

static Value omg_builtin_subprocess(Value v) {
    if (v.tag != OMG_LIST) omg_panic("TypeError", "subprocess() expects a list of strings");
    OmgList *l = v.v.l;
    if (l->len == 0) omg_panic("ValueError", "subprocess() needs at least the command");
    for (int i = 0; i < l->len; i++) {
        if (l->items[i].tag != OMG_STR) omg_panic("TypeError", "subprocess() expects a list of strings");
    }
    OmgCmdBuf cmd = {NULL, 0, 0};
    for (int i = 0; i < l->len; i++) omg_cmd_arg(&cmd, l->items[i].v.s);
    wchar_t *wcmd = omg_os_wide(cmd.p);
    free(cmd.p);
    if (!wcmd) omg_panicf("ValueError", "subprocess: cannot exec '%s': invalid UTF-8", l->items[0].v.s);

    fflush(stdout);
    fflush(stderr);

    STARTUPINFOW si;
    PROCESS_INFORMATION pi;
    memset(&si, 0, sizeof(si));
    si.cb = sizeof(si);
    memset(&pi, 0, sizeof(pi));
    /* A NULL application name makes CreateProcess take the program from
     * the command line, search PATH for it, and add ".exe" if needed. */
    if (!CreateProcessW(NULL, wcmd, NULL, NULL, TRUE, 0, NULL, NULL, &si, &pi)) {
        DWORD err = GetLastError();
        free(wcmd);
        /* The system's own wording, as the Rust runtime prints it. */
        char text[512];
        text[0] = 0;
        wchar_t wtext[256];
        DWORD n = FormatMessageW(FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
                                 NULL, err, 0, wtext, 256, NULL);
        while (n > 0 && (wtext[n - 1] == L'\r' || wtext[n - 1] == L'\n' || wtext[n - 1] == L' ')) n--;
        wtext[n] = 0;
        if (n > 0) {
            int m = WideCharToMultiByte(CP_UTF8, 0, wtext, -1, text, (int)sizeof(text) - 2, NULL, NULL);
            if (m <= 0) text[0] = 0;
            else strcat(text, " ");
        }
        omg_panicf("ValueError", "subprocess: cannot exec '%s': %s(os error %lu)",
                   l->items[0].v.s, text, (unsigned long)err);
    }
    free(wcmd);
    WaitForSingleObject(pi.hProcess, INFINITE);
    DWORD code = 0;
    if (!GetExitCodeProcess(pi.hProcess, &code)) code = (DWORD)-1;
    CloseHandle(pi.hThread);
    CloseHandle(pi.hProcess);
    return omg_int((int64_t)(int32_t)code);
}
#else
static Value omg_builtin_subprocess(Value v) {
    if (v.tag != OMG_LIST) omg_panic("TypeError", "subprocess() expects a list of strings");
    OmgList *l = v.v.l;
    if (l->len == 0) omg_panic("ValueError", "subprocess() needs at least the command");

    /* Build NULL-terminated argv array. We borrow the string pointers
     * from the OMG values; execvp doesn't require them to be writable
     * even though its prototype says char *const[]. */
    char **argv = (char **)malloc(((size_t)l->len + 1) * sizeof(char *));
    if (!argv) { fprintf(stderr, "out of memory\n"); exit(1); }
    for (int i = 0; i < l->len; i++) {
        if (l->items[i].tag != OMG_STR) {
            free(argv);
            omg_panic("TypeError", "subprocess() expects a list of strings");
        }
        argv[i] = (char *)l->items[i].v.s;
    }
    argv[l->len] = NULL;

    /* Flush stdio so output ordering matches Rust's behaviour around
     * the fork. */
    fflush(stdout);
    fflush(stderr);

    /* A program that cannot be started is an error the caller can
     * catch, as on every other backend, not an exit code from a child
     * that never ran. The child reports a failed exec through a pipe
     * that a successful exec closes by itself (close-on-exec). */
    int report[2];
    if (pipe(report) != 0) {
        free(argv);
        omg_panicf("ValueError", "subprocess: pipe failed: %s", strerror(errno));
    }
    fcntl(report[0], F_SETFD, FD_CLOEXEC);
    fcntl(report[1], F_SETFD, FD_CLOEXEC);

    pid_t pid = fork();
    if (pid < 0) {
        int e = errno;
        close(report[0]);
        close(report[1]);
        free(argv);
        omg_panicf("ValueError", "subprocess: fork failed: %s", strerror(e));
    }
    if (pid == 0) {
        /* Child: exec replaces this process. */
        close(report[0]);
        execvp(argv[0], argv);
        int e = errno;
        ssize_t w = write(report[1], &e, sizeof(e));
        (void)w;
        _exit(127);
    }

    /* Parent: learn whether the exec happened, then wait for the child. */
    close(report[1]);
    int exec_errno = 0;
    ssize_t got;
    do {
        got = read(report[0], &exec_errno, sizeof(exec_errno));
    } while (got < 0 && errno == EINTR);
    close(report[0]);
    if (got != (ssize_t)sizeof(exec_errno)) exec_errno = 0;

    int status = 0;
    pid_t waited;
    do {
        waited = waitpid(pid, &status, 0);
    } while (waited < 0 && errno == EINTR);
    if (waited < 0) {
        free(argv);
        omg_panicf("ValueError", "subprocess: waitpid failed: %s", strerror(errno));
    }
    if (exec_errno != 0) {
        /* argv[0] belongs to the caller's list, which outlives this. */
        const char *prog = argv[0];
        free(argv);
        omg_panicf("ValueError", "subprocess: cannot exec '%s': %s (os error %d)",
                   prog, strerror(exec_errno), exec_errno);
    }
    free(argv);

    int code;
    if (WIFEXITED(status))        code = WEXITSTATUS(status);
    else if (WIFSIGNALED(status)) code = 128 + WTERMSIG(status);
    else                          code = -1;
    return omg_int((int64_t)code);
}
#endif /* _WIN32 */

/* === File I/O ============================================================ */

/* Path-resolution base. main() initialises this from getcwd(); file
 * builtins join relative paths against it the same way the OMG VM's
 * resolve_path() does. Mutating v_current_dir from inside a program
 * does NOT update this — that's a documented divergence from the VM,
 * which honours mid-program changes; we don't because the runtime
 * header has no dynamic link to user globals. */
static const char *omg_cwd_str = ".";

/* Slash-or-drive-letter detection — keep cheap and ASCII. Either
 * separator counts, so the same test serves Windows paths. */
static const char *omg_resolve_path(const char *p) {
    if (p == NULL) return p;
    int absolute = (p[0] == '/') || (p[0] == '\\') || (p[0] && p[1] == ':');
    if (absolute) return p;
    size_t lb = strlen(omg_cwd_str), lp = strlen(p);
    char *buf = (char *)malloc(lb + 2 + lp);
    if (!buf) { fprintf(stderr, "out of memory\n"); exit(1); }
    memcpy(buf, omg_cwd_str, lb);
    if (lb > 0 && omg_cwd_str[lb - 1] != '/' && omg_cwd_str[lb - 1] != '\\') {
        buf[lb] = '/';
        memcpy(buf + lb + 1, p, lp + 1);
    } else {
        memcpy(buf + lb, p, lp + 1);
    }
    return buf;
}

typedef struct {
    FILE *fp;
    int binary;
    int valid;
} OmgFileEntry;

#define OMG_MAX_FILES 64
static OmgFileEntry omg_file_table[OMG_MAX_FILES];

static int omg_file_alloc(FILE *fp, int binary) {
    for (int i = 0; i < OMG_MAX_FILES; i++) {
        if (!omg_file_table[i].valid) {
            omg_file_table[i].fp = fp;
            omg_file_table[i].binary = binary;
            omg_file_table[i].valid = 1;
            return i + 1;  /* 1-based so a 0 handle is always invalid */
        }
    }
    omg_panic("ValueError", "too many open files");
    return -1;
}

static OmgFileEntry *omg_file_get(int handle) {
    int idx = handle - 1;
    if (idx < 0 || idx >= OMG_MAX_FILES || !omg_file_table[idx].valid) return NULL;
    return &omg_file_table[idx];
}

static Value omg_builtin_file_open(Value path, Value mode) {
    if (path.tag != OMG_STR || mode.tag != OMG_STR) {
        omg_panic("TypeError", "file_open() expects path and mode");
    }
    const char *m = mode.v.s;
    int binary = strchr(m, 'b') != NULL;
    const char *fm = NULL;
    if (strcmp(m, "r") == 0 || strcmp(m, "rb") == 0) fm = binary ? "rb" : "r";
    else if (strcmp(m, "w") == 0 || strcmp(m, "wb") == 0) fm = binary ? "wb" : "w";
    else if (strcmp(m, "a") == 0 || strcmp(m, "ab") == 0) fm = binary ? "ab" : "a";
    /* Random-access binary modes (used by tools/db for paged I/O):
     *   rb+ — open existing for read+write; preserves contents.
     *   wb+ — create/truncate for read+write. */
    else if (strcmp(m, "rb+") == 0) fm = "rb+";
    else if (strcmp(m, "wb+") == 0) fm = "wb+";
    else omg_panic("ValueError", "invalid file mode");
    const char *full = omg_resolve_path(path.v.s);
    FILE *fp = omg_os_fopen(full, fm);
    if (!fp) {
        omg_panicf("ValueError", "cannot open '%s': %s (os error %d)",
                   full, strerror(errno), errno);
    }
    return omg_int((int64_t)omg_file_alloc(fp, binary));
}

/* Slurp the rest of `fp` into a freshly-malloc'd buffer; returns the
 * pointer in *out_buf and the byte count in *out_len. */
static void omg_slurp_file(FILE *fp, char **out_buf, size_t *out_len) {
    size_t cap = 1024, len = 0;
    char *buf = (char *)malloc(cap);
    if (!buf) { fprintf(stderr, "out of memory\n"); exit(1); }
    for (;;) {
        if (len == cap) {
            cap *= 2;
            char *nb = (char *)realloc(buf, cap);
            if (!nb) { fprintf(stderr, "out of memory\n"); exit(1); }
            buf = nb;
        }
        size_t got = fread(buf + len, 1, cap - len, fp);
        len += got;
        if (got == 0) break;
    }
    /* Trim and terminate; some callers want a NUL-terminated string. */
    char *nb = (char *)realloc(buf, len + 1);
    if (!nb) { fprintf(stderr, "out of memory\n"); exit(1); }
    nb[len] = 0;
    *out_buf = nb;
    *out_len = len;
}

static Value omg_builtin_file_read(Value h) {
    if (h.tag != OMG_INT) omg_panic("TypeError", "file_read() expects a handle");
    OmgFileEntry *e = omg_file_get((int)h.v.i);
    if (!e) omg_panic("ValueError", "invalid file handle");
    char *buf;
    size_t len;
    omg_slurp_file(e->fp, &buf, &len);
    if (e->binary) {
        Value out;
        out.tag = OMG_LIST;
        out.v.l = omg_list_alloc((int)len);
        for (size_t i = 0; i < len; i++) {
            omg_list_push(out.v.l, omg_int((unsigned char)buf[i]));
        }
        free(buf);
        return out;
    }
    return omg_str(buf);
}

/* file_read(h, max): at most max bytes from where the handle stands;
 * fewer only when the file ends first. */
static Value omg_builtin_file_read2(Value h, Value maxbytes) {
    if (h.tag != OMG_INT || maxbytes.tag != OMG_INT) {
        omg_panic("TypeError", "file_read() expects (handle: int, max_bytes: int)");
    }
    if (maxbytes.v.i < 0) omg_panic("ValueError", "file_read: max_bytes must be non-negative");
    OmgFileEntry *e = omg_file_get((int)h.v.i);
    if (!e) omg_panic("ValueError", "invalid file handle");
    size_t want = (size_t)maxbytes.v.i, cap = 0, len = 0;
    char *buf = NULL;
    while (len < want) {
        size_t step = want - len;
        if (step > 65536) step = 65536;
        if (len + step + 1 > cap) {
            cap = (len + step + 1) * 2;
            char *nb = (char *)realloc(buf, cap);
            if (!nb) { fprintf(stderr, "out of memory\n"); exit(1); }
            buf = nb;
        }
        size_t got = fread(buf + len, 1, step, e->fp);
        len += got;
        if (got < step) break;
    }
    if (e->binary) {
        Value out;
        out.tag = OMG_LIST;
        out.v.l = omg_list_alloc(len > 0 ? (int)len : 1);
        for (size_t i = 0; i < len; i++) {
            omg_list_push(out.v.l, omg_int((unsigned char)buf[i]));
        }
        free(buf);
        return out;
    }
    if (!buf) {
        buf = (char *)malloc(1);
        if (!buf) { fprintf(stderr, "out of memory\n"); exit(1); }
    }
    buf[len] = 0;
    return omg_str(buf);
}

static Value omg_builtin_file_size(Value path) {
    if (path.tag != OMG_STR) omg_panic("TypeError", "file_size() expects a path");
    const char *full = omg_resolve_path(path.v.s);
    int64_t size = 0, mtime = 0;
    if (omg_os_stat(full, &size, &mtime) != 0) {
        omg_panicf("ValueError", "cannot stat '%s': %s (os error %d)", full, strerror(errno), errno);
    }
    return omg_int(size);
}

static Value omg_builtin_file_mtime(Value path) {
    if (path.tag != OMG_STR) omg_panic("TypeError", "file_mtime() expects a path");
    const char *full = omg_resolve_path(path.v.s);
    int64_t size = 0, mtime = 0;
    if (omg_os_stat(full, &size, &mtime) != 0) {
        omg_panicf("ValueError", "cannot stat '%s': %s (os error %d)", full, strerror(errno), errno);
    }
    return omg_int(mtime);
}

static Value omg_builtin_file_write(Value h, Value data) {
    if (h.tag != OMG_INT) omg_panic("TypeError", "file_write() expects a handle");
    OmgFileEntry *e = omg_file_get((int)h.v.i);
    if (!e) omg_panic("ValueError", "invalid file handle");
    if (e->binary) {
        if (data.tag != OMG_LIST) omg_panic("TypeError", "file_write() binary handle expects list");
        OmgList *l = data.v.l;
        unsigned char *buf = NULL;
        if (l->len > 0) {
            buf = (unsigned char *)malloc(l->len);
            if (!buf) { fprintf(stderr, "out of memory\n"); exit(1); }
            for (int i = 0; i < l->len; i++) {
                if (l->items[i].tag != OMG_INT || l->items[i].v.i < 0 || l->items[i].v.i > 255) {
                    free(buf);
                    omg_panic("TypeError", "file_write() expects bytes 0-255");
                }
                buf[i] = (unsigned char)l->items[i].v.i;
            }
        }
        size_t wrote = l->len > 0 ? fwrite(buf, 1, l->len, e->fp) : 0;
        free(buf);
        return omg_int((int64_t)wrote);
    }
    if (data.tag != OMG_STR) omg_panic("TypeError", "file_write() text handle expects string");
    size_t n = strlen(data.v.s);
    size_t wrote = fwrite(data.v.s, 1, n, e->fp);
    return omg_int((int64_t)wrote);
}

static Value omg_builtin_file_close(Value h) {
    if (h.tag != OMG_INT) omg_panic("TypeError", "file_close() expects handle");
    OmgFileEntry *e = omg_file_get((int)h.v.i);
    if (!e) omg_panic("ValueError", "invalid file handle");
    fclose(e->fp);
    e->fp = NULL;
    e->valid = 0;
    return omg_none();
}

/* file_seek(handle, offset): seek the handle to `offset` bytes from
 * the start of the file. Negative offsets rejected. Returns the new
 * absolute position (always equals `offset` on success). */
static Value omg_builtin_file_seek(Value h, Value off) {
    if (h.tag != OMG_INT) omg_panic("TypeError", "file_seek() expects a handle");
    if (off.tag != OMG_INT) omg_panic("TypeError", "file_seek() expects an integer offset");
    if (off.v.i < 0) omg_panic("ValueError", "file_seek() expects a non-negative offset");
    OmgFileEntry *e = omg_file_get((int)h.v.i);
    if (!e) omg_panic("ValueError", "invalid file handle");
    if (omg_os_seek(e->fp, off.v.i) != 0) {
        omg_panicf("ValueError", "file_seek: %s", strerror(errno));
    }
    return omg_int(off.v.i);
}

/* file_tell(handle): current absolute byte position of the handle. */
static Value omg_builtin_file_tell(Value h) {
    if (h.tag != OMG_INT) omg_panic("TypeError", "file_tell() expects a handle");
    OmgFileEntry *e = omg_file_get((int)h.v.i);
    if (!e) omg_panic("ValueError", "invalid file handle");
    int64_t pos = omg_os_tell(e->fp);
    if (pos < 0) {
        omg_panicf("ValueError", "file_tell: %s", strerror(errno));
    }
    return omg_int(pos);
}

static Value omg_builtin_read_file(Value path) {
    if (path.tag != OMG_STR) omg_panic("TypeError", "read_file() expects a file path");
    const char *full = omg_resolve_path(path.v.s);
    FILE *fp = omg_os_fopen(full, "r");
    if (!fp) {
        omg_panicf("ModuleImportError", "failed to read '%s': %s (os error %d)",
                   full, strerror(errno), errno);
    }
    char *buf;
    size_t len;
    omg_slurp_file(fp, &buf, &len);
    fclose(fp);
    return omg_str(buf);
}

static Value omg_builtin_file_exists(Value path) {
    if (path.tag != OMG_STR) omg_panic("TypeError", "file_exists() expects a path");
    const char *full = omg_resolve_path(path.v.s);
    return omg_bool(omg_os_exists(full));
}

static Value omg_builtin_is_dir(Value path) {
    if (path.tag != OMG_STR) omg_panic("TypeError", "is_dir() expects a path");
    const char *full = omg_resolve_path(path.v.s);
    return omg_bool(omg_os_is_dir(full));
}

static Value omg_builtin_read_dir(Value path) {
    if (path.tag != OMG_STR) omg_panic("TypeError", "read_dir() expects a directory path");
    const char *full = omg_resolve_path(path.v.s);
    Value out;
#ifdef _WIN32
    /* FindFirstFile wants a pattern: "<dir>\*". */
    size_t flen = strlen(full);
    char *pattern = (char *)malloc(flen + 3);
    if (!pattern) { fprintf(stderr, "out of memory\n"); exit(1); }
    memcpy(pattern, full, flen);
    if (flen > 0 && full[flen - 1] != '/' && full[flen - 1] != '\\') pattern[flen++] = '\\';
    pattern[flen++] = '*';
    pattern[flen] = 0;
    wchar_t *wpattern = omg_os_wide(pattern);
    free(pattern);
    WIN32_FIND_DATAW fd;
    HANDLE find = wpattern ? FindFirstFileW(wpattern, &fd) : INVALID_HANDLE_VALUE;
    free(wpattern);
    if (find == INVALID_HANDLE_VALUE) {
        DWORD err = GetLastError();
        omg_panicf("ValueError", "cannot read directory '%s': (os error %lu)",
                   full, (unsigned long)err);
    }
    out.tag = OMG_LIST;
    out.v.l = omg_list_alloc(8);
    do {
        if (wcscmp(fd.cFileName, L".") == 0 || wcscmp(fd.cFileName, L"..") == 0) continue;
        char *copy = omg_os_utf8(fd.cFileName);
        if (!copy) { fprintf(stderr, "out of memory\n"); exit(1); }
        omg_list_push(out.v.l, omg_str(copy));
    } while (FindNextFileW(find, &fd));
    FindClose(find);
#else
    DIR *dir = opendir(full);
    if (!dir) {
        omg_panicf("ValueError", "cannot read directory '%s': %s (os error %d)",
                   full, strerror(errno), errno);
    }
    out.tag = OMG_LIST;
    out.v.l = omg_list_alloc(8);
    struct dirent *de;
    while ((de = readdir(dir)) != NULL) {
        if (strcmp(de->d_name, ".") == 0 || strcmp(de->d_name, "..") == 0) continue;
        size_t nlen = strlen(de->d_name);
        char *copy = (char *)malloc(nlen + 1);
        if (!copy) { fprintf(stderr, "out of memory\n"); exit(1); }
        memcpy(copy, de->d_name, nlen + 1);
        omg_list_push(out.v.l, omg_str(copy));
    }
    closedir(dir);
#endif
    /* Insertion sort lexicographically. Directory listings are small. */
    int n = out.v.l->len;
    for (int i = 1; i < n; i++) {
        Value cur = out.v.l->items[i];
        int j = i;
        while (j > 0 && strcmp(out.v.l->items[j - 1].v.s, cur.v.s) > 0) {
            out.v.l->items[j] = out.v.l->items[j - 1];
            j--;
        }
        out.v.l->items[j] = cur;
    }
    return out;
}

/* Forward declaration so omg_call_builtin can recurse into itself. */
static Value omg_call_builtin(Value name, Value args);

static Value omg_builtin_make_dir(Value path) {
    if (path.tag != OMG_STR) omg_panic("TypeError", "make_dir() expects a path");
    const char *full = omg_resolve_path(path.v.s);
    size_t len = strlen(full);
    if (len + 1 > 1024) omg_panic("ValueError", "path too long");
    char buf[1024];
    memcpy(buf, full, len + 1);
    while (len > 0 && (buf[len - 1] == '/' || buf[len - 1] == '\\')) {
        buf[--len] = 0;
    }
    /* mkdir each prefix; ignore EEXIST. */
    for (size_t i = 1; i <= len; i++) {
        if (i == len || buf[i] == '/' || buf[i] == '\\') {
            char saved = buf[i];
            buf[i] = 0;
            if (omg_os_mkdir(buf) != 0) {
                omg_panicf("ValueError", "cannot create directory '%s': %s (os error %d)",
                           full, strerror(errno), errno);
            }
            buf[i] = saved;
        }
    }
    return omg_bool(1);
}

/* chmod(path, mode) — set a file's permission bits (0o755 is 493). The
 * native back end writes executables itself and has to mark them
 * runnable. Windows has no such bits, so there it only checks that the
 * file exists. */
static Value omg_builtin_chmod(Value path, Value mode) {
    if (path.tag != OMG_STR || mode.tag != OMG_INT) {
        omg_panic("TypeError", "chmod() expects (path, int)");
    }
    const char *full = omg_resolve_path(path.v.s);
#ifdef _WIN32
    if (!omg_os_exists(full)) {
        omg_panicf("ValueError", "cannot chmod '%s': no such file", full);
    }
#else
    if (chmod(full, (mode_t)mode.v.i) != 0) {
        omg_panicf("ValueError", "cannot chmod '%s': %s (os error %d)",
                   full, strerror(errno), errno);
    }
#endif
    return omg_bool(1);
}

/* === TCP networking ====================================================== */

/* Two kinds of handle behind one table: a passive listener (the result
 * of tcp_listen) and a connected stream (the result of accept/connect).
 * The kind decides which calls are valid on the handle.
 *
 * A socket starts out blocking. tcp_nonblocking(h, true) changes that
 * for one socket: tcp_accept and tcp_read then return false when there
 * is nothing to take yet, and tcp_write sends what there is room for.
 * tcp_poll waits for many sockets at once. */
#ifdef _WIN32
typedef SOCKET omg_sock_t;
#define OMG_SOCK_BAD(fd)            ((fd) == INVALID_SOCKET)
#define omg_sock_close(fd)          closesocket(fd)
#define omg_sock_read(fd, buf, n)   recv((fd), (char *)(buf), (int)((n) > 0x40000000 ? 0x40000000 : (n)), 0)
#define omg_sock_write(fd, buf, n)  send((fd), (const char *)(buf), (int)((n) > 0x40000000 ? 0x40000000 : (n)), 0)
#define OMG_SOCK_EINTR()            (WSAGetLastError() == WSAEINTR)
#define OMG_SOCK_WOULD_BLOCK()      (WSAGetLastError() == WSAEWOULDBLOCK)

/* Winsock keeps its own error code and has no strerror. */
static const char *omg_sock_errstr(void) {
    static char buf[64];
    snprintf(buf, sizeof(buf), "winsock error %d", WSAGetLastError());
    return buf;
}

/* Winsock must be started once per process before the first call. */
static void omg_sock_init(void) {
    static int started = 0;
    if (started) return;
    WSADATA wsa;
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
        omg_panic("ValueError", "tcp: cannot start Winsock");
    }
    started = 1;
}
#else
typedef int omg_sock_t;
#define OMG_SOCK_BAD(fd)            ((fd) < 0)
#define omg_sock_close(fd)          close(fd)
#define omg_sock_read(fd, buf, n)   read((fd), (buf), (n))
/* send, not write: a connection the other side has closed must be an
 * error to report, not a SIGPIPE that ends the program. Where the flag
 * does not exist (macOS) the socket is marked instead, in
 * omg_sock_prepare. */
#ifdef MSG_NOSIGNAL
#define omg_sock_write(fd, buf, n)  send((fd), (buf), (n), MSG_NOSIGNAL)
#else
#define omg_sock_write(fd, buf, n)  send((fd), (buf), (n), 0)
#endif
#define OMG_SOCK_EINTR()            (errno == EINTR)
#define OMG_SOCK_WOULD_BLOCK()      (errno == EAGAIN || errno == EWOULDBLOCK)
static const char *omg_sock_errstr(void) { return strerror(errno); }
static void omg_sock_init(void) {}
#endif

typedef struct {
    omg_sock_t fd; /* OS socket, or 0 if the slot is unused */
    int is_listener;
} OmgTcpEntry;

#define OMG_MAX_TCP 4096
static OmgTcpEntry omg_tcp_table[OMG_MAX_TCP];

/* Allocate a slot. Returns a 1-based handle so 0 is always invalid. */
static int omg_tcp_alloc(omg_sock_t fd, int is_listener) {
    for (int i = 0; i < OMG_MAX_TCP; i++) {
        if (omg_tcp_table[i].fd == 0 && omg_tcp_table[i].is_listener == 0) {
            omg_tcp_table[i].fd = fd;
            omg_tcp_table[i].is_listener = is_listener;
            return i + 1;
        }
    }
    omg_panic("ValueError", "too many open tcp handles");
    return -1;
}

static OmgTcpEntry *omg_tcp_get(int handle) {
    int idx = handle - 1;
    if (idx < 0 || idx >= OMG_MAX_TCP) return NULL;
    if (omg_tcp_table[idx].fd == 0 || OMG_SOCK_BAD(omg_tcp_table[idx].fd)) return NULL;
    return &omg_tcp_table[idx];
}

/* Switch a socket between waiting and not waiting. 0 on success. */
static int omg_sock_set_nonblocking(omg_sock_t fd, int on) {
#ifdef _WIN32
    u_long mode = on ? 1 : 0;
    return ioctlsocket(fd, FIONBIO, &mode) == 0 ? 0 : -1;
#else
    int fl = fcntl(fd, F_GETFL, 0);
    if (fl < 0) return -1;
    fl = on ? (fl | O_NONBLOCK) : (fl & ~O_NONBLOCK);
    return fcntl(fd, F_SETFL, fl) == 0 ? 0 : -1;
#endif
}

/* A connection fresh from accept or connect: send what is written at
 * once (TCP_NODELAY). A program that writes a reply in two pieces would
 * otherwise have the second held back until the first is acknowledged,
 * which the other side may put off for 40 ms. */
static void omg_sock_prepare(omg_sock_t fd) {
    int one = 1;
    setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, (const char *)&one, sizeof(one));
#if !defined(_WIN32) && !defined(MSG_NOSIGNAL) && defined(SO_NOSIGPIPE)
    setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &one, sizeof(one));
#endif
}

/* Resolve "host:port" via getaddrinfo, suitable for either AI_PASSIVE
 * (listen/bind) or active (connect). Caller owns the returned addrinfo
 * and must freeaddrinfo it. Returns NULL on lookup failure (callers
 * panic with the gai_strerror string). */
static struct addrinfo *omg_resolve_addr(const char *host, int port, int passive) {
    omg_sock_init();
    struct addrinfo hints, *res = NULL;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    hints.ai_flags = passive ? AI_PASSIVE : 0;
    char port_str[8];
    snprintf(port_str, sizeof(port_str), "%d", port);
    int rc = getaddrinfo(host, port_str, &hints, &res);
    if (rc != 0) return NULL;
    return res;
}

static Value omg_builtin_tcp_listen(Value host, Value port) {
    if (host.tag != OMG_STR || port.tag != OMG_INT) {
        omg_panic("TypeError", "tcp_listen() expects (host: str, port: int)");
    }
    if (port.v.i < 0 || port.v.i > 65535) {
        omg_panicf("ValueError", "tcp_listen: port %lld out of range 0..65535",
                   (long long)port.v.i);
    }
    struct addrinfo *res = omg_resolve_addr(host.v.s, (int)port.v.i, 1);
    if (!res) {
        omg_panicf("ValueError", "tcp_listen: cannot resolve '%s'", host.v.s);
    }
    omg_sock_t fd = socket(res->ai_family, res->ai_socktype, res->ai_protocol);
    if (OMG_SOCK_BAD(fd)) {
        freeaddrinfo(res);
        omg_panicf("ValueError", "tcp_listen: socket: %s", omg_sock_errstr());
    }
#ifndef _WIN32
    /* Let a restarted server rebind its port at once. Not on Windows,
     * where SO_REUSEADDR means something else (two live listeners on
     * one port); the Rust runtime leaves it off there too. */
    int one = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one));
#endif
    if (bind(fd, res->ai_addr, (int)res->ai_addrlen) != 0) {
        const char *why = omg_sock_errstr();
        omg_sock_close(fd);
        freeaddrinfo(res);
        omg_panicf("ValueError", "tcp_listen: bind: %s", why);
    }
    freeaddrinfo(res);
    if (listen(fd, 511) != 0) {
        const char *why = omg_sock_errstr();
        omg_sock_close(fd);
        omg_panicf("ValueError", "tcp_listen: listen: %s", why);
    }
    return omg_int((int64_t)omg_tcp_alloc(fd, 1));
}

static Value omg_builtin_tcp_accept(Value h) {
    if (h.tag != OMG_INT) omg_panic("TypeError", "tcp_accept() expects a handle");
    OmgTcpEntry *e = omg_tcp_get((int)h.v.i);
    if (!e) omg_panic("ValueError", "tcp_accept: invalid tcp handle");
    if (!e->is_listener) omg_panic("ValueError", "tcp_accept: handle is a stream, not a listener");
    struct sockaddr_storage peer;
    socklen_t peer_len = sizeof(peer);
    fflush(stdout);
    omg_sock_t client_fd;
    for (;;) {
        client_fd = accept(e->fd, (struct sockaddr *)&peer, &peer_len);
        if (!OMG_SOCK_BAD(client_fd) || !OMG_SOCK_EINTR()) break;
    }
    if (OMG_SOCK_BAD(client_fd)) {
        /* A non-blocking listener with nobody waiting. */
        if (OMG_SOCK_WOULD_BLOCK()) return omg_bool(0);
        omg_panicf("ValueError", "tcp_accept: %s", omg_sock_errstr());
    }
    /* Some systems hand a connection the listener's mode. Every socket
     * starts out blocking, whatever it came from. */
    omg_sock_set_nonblocking(client_fd, 0);
    omg_sock_prepare(client_fd);
    return omg_int((int64_t)omg_tcp_alloc(client_fd, 0));
}

static Value omg_builtin_tcp_connect(Value host, Value port) {
    if (host.tag != OMG_STR || port.tag != OMG_INT) {
        omg_panic("TypeError", "tcp_connect() expects (host: str, port: int)");
    }
    if (port.v.i < 0 || port.v.i > 65535) {
        omg_panicf("ValueError", "tcp_connect: port %lld out of range 0..65535",
                   (long long)port.v.i);
    }
    struct addrinfo *res = omg_resolve_addr(host.v.s, (int)port.v.i, 0);
    if (!res) {
        omg_panicf("ValueError", "tcp_connect: cannot resolve '%s'", host.v.s);
    }
    omg_sock_t fd = 0;
    int connected = 0;
    const char *why = "no address";
    for (struct addrinfo *p = res; p != NULL; p = p->ai_next) {
        fd = socket(p->ai_family, p->ai_socktype, p->ai_protocol);
        if (OMG_SOCK_BAD(fd)) { why = omg_sock_errstr(); continue; }
        if (connect(fd, p->ai_addr, (int)p->ai_addrlen) == 0) { connected = 1; break; }
        why = omg_sock_errstr();
        omg_sock_close(fd);
    }
    freeaddrinfo(res);
    if (!connected) {
        omg_panicf("ValueError", "tcp_connect: cannot connect to '%s': %s",
                   host.v.s, why);
    }
    omg_sock_prepare(fd);
    return omg_int((int64_t)omg_tcp_alloc(fd, 0));
}

static Value omg_builtin_tcp_read(Value h, Value maxbytes) {
    if (h.tag != OMG_INT) omg_panic("TypeError", "tcp_read() expects a handle");
    if (maxbytes.tag != OMG_INT) omg_panic("TypeError", "tcp_read() expects an integer max_bytes");
    if (maxbytes.v.i < 0) omg_panic("ValueError", "tcp_read: max_bytes must be non-negative");
    OmgTcpEntry *e = omg_tcp_get((int)h.v.i);
    if (!e) omg_panic("ValueError", "tcp_read: invalid tcp handle");
    if (e->is_listener) omg_panic("ValueError", "tcp_read: handle is a listener, not a stream");
    size_t cap = (size_t)maxbytes.v.i;
    Value out;
    out.tag = OMG_LIST;
    out.v.l = omg_list_alloc(cap > 0 ? (int)cap : 1);
    if (cap == 0) return out;
    unsigned char *buf = (unsigned char *)malloc(cap);
    if (!buf) { fprintf(stderr, "out of memory\n"); exit(1); }
    int64_t n;
    for (;;) {
        n = (int64_t)omg_sock_read(e->fd, buf, cap);
        if (n >= 0 || !OMG_SOCK_EINTR()) break;
    }
    if (n < 0) {
        free(buf);
        /* A non-blocking socket with nothing to read yet. */
        if (OMG_SOCK_WOULD_BLOCK()) return omg_bool(0);
        omg_panicf("ValueError", "tcp_read: %s", omg_sock_errstr());
    }
    for (int64_t i = 0; i < n; i++) {
        omg_list_push(out.v.l, omg_int((int64_t)buf[i]));
    }
    free(buf);
    return out;
}

/* tcp_write(h, data, offset): send the bytes of data (a string or a
 * list of bytes) from byte `offset` on. Returns how many were sent:
 * all of them on a blocking socket, as many as there was room for
 * (possibly none) on a non-blocking one. */
static Value omg_builtin_tcp_write3(Value h, Value data, Value offset) {
    if (h.tag != OMG_INT) omg_panic("TypeError", "tcp_write() expects a handle");
    if (offset.tag != OMG_INT) {
        omg_panic("TypeError", "tcp_write() expects (handle: int, data: str | [int], offset: int)");
    }
    OmgTcpEntry *e = omg_tcp_get((int)h.v.i);
    if (!e) omg_panic("ValueError", "tcp_write: invalid tcp handle");
    if (e->is_listener) omg_panic("ValueError", "tcp_write: handle is a listener, not a stream");

    const unsigned char *buf = NULL;
    size_t len = 0;
    unsigned char *owned = NULL;
    int64_t off = offset.v.i;
    if (data.tag == OMG_STR) {
        size_t all = strlen(data.v.s);
        if (off < 0 || (size_t)off > all) omg_panic("ValueError", "tcp_write: offset is outside the data");
        buf = (const unsigned char *)data.v.s + off;
        len = all - (size_t)off;
    } else if (data.tag == OMG_LIST) {
        OmgList *l = data.v.l;
        if (off < 0 || off > (int64_t)l->len) omg_panic("ValueError", "tcp_write: offset is outside the data");
        len = (size_t)l->len - (size_t)off;
        if (len > 0) {
            owned = (unsigned char *)malloc(len);
            if (!owned) { fprintf(stderr, "out of memory\n"); exit(1); }
            for (int64_t i = off; i < (int64_t)l->len; i++) {
                if (l->items[i].tag != OMG_INT || l->items[i].v.i < 0 || l->items[i].v.i > 255) {
                    free(owned);
                    omg_panic("TypeError", "tcp_write() expects bytes 0-255");
                }
                owned[i - off] = (unsigned char)l->items[i].v.i;
            }
            buf = owned;
        }
    } else {
        omg_panic("TypeError", "tcp_write() expects (handle: int, data: str | [int])");
    }
    /* Loop on partial writes: a stream socket can accept fewer bytes
     * than requested under load. A non-blocking one stops when it is
     * full, and the caller is told how far it got. */
    size_t total = 0;
    while (total < len) {
        int64_t w = (int64_t)omg_sock_write(e->fd, buf + total, len - total);
        if (w < 0) {
            if (OMG_SOCK_EINTR()) continue;
            if (OMG_SOCK_WOULD_BLOCK()) break;
            if (owned) free(owned);
            omg_panicf("ValueError", "tcp_write: %s", omg_sock_errstr());
        }
        total += (size_t)w;
    }
    if (owned) free(owned);
    return omg_int((int64_t)total);
}

static Value omg_builtin_tcp_write(Value h, Value data) {
    return omg_builtin_tcp_write3(h, data, omg_int(0));
}

static Value omg_builtin_tcp_close(Value h) {
    if (h.tag != OMG_INT) omg_panic("TypeError", "tcp_close() expects a handle");
    OmgTcpEntry *e = omg_tcp_get((int)h.v.i);
    if (!e) omg_panic("ValueError", "invalid tcp handle");
    omg_sock_close(e->fd);
    e->fd = 0;
    e->is_listener = 0;
    return omg_none();
}

/* tcp_nonblocking(h, on): choose whether calls on this socket (a
 * listener or a connection) wait. */
static Value omg_builtin_tcp_nonblocking(Value h, Value on) {
    if (h.tag != OMG_INT) omg_panic("TypeError", "tcp_nonblocking() expects (handle: int, on: bool)");
    OmgTcpEntry *e = omg_tcp_get((int)h.v.i);
    if (!e) omg_panic("ValueError", "tcp_nonblocking: invalid tcp handle");
    if (omg_sock_set_nonblocking(e->fd, omg_truthy(on)) != 0) {
        omg_panicf("ValueError", "tcp_nonblocking: %s", omg_sock_errstr());
    }
    return omg_none();
}

/* tcp_peer(h): who is at the other end, as "address:port". An IPv6
 * address is written "[x:x:x:x:x:x:x:x]:port", every group in hex
 * without leading zeros, as the x86-64 runtime writes it. */
static Value omg_builtin_tcp_peer(Value h) {
    if (h.tag != OMG_INT) omg_panic("TypeError", "tcp_peer() expects a handle");
    OmgTcpEntry *e = omg_tcp_get((int)h.v.i);
    if (!e) omg_panic("ValueError", "tcp_peer: invalid tcp handle");
    if (e->is_listener) omg_panic("ValueError", "tcp_peer: handle is a listener, not a stream");
    struct sockaddr_storage ss;
    socklen_t len = sizeof(ss);
    memset(&ss, 0, sizeof(ss));
    if (getpeername(e->fd, (struct sockaddr *)&ss, &len) != 0) {
        omg_panic("ValueError", "tcp_peer: the connection has no peer");
    }
    char *out = (char *)malloc(64);
    if (!out) { fprintf(stderr, "out of memory\n"); exit(1); }
    const unsigned char *v4 = NULL;
    int port = 0;
    if (ss.ss_family == AF_INET) {
        struct sockaddr_in *a = (struct sockaddr_in *)&ss;
        v4 = (const unsigned char *)&a->sin_addr;
        port = ntohs(a->sin_port);
    } else if (ss.ss_family == AF_INET6) {
        struct sockaddr_in6 *a = (struct sockaddr_in6 *)&ss;
        const unsigned char *b = (const unsigned char *)&a->sin6_addr;
        port = ntohs(a->sin6_port);
        int mapped = b[10] == 255 && b[11] == 255;
        for (int i = 0; i < 10; i++) if (b[i] != 0) mapped = 0;
        if (mapped) {
            v4 = b + 12;
        } else {
            int at = snprintf(out, 64, "[");
            for (int i = 0; i < 8; i++) {
                at += snprintf(out + at, 64 - (size_t)at, "%s%x", i > 0 ? ":" : "",
                               (unsigned)(b[i * 2] * 256 + b[i * 2 + 1]));
            }
            snprintf(out + at, 64 - (size_t)at, "]:%d", port);
            return omg_str(out);
        }
    } else {
        free(out);
        omg_panic("ValueError", "tcp_peer: the connection has no peer");
    }
    snprintf(out, 64, "%d.%d.%d.%d:%d", v4[0], v4[1], v4[2], v4[3], port);
    return omg_str(out);
}

/* --- requests to stop ----------------------------------------------------
 *
 * interrupted(): how many times the program has been asked to stop
 * (Ctrl-C, or a request to terminate) since it first called this. The
 * first call takes these requests over: from then on they no longer
 * end the program, they are counted for it to act on. */
#ifdef _WIN32
typedef LONG omg_interrupt_t;           /* counted on a thread of its own */
#else
typedef sig_atomic_t omg_interrupt_t;
#endif
static volatile omg_interrupt_t omg_interrupt_count = 0;
static int omg_interrupt_armed = 0;

#ifdef _WIN32
static BOOL WINAPI omg_on_ctrl(DWORD kind) {
    (void)kind;
    InterlockedIncrement(&omg_interrupt_count);
    return TRUE;
}
#else
static void omg_on_signal(int sig) {
    (void)sig;
    omg_interrupt_count++;
}
#endif

static Value omg_builtin_interrupted(void) {
    if (!omg_interrupt_armed) {
        omg_interrupt_armed = 1;
#ifdef _WIN32
        SetConsoleCtrlHandler(omg_on_ctrl, TRUE);
#else
        /* No SA_RESTART: a wait in tcp_poll must end when one arrives. */
        struct sigaction sa;
        memset(&sa, 0, sizeof(sa));
        sa.sa_handler = omg_on_signal;
        sigemptyset(&sa.sa_mask);
        sigaction(SIGINT, &sa, NULL);
        sigaction(SIGTERM, &sa, NULL);
#endif
    }
    return omg_int((int64_t)omg_interrupt_count);
}

/* window_open / window_draw / window_poll / window_close: a window is
 * opened by OMG's own runtime, which speaks to the display server
 * itself. Doing the same here would mean a second copy of that, or a
 * dependency on a windowing library, so a program built through C has
 * no windows: the calls raise, and the program can catch that. */
static Value omg_builtin_window_unavailable(const char *name) {
    char msg[160];
    snprintf(msg, sizeof msg, "%s is not available in a program built through C; build it with omg --build (target linux-x64 or windows-x64)", name);
    omg_panic("ValueError", msg);
    return omg_none();
}
static Value omg_builtin_window_open(Value t, Value w, Value h) {
    (void)t; (void)w; (void)h;
    return omg_builtin_window_unavailable("window_open");
}
static Value omg_builtin_window_draw(Value win, Value pixels, Value w) {
    (void)win; (void)pixels; (void)w;
    return omg_builtin_window_unavailable("window_draw");
}
static Value omg_builtin_window_poll(Value win) {
    (void)win;
    return omg_builtin_window_unavailable("window_poll");
}
static Value omg_builtin_window_close(Value win) {
    (void)win;
    return omg_builtin_window_unavailable("window_close");
}

/* tcp_poll(reads, writes, timeout_ms): wait until a socket in `reads`
 * has something to take (data, the end of the stream, or for a
 * listener a connection), or one in `writes` has room to send, or the
 * time is up. A negative timeout waits without limit; 0 only looks.
 * Returns [readable, writable], the handles that are ready in the
 * order given. A socket in trouble counts as ready, so that the read
 * or write that follows finds out. Ends early when the program is
 * interrupted (see interrupted()). */
static Value omg_builtin_tcp_poll(Value reads, Value writes, Value timeout) {
    if (reads.tag != OMG_LIST || writes.tag != OMG_LIST || timeout.tag != OMG_INT) {
        omg_panic("TypeError", "tcp_poll() expects (reads: [int], writes: [int], timeout_ms: int)");
    }
    int nr = (int)reads.v.l->len, nw = (int)writes.v.l->len, n = nr + nw;
    omg_sock_t *socks = (omg_sock_t *)malloc(sizeof(omg_sock_t) * (size_t)(n + 1));
    char *ready = (char *)calloc((size_t)n + 1, 1);
    if (!socks || !ready) { fprintf(stderr, "out of memory\n"); exit(1); }
    for (int i = 0; i < n; i++) {
        Value v = i < nr ? reads.v.l->items[i] : writes.v.l->items[i - nr];
        OmgTcpEntry *e = v.tag == OMG_INT ? omg_tcp_get((int)v.v.i) : NULL;
        if (!e) {
            free(socks); free(ready);
            omg_panic("ValueError", "tcp_poll: invalid tcp handle");
        }
        socks[i] = e->fd;
    }
    fflush(stdout);
    int64_t left = timeout.v.i;
    if (left > 2147483647) left = 2147483647;
#ifdef _WIN32
    /* select() with sets sized for this call: an fd_set is a count and
     * an array, and Winsock reads as many entries as the count says. */
    size_t set_bytes = sizeof(u_int) * 2 + sizeof(SOCKET) * (size_t)(n + 1);
    fd_set *rs = (fd_set *)malloc(set_bytes), *ws = (fd_set *)malloc(set_bytes), *es = (fd_set *)malloc(set_bytes);
    if (!rs || !ws || !es) { fprintf(stderr, "out of memory\n"); exit(1); }
    omg_interrupt_t seen = omg_interrupt_count;
    for (;;) {
        /* Nothing wakes select when the user presses Ctrl-C, so once
         * the program listens for that the wait goes in short steps. */
        int64_t step = left;
        if (omg_interrupt_armed && (step < 0 || step > 100)) step = 100;
        int got = 0;
        if (n == 0) {
            Sleep((DWORD)(step < 0 ? 3600000 : step));
        } else {
            rs->fd_count = 0; ws->fd_count = 0; es->fd_count = 0;
            for (int i = 0; i < n; i++) {
                if (i < nr) rs->fd_array[rs->fd_count++] = socks[i];
                else ws->fd_array[ws->fd_count++] = socks[i];
                es->fd_array[es->fd_count++] = socks[i];
            }
            struct timeval tv;
            tv.tv_sec = (long)(step / 1000);
            tv.tv_usec = (long)((step % 1000) * 1000);
            got = select(0, rs, ws, es, step < 0 ? NULL : &tv);
            if (got < 0) {
                free(rs); free(ws); free(es); free(socks); free(ready);
                omg_panicf("ValueError", "tcp_poll: %s", omg_sock_errstr());
            }
            if (got > 0) {
                for (int i = 0; i < n; i++) {
                    fd_set *set = i < nr ? rs : ws;
                    for (u_int k = 0; k < set->fd_count; k++) if (set->fd_array[k] == socks[i]) ready[i] = 1;
                    for (u_int k = 0; k < es->fd_count; k++) if (es->fd_array[k] == socks[i]) ready[i] = 1;
                }
                break;
            }
        }
        if (omg_interrupt_count != seen) break;
        if (left >= 0) {
            left -= step;
            if (left <= 0) break;
        }
    }
    free(rs); free(ws); free(es);
#else
    struct pollfd *fds = (struct pollfd *)calloc((size_t)n + 1, sizeof(struct pollfd));
    if (!fds) { fprintf(stderr, "out of memory\n"); exit(1); }
    for (int i = 0; i < n; i++) {
        fds[i].fd = socks[i];
        fds[i].events = i < nr ? POLLIN : POLLOUT;
    }
    int got = poll(fds, (nfds_t)n, left < 0 ? -1 : (int)left);
    if (got < 0 && errno != EINTR) {
        free(fds); free(socks); free(ready);
        omg_panicf("ValueError", "tcp_poll: %s", strerror(errno));
    }
    if (got > 0) {
        for (int i = 0; i < n; i++) {
            if (fds[i].revents & (fds[i].events | POLLERR | POLLHUP | POLLNVAL)) ready[i] = 1;
        }
    }
    free(fds);
#endif
    Value readable, writable, out;
    readable.tag = OMG_LIST; readable.v.l = omg_list_alloc(4);
    writable.tag = OMG_LIST; writable.v.l = omg_list_alloc(4);
    for (int i = 0; i < n; i++) {
        if (!ready[i]) continue;
        if (i < nr) omg_list_push(readable.v.l, reads.v.l->items[i]);
        else omg_list_push(writable.v.l, writes.v.l->items[i - nr]);
    }
    free(socks); free(ready);
    out.tag = OMG_LIST; out.v.l = omg_list_alloc(2);
    omg_list_push(out.v.l, readable);
    omg_list_push(out.v.l, writable);
    return out;
}

/* === Process control: fork() ============================================= */

/* fork() — POSIX fork. Returns 0 in the child and the child's PID in
 * the parent (or panics on failure). Setting SIGCHLD to SIG_IGN tells
 * the kernel to auto-reap exiting children, so naive callers don't
 * accumulate zombies. */
static Value omg_builtin_fork(void) {
#ifdef _WIN32
    /* Same message as the Rust runtime on Windows. */
    omg_panic("ValueError", "fork(): not supported on Windows (there is no fork; use subprocess())");
    return omg_none();
#else
    signal(SIGCHLD, SIG_IGN);
    pid_t pid = fork();
    if (pid < 0) {
        omg_panicf("ValueError", "fork: %s", strerror(errno));
    }
    return omg_int((int64_t)pid);
#endif
}

/* === Reflective dispatch ================================================== */

/* call_builtin(name, args_list) — dispatch to another builtin by name.
 * Used by the OMG-in-OMG interpreter to evaluate generic OP_BUILTIN
 * instructions without a hard-coded switch. */
static Value omg_call_builtin(Value name, Value args) {
    if (name.tag != OMG_STR || args.tag != OMG_LIST) {
        omg_panic("TypeError", "call_builtin() expects a name and argument list");
    }
    const char *n = name.v.s;
    OmgList *l = args.v.l;
    int argc = l->len;
    Value *a = l->items;

    if (argc == 1) {
        if (strcmp(n, "length") == 0)          return omg_length(a[0]);
        if (strcmp(n, "chr") == 0)             return omg_chr(a[0]);
        if (strcmp(n, "ascii") == 0)           return omg_ascii(a[0]);
        if (strcmp(n, "freeze") == 0)          return omg_dict_freeze(a[0]);
        if (strcmp(n, "dict_keys") == 0)       return omg_dict_keys(a[0]);
        if (strcmp(n, "int") == 0)             return omg_builtin_int(a[0]);
        if (strcmp(n, "float") == 0)           return omg_builtin_float(a[0]);
        if (strcmp(n, "floor") == 0)           return omg_builtin_floor(a[0]);
        if (strcmp(n, "ceil") == 0)            return omg_builtin_ceil(a[0]);
        if (strcmp(n, "round") == 0)           return omg_builtin_round(a[0]);
        if (strcmp(n, "abs") == 0)             return omg_builtin_abs(a[0]);
        if (strcmp(n, "sqrt") == 0)            return omg_builtin_sqrt(a[0]);
        if (strcmp(n, "log") == 0)             return omg_builtin_log(a[0]);
        if (strcmp(n, "sin") == 0)             return omg_builtin_sin(a[0]);
        if (strcmp(n, "cos") == 0)             return omg_builtin_cos(a[0]);
        if (strcmp(n, "tan") == 0)             return omg_builtin_tan(a[0]);
        if (strcmp(n, "hex") == 0)             return omg_builtin_hex(a[0]);
        if (strcmp(n, "binary") == 0)          return omg_builtin_binary1(a[0]);
        if (strcmp(n, "string_bytes") == 0)    return omg_builtin_string_bytes(a[0]);
        if (strcmp(n, "bytes_to_string") == 0) return omg_builtin_bytes_to_string(a[0]);
        if (strcmp(n, "float_bits") == 0)      return omg_builtin_float_bits(a[0]);
        if (strcmp(n, "bits_to_float") == 0)   return omg_builtin_bits_to_float(a[0]);
        if (strcmp(n, "file_read") == 0)       return omg_builtin_file_read(a[0]);
        if (strcmp(n, "file_close") == 0)      return omg_builtin_file_close(a[0]);
        if (strcmp(n, "file_tell") == 0)       return omg_builtin_file_tell(a[0]);
        if (strcmp(n, "read_file") == 0)       return omg_builtin_read_file(a[0]);
        if (strcmp(n, "file_exists") == 0)     return omg_builtin_file_exists(a[0]);
        if (strcmp(n, "is_dir") == 0)          return omg_builtin_is_dir(a[0]);
        if (strcmp(n, "read_dir") == 0)        return omg_builtin_read_dir(a[0]);
        if (strcmp(n, "make_dir") == 0)        return omg_builtin_make_dir(a[0]);
        if (strcmp(n, "panic") == 0)           return omg_builtin_panic(a[0]);
        if (strcmp(n, "raise") == 0)           return omg_builtin_raise(a[0]);
        if (strcmp(n, "exit_with_error") == 0) return omg_builtin_exit_with_error(a[0]);
        if (strcmp(n, "exit") == 0)            return omg_builtin_exit(a[0]);
        if (strcmp(n, "subprocess") == 0)      return omg_builtin_subprocess(a[0]);
        if (strcmp(n, "tcp_accept") == 0)      return omg_builtin_tcp_accept(a[0]);
        if (strcmp(n, "tcp_close") == 0)       return omg_builtin_tcp_close(a[0]);
        if (strcmp(n, "tcp_peer") == 0)        return omg_builtin_tcp_peer(a[0]);
        if (strcmp(n, "window_poll") == 0)     return omg_builtin_window_poll(a[0]);
        if (strcmp(n, "window_close") == 0)    return omg_builtin_window_close(a[0]);
        if (strcmp(n, "file_size") == 0)       return omg_builtin_file_size(a[0]);
        if (strcmp(n, "file_mtime") == 0)      return omg_builtin_file_mtime(a[0]);
    }
    if (argc == 0) {
        if (strcmp(n, "getpid") == 0)            return omg_builtin_getpid();
        if (strcmp(n, "stdin_readline") == 0)    return omg_builtin_stdin_readline();
        if (strcmp(n, "stdin_read") == 0)        return omg_builtin_stdin_read();
        if (strcmp(n, "stdin_read_bytes") == 0)  return omg_builtin_stdin_read_bytes();
        if (strcmp(n, "stdin_read_key") == 0)    return omg_builtin_stdin_read_key();
        if (strcmp(n, "time_ms") == 0)           return omg_builtin_time_ms();
        if (strcmp(n, "executable_path") == 0)   return omg_builtin_executable_path();
        if (strcmp(n, "fork") == 0)              return omg_builtin_fork();
        if (strcmp(n, "interrupted") == 0)       return omg_builtin_interrupted();
    }
    if (argc == 1) {
        if (strcmp(n, "print") == 0)           return omg_builtin_print(a[0]);
        if (strcmp(n, "sleep_ms") == 0)        return omg_builtin_sleep_ms(a[0]);
        if (strcmp(n, "stdin_set_raw") == 0)   return omg_builtin_stdin_set_raw(a[0]);
        if (strcmp(n, "pty_spawn") == 0)       return omg_builtin_pty_spawn(a[0]);
        if (strcmp(n, "pty_read") == 0)        return omg_builtin_pty_read(a[0]);
        if (strcmp(n, "pty_close") == 0)       return omg_builtin_pty_close(a[0]);
    }
    if (argc == 2) {
        if (strcmp(n, "pow") == 0)        return omg_builtin_pow(a[0], a[1]);
        if (strcmp(n, "binary") == 0)     return omg_builtin_binary2(a[0], a[1]);
        if (strcmp(n, "has_key") == 0)    return omg_has_key(a[0], a[1]);
        if (strcmp(n, "list_repeat") == 0) return omg_list_repeat(a[0], a[1]);
        if (strcmp(n, "append") == 0)     return omg_builtin_append(a[0], a[1]);
        if (strcmp(n, "chmod") == 0)      return omg_builtin_chmod(a[0], a[1]);
        if (strcmp(n, "file_open") == 0)  return omg_builtin_file_open(a[0], a[1]);
        if (strcmp(n, "file_write") == 0) return omg_builtin_file_write(a[0], a[1]);
        if (strcmp(n, "file_seek") == 0)  return omg_builtin_file_seek(a[0], a[1]);
        if (strcmp(n, "call_builtin") == 0) return omg_call_builtin(a[0], a[1]);
        if (strcmp(n, "tcp_listen") == 0)  return omg_builtin_tcp_listen(a[0], a[1]);
        if (strcmp(n, "tcp_connect") == 0) return omg_builtin_tcp_connect(a[0], a[1]);
        if (strcmp(n, "tcp_read") == 0)    return omg_builtin_tcp_read(a[0], a[1]);
        if (strcmp(n, "tcp_write") == 0)   return omg_builtin_tcp_write(a[0], a[1]);
        if (strcmp(n, "tcp_nonblocking") == 0) return omg_builtin_tcp_nonblocking(a[0], a[1]);
        if (strcmp(n, "file_read") == 0)   return omg_builtin_file_read2(a[0], a[1]);
        if (strcmp(n, "pty_write") == 0)   return omg_builtin_pty_write(a[0], a[1]);
    }
    if (argc == 3) {
        if (strcmp(n, "pty_resize") == 0) return omg_builtin_pty_resize(a[0], a[1], a[2]);
        if (strcmp(n, "tcp_write") == 0)  return omg_builtin_tcp_write3(a[0], a[1], a[2]);
        if (strcmp(n, "tcp_poll") == 0)   return omg_builtin_tcp_poll(a[0], a[1], a[2]);
        if (strcmp(n, "window_open") == 0) return omg_builtin_window_open(a[0], a[1], a[2]);
        if (strcmp(n, "window_draw") == 0) return omg_builtin_window_draw(a[0], a[1], a[2]);
    }
    omg_panicf("TypeError", "unknown builtin: %s", n);
    return omg_none(); /* unreachable */
}

/* === Program entry ==========================================================
 * The generated code ends with
 *
 *     int main(int argc, char **argv) { return omg_start(argc, argv, omg_main); }
 *
 * where omg_main() holds the program's top-level statements.
 * omg_start() is the per-platform start-up:
 *
 *   POSIX    line-buffer stdout, then run omg_main.
 *   Windows  binary stdio (no "\r\n" translation), a console that
 *            understands ANSI escapes and UTF-8, the command line as
 *            UTF-8 (the `argv` Windows hands to main is in the legacy
 *            code page), and omg_main on a thread with a 64 MiB stack,
 *            because the main thread's stack size is fixed by the
 *            linker (1 or 2 MiB) and OMG recursion uses the C stack.
 */
typedef int (*OmgEntry)(int argc, char **argv);

#ifdef _WIN32
static UINT omg_saved_output_cp = 0;

static void omg_os_restore_output_cp(void) {
    if (omg_saved_output_cp != 0) SetConsoleOutputCP(omg_saved_output_cp);
}

typedef struct {
    OmgEntry entry;
    int argc;
    char **argv;
    int rc;
} OmgMainArgs;

static DWORD WINAPI omg_main_thread(LPVOID p) {
    OmgMainArgs *a = (OmgMainArgs *)p;
    a->rc = a->entry(a->argc, a->argv);
    return 0;
}

static int omg_start(int argc, char **argv, OmgEntry entry) {
    _setmode(_fileno(stdin), _O_BINARY);
    _setmode(_fileno(stdout), _O_BINARY);
    _setmode(_fileno(stderr), _O_BINARY);

    /* Windows Terminal interprets ANSI escapes already; the classic
     * console host has to be asked. Fails harmlessly on a pipe. */
    DWORD which[2] = {STD_OUTPUT_HANDLE, STD_ERROR_HANDLE};
    for (int i = 0; i < 2; i++) {
        HANDLE h = GetStdHandle(which[i]);
        DWORD mode = 0;
        if (h != NULL && h != INVALID_HANDLE_VALUE && GetConsoleMode(h, &mode)) {
            SetConsoleMode(h, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
    }
    /* OMG strings are UTF-8 and are written as bytes, so the console
     * must decode UTF-8. The code page belongs to the console, not to
     * this process: put the old one back on exit. */
    UINT cp = GetConsoleOutputCP();
    if (cp != 0 && cp != CP_UTF8 && SetConsoleOutputCP(CP_UTF8)) {
        omg_saved_output_cp = cp;
        atexit(omg_os_restore_output_cp);
    }

    int wargc = 0;
    LPWSTR *wargv = CommandLineToArgvW(GetCommandLineW(), &wargc);
    if (wargv != NULL && wargc > 0) {
        char **u = (char **)malloc(((size_t)wargc + 1) * sizeof(char *));
        if (u) {
            for (int i = 0; i < wargc; i++) {
                u[i] = omg_os_utf8(wargv[i]);
                if (!u[i]) u[i] = (char *)"";
            }
            u[wargc] = NULL;
            argc = wargc;
            argv = u;
        }
        LocalFree(wargv);
    }

    OmgMainArgs a;
    a.entry = entry;
    a.argc = argc;
    a.argv = argv;
    a.rc = 0;
    HANDLE t = CreateThread(NULL, (SIZE_T)64 * 1024 * 1024, omg_main_thread, &a,
                            STACK_SIZE_PARAM_IS_A_RESERVATION, NULL);
    if (t == NULL) return entry(argc, argv);
    WaitForSingleObject(t, INFINITE);
    CloseHandle(t);
    return a.rc;
}
#else
static int omg_start(int argc, char **argv, OmgEntry entry) {
    /* Line-buffered even when piped, the way Rust's println! flushes
     * on "\n". Matters for error tests, where a panic on stderr must
     * come AFTER the emits before it. */
    setvbuf(stdout, NULL, _IOLBF, 0);
    return entry(argc, argv);
}
#endif
