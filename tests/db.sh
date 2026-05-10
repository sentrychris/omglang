#!/bin/bash
# Tests for the omgdb storage layer (tools/db/). Exercises:
#   * Round-trip: create / insert / select via the OMG API
#   * Multi-page: enough rows to force a second-leaf allocation
#   * Persistence: changes survive close + reopen
#   * Cross-backend parity (Rust runtime vs native interpreted vs AOT)
#
# These are storage-layer tests; SQL parsing arrives in Day 3.

set -u
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
require_native_toolchain

cd "$REPO_ROOT"

# === Round-trip: API-level INSERT then SELECT ============================
section "DB: programmatic round-trip"

# Drop a tiny driver next to tools/db/ so its imports of wire/exec
# resolve relative to the importer's directory.
cat > "$REPO_ROOT/tools/db/_test_basic.omg" <<EOF
;;;omg
import "exec.omg" as exec

alloc db := exec.open_db(args[1])
exec.create_table(db, "users", ["id", "name", "age"])
exec.insert(db, "users", [1, "Alice", 30])
exec.insert(db, "users", [2, "Bob", 25])
alloc rows := exec.select_all(db, "users")
emit "n=" + length(rows)
alloc i := 0
loop i < length(rows) {
    alloc r := rows[i]
    emit r[0] + ":" + r[1][0] + "/" + r[1][1] + "/" + r[1][2]
    i := i + 1
}
exec.close_db(db)
EOF

expected="n=2
1:1/Alice/30
2:2/Bob/25"

actual=$("$OMG_RUST" "$REPO_ROOT/tools/db/_test_basic.omg" "$TMPDIR_TEST/basic_rust.db" 2>&1)
assert_eq "API round-trip: Rust runtime" "$expected" "$actual"

actual=$("$OMG_NATIVE" "$REPO_ROOT/tools/db/_test_basic.omg" "$TMPDIR_TEST/basic_native.db" 2>&1)
assert_eq "API round-trip: native interpreted" "$expected" "$actual"

"$OMG_NATIVE" --build "$REPO_ROOT/tools/db/_test_basic.omg" "$TMPDIR_TEST/basic_aot" >/dev/null
actual=$("$TMPDIR_TEST/basic_aot" "$TMPDIR_TEST/basic_aot.db" 2>&1)
assert_eq "API round-trip: AOT binary" "$expected" "$actual"

rm -f "$REPO_ROOT/tools/db/_test_basic.omg"

# === Persistence: changes survive close + reopen =========================
section "DB: persistence across reopen"

# Phase 1 writes the table; phase 2 (separate process) reads it back.
cat > "$REPO_ROOT/tools/db/_test_write.omg" <<EOF
;;;omg
import "exec.omg" as exec
alloc db := exec.open_db(args[1])
exec.create_table(db, "kv", ["k", "v"])
exec.insert(db, "kv", ["alpha", 1])
exec.insert(db, "kv", ["beta", 2])
exec.close_db(db)
EOF
cat > "$REPO_ROOT/tools/db/_test_read.omg" <<EOF
;;;omg
import "exec.omg" as exec
alloc db := exec.open_db(args[1])
alloc rows := exec.select_all(db, "kv")
alloc i := 0
loop i < length(rows) {
    emit rows[i][1][0] + "=" + rows[i][1][1]
    i := i + 1
}
exec.close_db(db)
EOF

DB="$TMPDIR_TEST/persist.db"
rm -f "$DB"
"$OMG_RUST" "$REPO_ROOT/tools/db/_test_write.omg" "$DB"
actual=$("$OMG_RUST" "$REPO_ROOT/tools/db/_test_read.omg" "$DB")
assert_eq "Rust → Rust round trip across processes" "alpha=1
beta=2" "$actual"

rm -f "$DB"
"$OMG_RUST" "$REPO_ROOT/tools/db/_test_write.omg" "$DB"
actual=$("$OMG_NATIVE" "$REPO_ROOT/tools/db/_test_read.omg" "$DB")
assert_eq "Rust write → native read (cross-backend persistence)" "alpha=1
beta=2" "$actual"

rm -f "$DB"
"$OMG_NATIVE" "$REPO_ROOT/tools/db/_test_write.omg" "$DB"
actual=$("$OMG_RUST" "$REPO_ROOT/tools/db/_test_read.omg" "$DB")
assert_eq "Native write → Rust read (cross-backend persistence)" "alpha=1
beta=2" "$actual"

rm -f "$REPO_ROOT/tools/db/_test_write.omg" \
      "$REPO_ROOT/tools/db/_test_read.omg"

# === Multi-page: 200 rows force a second leaf-page allocation =============
section "DB: multi-page allocation"

cat > "$REPO_ROOT/tools/db/_test_bulk.omg" <<EOF
;;;omg
import "exec.omg" as exec
alloc db := exec.open_db(args[1])
exec.create_table(db, "nums", ["n"])
# 400 rows × ~17 bytes/cell = ~6800 bytes total — guaranteed to spill
# beyond the ~4080 bytes of cell space in a single 4 KiB leaf page.
alloc i := 1
loop i <= 400 {
    exec.insert(db, "nums", [i])
    i := i + 1
}
alloc rows := exec.select_all(db, "nums")
emit "count=" + length(rows)
emit "first=" + rows[0][1][0]
emit "last=" + rows[length(rows) - 1][1][0]
exec.close_db(db)
EOF
DB="$TMPDIR_TEST/bulk.db"
rm -f "$DB"
actual=$("$OMG_RUST" "$REPO_ROOT/tools/db/_test_bulk.omg" "$DB" 2>&1)
expected="count=400
first=1
last=400"
assert_eq "400 rows round-trip (Rust)" "$expected" "$actual"

# File should have at least 3 pages (meta + 2 leaf).
size=$(stat -c%s "$DB")
if [ "$size" -ge 12288 ]; then
    pass "400-row file allocated ≥3 pages (size=$size)"
else
    fail "400-row file too small (size=$size, expected ≥12288)"
fi

rm -f "$REPO_ROOT/tools/db/_test_bulk.omg"

# === Catalog: list_tables / table_columns =================================
section "DB: catalog metadata"

cat > "$REPO_ROOT/tools/db/_test_catalog.omg" <<EOF
;;;omg
import "exec.omg" as exec
alloc db := exec.open_db(args[1])
exec.create_table(db, "users", ["id", "name"])
exec.create_table(db, "posts", ["id", "title", "body"])
alloc tables := exec.list_tables(db)
emit "tables=" + length(tables)
alloc i := 0
loop i < length(tables) {
    alloc t := tables[i]
    alloc cols_str := ""
    alloc j := 0
    loop j < length(t.columns) {
        if j > 0 { cols_str := cols_str + "," }
        cols_str := cols_str + t.columns[j]
        j := j + 1
    }
    emit t.name + ":" + cols_str
    i := i + 1
}
exec.close_db(db)
EOF
DB="$TMPDIR_TEST/catalog.db"
rm -f "$DB"
# OMG dict_keys() iteration order isn't stable across implementations
# (despite the README claim), so sort the per-table lines before
# comparing. The header line stays put.
raw=$("$OMG_RUST" "$REPO_ROOT/tools/db/_test_catalog.omg" "$DB" 2>&1)
sorted=$(printf '%s\n' "$raw" | { read -r head; printf '%s\n' "$head"; sort; })
expected="tables=2
posts:id,title,body
users:id,name"
assert_eq "list_tables / table_columns" "$expected" "$sorted"

rm -f "$REPO_ROOT/tools/db/_test_catalog.omg"

# === Reject duplicate table name ==========================================
section "DB: error paths"

cat > "$REPO_ROOT/tools/db/_test_dup.omg" <<EOF
;;;omg
import "exec.omg" as exec
alloc db := exec.open_db(args[1])
exec.create_table(db, "t", ["c"])
try {
    exec.create_table(db, "t", ["c"])
    emit "no-error"
} except err {
    emit "caught"
}
exec.close_db(db)
EOF
DB="$TMPDIR_TEST/dup.db"
rm -f "$DB"
actual=$("$OMG_RUST" "$REPO_ROOT/tools/db/_test_dup.omg" "$DB" 2>&1)
assert_eq "duplicate table name rejected (Rust)" "caught" "$actual"

cat > "$REPO_ROOT/tools/db/_test_missing.omg" <<EOF
;;;omg
import "exec.omg" as exec
alloc db := exec.open_db(args[1])
try {
    alloc _ := exec.select_all(db, "ghost")
    emit "no-error"
} except err {
    emit "caught"
}
exec.close_db(db)
EOF
DB="$TMPDIR_TEST/missing.db"
rm -f "$DB"
actual=$("$OMG_RUST" "$REPO_ROOT/tools/db/_test_missing.omg" "$DB" 2>&1)
assert_eq "missing table rejected (Rust)" "caught" "$actual"

rm -f "$REPO_ROOT/tools/db/_test_dup.omg" \
      "$REPO_ROOT/tools/db/_test_missing.omg"
