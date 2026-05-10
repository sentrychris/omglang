# omgdb — SQL-subset embedded database

## Goal

A SQLite-shaped tool, all in OMG: persistent on-disk pages, a B-tree
storage engine, a SQL parser, and an executor. Targets a useful
subset, not a complete SQL implementation.

## Why this is a showcase

This is the "real systems programming" demo. It exercises OMG on:

- **Binary file format design** (page layout, big-endian integer
  packing, free-list management).
- **Algorithmic complexity** (B-tree split/merge logic is famously
  fiddly to get right).
- **Recursive descent parsing** of a non-trivial grammar.
- **Long-lived persistent state**: a `.db` file that survives between
  invocations and stays consistent under repeated mutation.

If the existing toolchain has bugs in file I/O, slicing, or integer
arithmetic at scale, this project surfaces them.

## SQL subset

Just enough to feel like SQL. No transactions, no joins, no indexes.

```sql
CREATE TABLE users (id INT, name TEXT, age INT);
INSERT INTO users VALUES (1, "Alice", 30);
SELECT * FROM users WHERE age > 25 ORDER BY name;
DELETE FROM users WHERE id = 1;
DROP TABLE users;
```

Types: `INT` (i64) and `TEXT` (UTF-8). No NULL, no FLOAT, no BLOB in
v1. Row format is `[type_tag, value, type_tag, value, ...]`.

## File layout

```
tools/db/
├── omgdb.omg          ← entry point: REPL or single-statement mode
├── btree.omg          ← page-based B-tree (split, merge, search)
├── sql.omg            ← lexer + recursive-descent parser → AST
├── exec.omg           ← AST → query plan → result rows
├── wire.omg           ← page read/write, byte packing/unpacking
└── tests/
    ├── btree_test.omg
    ├── sql_test.omg
    └── roundtrip_test.omg
```

## On-disk format

Fixed 4096-byte pages. Page 0 is the metadata page (table catalog).
Pages 1+ are B-tree nodes or overflow data.

```
Page header (16 bytes):
  +0   u8     page_type (0=meta, 1=internal, 2=leaf, 3=overflow)
  +1   u8     reserved
  +2   u16    cell_count
  +4   u16    free_offset (where the next cell would go)
  +6   u32    right_child (internal only) or next_overflow (overflow only)
  +10  u32    parent_page
  +14  u16    reserved
```

Cells store rowid + serialised row. Cell layout:

```
  +0   u32    rowid
  +4   u32    payload_len
  +8   ...    payload (type-tagged values)
```

Each table is one B-tree, indexed by rowid (auto-incrementing). The
meta page maps `table_name → root_page_number`.

## Architecture per layer

### wire.omg (~300 lines)
- `read_page(fh, n) → bytes`
- `write_page(fh, n, bytes)`
- `pack_u32(n) → bytes`, `unpack_u32(bytes, off) → int`
- `pack_text(s) → bytes`, `unpack_text(bytes, off) → [s, new_off]`
- Uses `file_open(path, "rb+")` if available (see Risks below).

### btree.omg (~600 lines)
- `btree_search(root, key) → cell or none`
- `btree_insert(root, key, payload) → new_root` (returns updated root
  page number; may change if root splits)
- `btree_delete(root, key) → new_root`
- `btree_iterate(root) → list of cells` (in-order traversal)
- All ops are page-local; the B-tree never holds the full tree in memory.

### sql.omg (~700 lines)
- `tokenise(src) → list of tokens`
- `parse(tokens) → ast`. AST shape:
  ```omg
  {kind: "select", columns: ["*"], table: "users",
   where: {op: ">", left: "age", right: 25},
   order_by: "name"}
  ```
- Recursive-descent. No precedence-climbing needed for this subset
  (only one tier of WHERE comparisons).

### exec.omg (~500 lines)
- `execute(ast, db) → rows` (or `rows_affected` for DML)
- For SELECT: open the table's B-tree, iterate, filter by WHERE, sort
  by ORDER BY (in-memory), project columns.
- For INSERT: serialise row, btree_insert.
- For DELETE: iterate, collect matching rowids, btree_delete each.
- For CREATE/DROP: update meta page.

### omgdb.omg (~300 lines)
- Bare invocation drops into a REPL: `> SELECT * FROM users;`
- `omgdb file.db -e "SQL ..."` runs one statement and exits.
- Pipes: `cat schema.sql | omgdb file.db` runs the whole script.

## Scope

| Piece | Lines |
|---|---|
| wire.omg | ~300 |
| btree.omg | ~600 |
| sql.omg | ~700 |
| exec.omg | ~500 |
| omgdb.omg | ~300 |
| tests | ~600 |
| **Total** | **~3000** |

~3–5 days.

## Testing strategy

- **Golden-file tests for the page format**: insert a known sequence
  of rows, write the database, hex-dump pages 0–N, compare against a
  checked-in golden hex dump. Catches accidental layout drift.
- **B-tree fuzzing-lite**: insert 1000 random keys, then delete them
  in a different random order, asserting the tree stays valid (well-
  formed pages, no orphaned nodes, in-order iteration matches sorted
  insertion set).
- **SQL round-trip**: a fixture script that creates a table, inserts
  ~50 rows, runs ~20 SELECTs, deletes some, runs SELECTs again. Compare
  output to a golden file.

## Risks

- **`file_open(path, "rb+")` may not exist.** OMG today supports `r`,
  `rb`, `w`, `wb`, `a`, `ab`. A B-tree needs random-access read+write.
  *Fix*: add `rb+` and `wb+` modes (open existing for read+write,
  truncate-and-open for read+write). ~20 lines in `omg_rt.h` and
  `runtime/src/vm/builtins.rs`. Probably worth doing as a prerequisite.
- **No `seek()` builtin.** Same issue. Need `file_seek(fh, off)` and
  probably `file_tell(fh)`. Two more builtins. Page-based access
  fundamentally needs them.
- **B-tree bugs corrupt data.** Mitigation: build the flat-file
  variant (page list, no tree) first to validate the SQL/exec layers
  on a simpler storage backend. Swap in B-tree once it's tested in
  isolation.
- **Strings in OMG are immutable.** Building large byte sequences via
  `+` is O(n²). Use `file_write` with a list of bytes for output, and
  for in-memory page assembly use a list-of-ints buffer that's joined
  via `bytes_to_string` at write time.

## Prerequisites worth landing first

1. `file_open(path, "rb+")` and `"wb+"` modes.
2. `file_seek(fh, offset)` + `file_tell(fh)` builtins.
3. Tests for both, in `tests/builtins.sh`.

These belong in the language regardless of omgdb.

## Where to start

1. **Day 1**: file_seek / rb+ / wb+ builtins, with tests. Land
   separately as their own commit/PR.
2. **Day 2**: wire.omg + a flat-file-only storage backend
   (no tree — just append). Get INSERT and SELECT round-tripping.
3. **Day 3**: SQL parser. Scaffolding only — focus on the four
   statement types.
4. **Day 4**: B-tree. Pure-OMG, no I/O at first (operate on in-memory
   page-shaped buffers); plug into wire.omg once it works.
5. **Day 5**: REPL, polish, golden tests, README.

## Done means

- A `tools/db/omgdb.omg` that creates a `.db` file, accepts the four
  SQL statement types, persists across invocations.
- `tests/run.sh db` (new suite) passes B-tree fuzzing, SQL round-trip,
  golden page format.
- README example: walk-through of a small users table.

## Open questions

- Worth supporting transactions (a journal file)? Probably not v1.
- Index support beyond the implicit rowid B-tree? Not v1; stretch goal.
- Should the on-disk format be versioned (a magic + version header on
  page 0)? Yes — costs nothing to add, makes future format changes
  diagnosable.
