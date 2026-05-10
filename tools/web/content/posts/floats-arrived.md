---
title: Floats have arrived
date: 2026-05-08
---

OMG used to be integers-only. Today it isn't:

```omg
;;;omg

emit 10 / 3         # 3       int / int stays floor division
emit 10 / 3.0       # 3.333…  any float promotes
emit 10 // 3        # 3       explicit floor division
emit sqrt(2.0)      # 1.4142…
```

`/` is **promote-on-float**: between two ints it stays as the old
floor-division behaviour, but as soon as either operand is a float you
get true division. Use `//` when you specifically want to floor.

The math kit ships with `floor`, `ceil`, `round` (banker's rounding),
`abs`, `sqrt`, `pow`, `log`, and the trig trio `sin` / `cos` / `tan`.

This very paragraph was rendered by an SSG written in OMG, on a runtime
written in Rust, with the OMG-in-OMG compiler verifying its own bytecode
byte-for-byte. The whole stack is real.
