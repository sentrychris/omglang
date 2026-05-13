# omgdb

A small SQL database **written in OMG**. Stores tables in a single paged
file on disk (4 KB pages, big-endian wire format) and ships with a
SQLite-style REPL.

It's a learning project, not a production engine, there are no transactions,
no indexes, no joins etc., but it does cover vital underlying parts such as an 
SQL lexer/parser, a paged on-disk format with multi-page tables, a catalog, 
persistence across reopens, and a REPL.

## Get it running

You need the OMG runtime on your `PATH` as `omg` (see the top-level
[`README.md`](../../README.md#get-it-running)). Then:

```sh
omg tools/db/omgdb.omg mydata.db
```

That opens (or creates) `mydata.db` and drops you at the `omgdb>`
prompt. The file is created on first write.

## Invocation modes

```sh
omg tools/db/omgdb.omg <file.db>                       # interactive REPL
omg tools/db/omgdb.omg <file.db> -e "SQL ..."          # one-shot batch
cat schema.sql | omg tools/db/omgdb.omg <file.db> -    # SQL from stdin
```

The `-e` and stdin forms accept any number of statements separated by
`;`. Errors inside one statement don't abort the rest, the REPL keeps
prompting and batch mode keeps executing.

In the REPL, end a statement with `;` to run it. A line without `;`
gets a `...>` continuation prompt and buffers until terminated. `quit`,
`exit`, or `\q` (or Ctrl-D) leaves.

## A two-minute tour

```
$ omg tools/db/omgdb.omg people.db
OMG database (omgdb). Type SQL terminated by `;`. `quit` or Ctrl-D to exit.
omgdb> CREATE TABLE users (id INT, name TEXT, age INT);
OK
omgdb> INSERT INTO users VALUES (1, 'Alice', 30);
OK (rowid=1)
omgdb> INSERT INTO users VALUES (2, 'Bob', 25);
OK (rowid=2)
omgdb> INSERT INTO users VALUES (3, 'Carol', 42);
OK (rowid=3)
omgdb> SELECT name, age FROM users WHERE age >= 30 ORDER BY name;
name    | age
--------+----
'Alice' | 30
'Carol' | 42
(2 rows)
omgdb> quit
```

Reopen `people.db` later and the rows are still there. Files written by
the Rust runtime read fine on the native interpreter and AOT binaries,
and vice-versa.

## SQL surface

The supported subset:

```sql
CREATE TABLE <name> (<col> <type> [, ...]);
INSERT INTO <name> VALUES (<value> [, ...]);
SELECT <cols> FROM <name> [WHERE <cmp>] [ORDER BY <col> [ASC]];
DELETE FROM <name> WHERE <cmp>;
DROP TABLE <name>;
```

- **Types:** `INT`, `TEXT`. Column types are recorded as advisory
  metadata, the executor doesn't reject mismatched values.
- **`SELECT` columns:** comma-separated names, or `*` for all.
- **`WHERE`:** a single comparison `<col> <op> <value>` where the op is
  one of `=`, `!=`, `<>`, `<`, `<=`, `>`, `>=`. No `AND` / `OR` in v1.
- **`ORDER BY`:** single column, ascending only. `ASC` is accepted;
  `DESC` raises a parse error.
- **Strings:** single-quoted, with `\n` / `\t` / `\\` / `\'` escapes.
- **Comments:** none. Each statement ends at `;` outside a string.

## What's in this directory

| File          | Responsibility |
|---------------|----------------|
| `omgdb.omg`   | CLI driver: argv dispatch, REPL, statement-boundary scanner, result printing. |
| `sql.omg`     | SQL lexer + recursive-descent parser. Produces dict-shaped AST nodes consumed by `exec.omg`. |
| `exec.omg`    | Catalog + storage layer. `open_db` / `create_table` / `insert` / `select_filtered` / `delete_where` / `drop_table`, plus `run_statement` which dispatches a parsed AST node. |
| `wire.omg`    | Pure byte packing and paged I/O, `pack_u16` / `read_page` / `alloc_page` etc. Doesn't know about tables. |

The boundary between layers is sharp on purpose: `wire.omg` never
mentions SQL, `sql.omg` never touches a file handle. If you want to
swap the storage backend (e.g. for a B-tree) you only edit `exec.omg`.

## On-disk format (cheat sheet)

- Page size: 4 KB. Page 0 is the meta page (magic `"OMGD"`, format
  version, table catalog as a small dict).
- Each table is a singly-linked list of leaf pages. `INSERT` appends
  to the tail page and allocates a new one when the current one fills.
- Cell 0 of a table's first page holds its column-name list as a single
  TEXT cell separated by `\x01`. That keeps the catalog tiny and lets
  `DROP TABLE` later just free pages.
- All multi-byte integers are big-endian.
- The leaf-page layout already matches the Phase-2 B-tree format, so a
  future B-tree migration is an `exec.omg`-only change.

## Using the storage layer from OMG code

If you'd rather drive omgdb programmatically than via SQL, import
`exec.omg` directly:

```omg
;;;omg

import "exec.omg" as exec

alloc db := exec.open_db("people.db")
exec.create_table(db, "users", ["id", "name", "age"])
exec.insert(db, "users", [1, "Alice", 30])
exec.insert(db, "users", [2, "Bob", 25])
alloc rows := exec.select_all(db, "users")
emit "n=" + length(rows)
exec.close_db(db)
```

Import paths are resolved relative to the importing file's directory,
so a script next to `exec.omg` works without extra wiring.

## Tests

`tests/db.sh` exercises the storage layer (programmatic round-trips,
multi-page allocation, catalog metadata, error paths) and the SQL
surface (lexer/parser, `-e` and stdin pipelines, native + AOT parity
with the Rust runtime). Run it from the repo root after building the
native toolchain:

```sh
bash tests/db.sh
```

## Limitations (deliberate)

- No transactions, no rollback. A crash mid-INSERT leaves the file in
  whatever state the OS last flushed.
- No indexes, all `WHERE` and `ORDER BY` scan the table.
- No `JOIN`, `GROUP BY`, aggregate functions, `UPDATE`, or `NULL`.
- Column types are advisory. The executor doesn't reject mismatched
  values; `INSERT INTO t (n INT) VALUES ('hello')` will succeed.
- Single connection per file. Concurrent writers will corrupt the meta
  page.
