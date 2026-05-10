/*
 * omg_rt.h — the OMG C runtime, prepended to every program emitted by
 * `bootstrap/native-c.omg`.
 */
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <setjmp.h>
#include <stdarg.h>
#include <errno.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <dirent.h>
#include <unistd.h>

typedef enum {
    OMG_INT,
    OMG_STR,
    OMG_BOOL,
    OMG_NONE,
    OMG_CLOSURE,
    OMG_LIST,
    OMG_DICT,
    OMG_FLOAT
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
 * If a proc ever needs more than MAX_ARITY parameters, raise the cap
 * here and at the call sites — but in practice OMG procs use ≤4. */
#define OMG_MAX_ARITY 8

typedef Value (*OmgFn)(Value *captured, int cap_count, int argc,
                       Value a0, Value a1, Value a2, Value a3,
                       Value a4, Value a5, Value a6, Value a7);

typedef struct OmgClosure {
    int rc;
    OmgFn fn;
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

/* Build a closure value. `captured` may be NULL when cap_count == 0
 * (top-level procs that don't capture anything). Refcounted: the
 * caller receives a closure with rc=1; transferring that reference
 * into a slot doesn't bump it, but copying it (LOAD, etc.) does. */
static inline Value omg_closure(OmgFn fn, Value *captured, int cap_count) {
    OmgClosure *c = (OmgClosure *)malloc(sizeof(OmgClosure));
    c->rc = 1;
    c->fn = fn;
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
        longjmp(b->jb, 1);
    }
    fprintf(stderr, "%s\n", full);
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

/* Coerce `v` to int64. Mirrors `Value::as_int` in runtime/src/value.rs:
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

/* `/` is promote-on-float: int÷int stays floor division (matches OMG
 * VM); any-float promotes both to double and does true division. Use
 * `//` for explicit floor division. */
static inline Value omg_div(Value a, Value b) {
    if (omg_is_float(a) || omg_is_float(b)) {
        double bd = omg_as_double(b);
        if (bd == 0.0) omg_panic("ZeroDivisionError", "integer division or modulo by zero");
        return omg_float(omg_as_double(a) / bd);
    }
    int64_t x = omg_as_int(a), y = omg_as_int(b);
    if (y == 0) omg_panic("ZeroDivisionError", "integer division or modulo by zero");
    int64_t q = x / y;
    if ((x % y != 0) && ((x < 0) != (y < 0))) q -= 1;
    return omg_int(q);
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
    Value *vals;
    int frozen;
} OmgDict;

static OmgDict *omg_dict_alloc(void) {
    OmgDict *d = (OmgDict *)malloc(sizeof(OmgDict));
    if (!d) { fprintf(stderr, "out of memory\n"); exit(1); }
    d->rc = 1;
    d->len = 0;
    d->cap = 0;
    d->keys = NULL;
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

static int omg_dict_find(OmgDict *d, const char *key) {
    for (int i = 0; i < d->len; i++) {
        if (strcmp(d->keys[i], key) == 0) return i;
    }
    return -1;
}

static void omg_dict_grow(OmgDict *d) {
    if (d->len < d->cap) return;
    int newcap = d->cap < 4 ? 4 : d->cap * 2;
    char **nk = (char **)realloc(d->keys, newcap * sizeof(char *));
    Value *nv = (Value *)realloc(d->vals, newcap * sizeof(Value));
    if (!nk || !nv) { fprintf(stderr, "out of memory\n"); exit(1); }
    d->keys = nk;
    d->vals = nv;
    d->cap = newcap;
}

static void omg_dict_set(OmgDict *d, const char *key, Value v) {
    int idx = omg_dict_find(d, key);
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

static void omg_print_value(Value v) {
    char buf[64];
    switch (v.tag) {
        case OMG_INT:     printf("%lld", (long long)v.v.i); break;
        case OMG_FLOAT:   fputs(omg_float_format(v.v.f, buf, sizeof(buf)), stdout); break;
        case OMG_STR:     fputs(v.v.s, stdout); break;
        case OMG_BOOL:    fputs(v.v.b ? "true" : "false", stdout); break;
        case OMG_NONE:    break;
        case OMG_CLOSURE: fputs("<proc>", stdout); break;
        case OMG_LIST: {
            putchar('[');
            for (int i = 0; i < v.v.l->len; i++) {
                if (i) fputs(", ", stdout);
                omg_print_value(v.v.l->items[i]);
            }
            putchar(']');
            break;
        }
        case OMG_DICT: {
            putchar('{');
            for (int i = 0; i < v.v.d->len; i++) {
                if (i) fputs(", ", stdout);
                fputs(v.v.d->keys[i], stdout);
                fputs(": ", stdout);
                omg_print_value(v.v.d->vals[i]);
            }
            putchar('}');
            break;
        }
    }
    (void)buf;
}

/* Stringify a list/dict for use in a string-concat context. Allocates a
 * fresh malloc'd buffer; leaked along with the rest of phase-5 storage. */
static char *omg_compound_to_cstr(Value v) {
    /* Use a temporary memstream-style growing buffer. C99 doesn't have
     * memstream; allocate generously and grow if needed. */
    size_t cap = 256, len = 0;
    char *buf = (char *)malloc(cap);
    if (!buf) { fprintf(stderr, "out of memory\n"); exit(1); }
    /* Sub-helper: render Value into the growing buffer. */
    /* For simplicity we re-render via snprintf into a scratch then
     * concatenate. This isn't fast but is correct. */
    /* Render to a temp file via fmemopen would be ideal, but POSIX-only.
     * We'll keep it portable: format with omg_print_value into a popen-
     * style sink? Easiest: use a recursive helper that writes into
     * the buffer. */
    /* For simplicity here, reuse omg_print_value via a temp memstream.
     * Most platforms support it; fall back to a simple length-only
     * serialiser if needed. We rely on POSIX open_memstream. */
    {
        size_t mlen = 0;
        char *mptr = NULL;
        FILE *m = open_memstream(&mptr, &mlen);
        if (!m) { fprintf(stderr, "open_memstream failed\n"); exit(1); }
        FILE *saved = stdout;
        /* Trick: repurpose stdout temporarily. Easier than refactoring
         * omg_print_value to take a FILE*. */
        stdout = m;
        omg_print_value(v);
        fflush(m);
        stdout = saved;
        fclose(m);
        free(buf);
        return mptr;
    }
    (void)cap; (void)len;
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
    omg_panic("TypeError", "cannot index this value");
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
        default: break;
    }
    omg_panic("TypeError", "length() expects list or string (type mismatch)");
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
        double diff = fabs(f - trunc(f));
        if (fabs(diff - 0.5) < 1e-9) {
            /* Exactly halfway: pick the even neighbour. */
            double down = trunc(f);
            double up = f >= 0.0 ? down + 1.0 : down - 1.0;
            r = (((int64_t)down) % 2 == 0) ? down : up;
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
        int64_t mask = ((int64_t)1 << width) - 1;
        int64_t masked = n & mask;
        char *buf = omg_str_alloc((size_t)width + 1);
        for (int64_t i = 0; i < width; i++) {
            buf[width - 1 - i] = (masked >> i) & 1 ? '1' : '0';
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
 * Used by the OMG-native `omg`/`omg-build` drivers to propagate child
 * exit codes from subprocess() back up to the shell. */
static Value omg_builtin_exit(Value v) {
    if (v.tag != OMG_INT) omg_panic("TypeError", "exit() expects an integer");
    fflush(stdout);
    fflush(stderr);
    exit((int)v.v.i);
    return omg_none(); /* unreachable */
}

/* getpid(): return the process ID. Used by the OMG-native drivers to
 * generate unique tempfile paths (e.g. /tmp/omg-<pid>.omgb) without
 * needing a mktemp builtin. */
static Value omg_builtin_getpid(void) {
    return omg_int((int64_t)getpid());
}

/* executable_path(): absolute path of the currently running binary, so
 * the toolchain can locate sibling files (omg_rt.h) regardless of how
 * it was invoked. Returns `false` on platforms where we don't have a
 * way to look this up — callers fall back to dirname(args[0]).
 *
 * Only Linux is implemented for now (readlink on /proc/self/exe). Mac
 * would use _NSGetExecutablePath; BSDs vary. The fallback path keeps
 * the toolchain working on those platforms when invoked via an
 * absolute or relative path. */
static Value omg_builtin_executable_path(void) {
#if defined(__linux__)
    char buf[4096];
    ssize_t n = readlink("/proc/self/exe", buf, sizeof(buf) - 1);
    if (n < 0) return omg_bool(0);
    buf[n] = 0;
    char *out = (char *)malloc((size_t)n + 1);
    if (!out) { fprintf(stderr, "out of memory\n"); exit(1); }
    memcpy(out, buf, (size_t)n + 1);
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

/* subprocess(argv): fork + execvp + waitpid. argv is a list of strings;
 * argv[0] is the program (PATH-resolved via execvp), the rest are args.
 * stdin/stdout/stderr are inherited. Returns the child's exit code.
 *
 * Mirrors std::process::Command::status() in the Rust runtime so the
 * OMG-native `omg` driver can dispatch to omgc/omgvm/cc the same way
 * the bash wrapper used to. */
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

    pid_t pid = fork();
    if (pid < 0) {
        free(argv);
        omg_panicf("ValueError", "subprocess: fork failed: %s", strerror(errno));
    }
    if (pid == 0) {
        /* Child: exec replaces this process. */
        execvp(argv[0], argv);
        /* If we get here, exec failed. Mirror Rust's "cannot exec" path. */
        fprintf(stderr, "subprocess: cannot exec '%s': %s\n", argv[0], strerror(errno));
        _exit(127);
    }

    /* Parent: wait for child. */
    int status = 0;
    if (waitpid(pid, &status, 0) < 0) {
        free(argv);
        omg_panicf("ValueError", "subprocess: waitpid failed: %s", strerror(errno));
    }
    free(argv);

    int code;
    if (WIFEXITED(status))        code = WEXITSTATUS(status);
    else if (WIFSIGNALED(status)) code = 128 + WTERMSIG(status);
    else                          code = -1;
    return omg_int((int64_t)code);
}

/* === File I/O ============================================================ */

/* Path-resolution base. main() initialises this from getcwd(); file
 * builtins join relative paths against it the same way the OMG VM's
 * resolve_path() does. Mutating v_current_dir from inside a program
 * does NOT update this — that's a documented divergence from the VM,
 * which honours mid-program changes; we don't because the runtime
 * header has no dynamic link to user globals. */
static const char *omg_cwd_str = ".";

/* Slash-or-drive-letter detection — keep cheap and ASCII. */
static const char *omg_resolve_path(const char *p) {
    if (p == NULL) return p;
    int absolute = (p[0] == '/') || (p[0] && p[1] == ':');
    if (absolute) return p;
    size_t lb = strlen(omg_cwd_str), lp = strlen(p);
    char *buf = (char *)malloc(lb + 2 + lp);
    if (!buf) { fprintf(stderr, "out of memory\n"); exit(1); }
    memcpy(buf, omg_cwd_str, lb);
    if (lb > 0 && omg_cwd_str[lb - 1] != '/') {
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
    FILE *fp = fopen(full, fm);
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
    if (fseek(e->fp, (long)off.v.i, SEEK_SET) != 0) {
        omg_panicf("ValueError", "file_seek: %s", strerror(errno));
    }
    return omg_int(off.v.i);
}

/* file_tell(handle): current absolute byte position of the handle. */
static Value omg_builtin_file_tell(Value h) {
    if (h.tag != OMG_INT) omg_panic("TypeError", "file_tell() expects a handle");
    OmgFileEntry *e = omg_file_get((int)h.v.i);
    if (!e) omg_panic("ValueError", "invalid file handle");
    long pos = ftell(e->fp);
    if (pos < 0) {
        omg_panicf("ValueError", "file_tell: %s", strerror(errno));
    }
    return omg_int((int64_t)pos);
}

static Value omg_builtin_read_file(Value path) {
    if (path.tag != OMG_STR) omg_panic("TypeError", "read_file() expects a file path");
    const char *full = omg_resolve_path(path.v.s);
    FILE *fp = fopen(full, "r");
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
    struct stat st;
    return omg_bool(stat(full, &st) == 0);
}

static Value omg_builtin_is_dir(Value path) {
    if (path.tag != OMG_STR) omg_panic("TypeError", "is_dir() expects a path");
    const char *full = omg_resolve_path(path.v.s);
    struct stat st;
    if (stat(full, &st) != 0) return omg_bool(0);
    return omg_bool((st.st_mode & S_IFMT) == S_IFDIR);
}

static Value omg_builtin_read_dir(Value path) {
    if (path.tag != OMG_STR) omg_panic("TypeError", "read_dir() expects a directory path");
    const char *full = omg_resolve_path(path.v.s);
    DIR *dir = opendir(full);
    if (!dir) {
        omg_panicf("ValueError", "cannot read directory '%s': %s (os error %d)",
                   full, strerror(errno), errno);
    }
    Value out;
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
            if (mkdir(buf, 0755) != 0 && errno != EEXIST) {
                omg_panicf("ValueError", "cannot create directory '%s': %s (os error %d)",
                           full, strerror(errno), errno);
            }
            buf[i] = saved;
        }
    }
    return omg_bool(1);
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
    }
    if (argc == 0) {
        if (strcmp(n, "getpid") == 0)            return omg_builtin_getpid();
        if (strcmp(n, "stdin_readline") == 0)    return omg_builtin_stdin_readline();
        if (strcmp(n, "stdin_read") == 0)        return omg_builtin_stdin_read();
        if (strcmp(n, "stdin_read_bytes") == 0)  return omg_builtin_stdin_read_bytes();
        if (strcmp(n, "executable_path") == 0)   return omg_builtin_executable_path();
    }
    if (argc == 1) {
        if (strcmp(n, "print") == 0)           return omg_builtin_print(a[0]);
    }
    if (argc == 2) {
        if (strcmp(n, "pow") == 0)        return omg_builtin_pow(a[0], a[1]);
        if (strcmp(n, "binary") == 0)     return omg_builtin_binary2(a[0], a[1]);
        if (strcmp(n, "file_open") == 0)  return omg_builtin_file_open(a[0], a[1]);
        if (strcmp(n, "file_write") == 0) return omg_builtin_file_write(a[0], a[1]);
        if (strcmp(n, "file_seek") == 0)  return omg_builtin_file_seek(a[0], a[1]);
        if (strcmp(n, "call_builtin") == 0) return omg_call_builtin(a[0], a[1]);
    }
    omg_panicf("TypeError", "unknown builtin: %s", n);
    return omg_none(); /* unreachable */
}
