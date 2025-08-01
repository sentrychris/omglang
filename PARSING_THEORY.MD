
### Notes

#### **1. Factor**

The most atomic element of an expression e.g., literals, identifiers, or parenthesized expressions.

**Level**: lowest-precedence unit in arithmetic grammar.

```python
def factor(self):
```

Handles:

* **Literals**: numbers, strings
* **Variables**: identifiers (`thingy`)
* **Function calls**: if followed by `(`
* **Parenthesized expressions**: grouped expressions `( ... )`

It does **not** handle any operators directly.

---

#### **2. Term**

A composition of factors joined by **multiplicative operators** like `*`, `/`, `%`.

**Level**: middle-precedence unit in arithmetic grammar.

```python
def term(self):
```


Builds:

* `factor` `*` `factor`
* `factor` `/` `factor`
* `factor` `%` `factor`

This method is recursive and left-associative.

---

#### **3. Expression (expr)**

A composition of terms joined by **additive operators** like `+` and `-`.

**Level**: top-level arithmetic unit.

```python
def expr(self):
```

Builds:

* `term` `+` `term`
* `term` `-` `term`

Also left-associative.

---

#### **4. Comparison**

An expression that evaluates a relational condition e.g., using `==`, `<`, `>=`, etc.

**Level**: boolean expressions.

```python
def comparison(self):
```

Handles:

* `expr == expr`
* `expr > expr`, etc.

Used in control flow (`maybe`, `hamsterwheel`) for condition evaluation.

---

#### **5. Statement**

A complete instruction in the program.

**Level**: top-level constructs.

```python
def statement(self):
```

Handles:

* Variable declarations (`thingy`)
* Output (`saywhat`)
* Control flow (`maybe`, `hamsterwheel`)
* Function definitions
* Function calls

Each statement evaluates for side effects or flow control.

---

#### **6. Block**

A sequence of statements grouped together, typically enclosed in braces `{}`.

```python
def block(self):
```

Parses multiple statements and returns them as a grouped structure. Used inside control flow or functions.

---

#### Precedence (from lowest to highest):

| Level       | Construct      | Purpose                             |
| ----------- | -------------- | ----------------------------------- |
| 6 (lowest)  | **block**      | Group multiple statements `{ ... }` |
| 5           | **statement**  | Performs an action or defines logic |
| 4           | **comparison** | Evaluates a condition               |
| 3           | **expression** | Handles `+`, `-`                    |
| 2           | **term**       | Handles `*`, `/`, `%`               |
| 1 (highest) | **factor**     | Literals, variables, function calls |
