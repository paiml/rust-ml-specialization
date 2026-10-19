# Lean from Zero (for Rust ML): key terms and reflections

One block per module. Each module ends with its reflection prompt.


## Module 1 — Lean as a language you already half-know

### Key terms

- **lake** — Lean's build tool; it reads the project manifest and builds modules in dependency order, as cargo does for crates.
- **lean-toolchain** — The file that pins the exact Lean version a project builds with.
- **def** — Declares a named value or function; `#eval` runs it immediately.
- **inductive type** — A type defined by listing its constructors; the Lean counterpart of a Rust enum.

### Reflection

Pick one Rust enum you wrote. Write the inductive type that matches it and say which match arm you would need to cover each constructor.


## Module 2 — Types are propositions

### Key terms

- **proposition** — A statement that can be true or false, represented in Lean as a type.
- **proof term** — A value whose type is a proposition; its existence is the proof.
- **theorem** — A declaration that names a proof of a proposition.
- **kernel** — The small checker that accepts or refuses every term; it is the only thing you must trust.

### Reflection

Describe a bug that a Rust test missed because it only tried a few inputs. State the property as a Lean proposition and say what a proof would cover that the test did not.


## Module 3 — Proving by tactic

### Key terms

- **tactic** — A command that builds a proof term step by step by changing the goal.
- **rfl** — Closes a goal whose two sides compute to the same value.
- **induction** — Proves a property for all lists or numbers from a base case and a step case.
- **sorry** — Admits a goal without proof; every one is a visible, countable hole.

### Reflection

Take a small list function (reverse, length, map). Which tactic would you reach for first to prove it correct, and what is the base case?


## Module 4 — From a Rust contract to a Lean theorem

### Key terms

- **precondition** — What must hold on entry for the function to be correct.
- **postcondition** — What the function guarantees on exit.
- **invariant** — A property that holds on every pass through a loop.
- **falsifier** — A known-bad input or a planted hole that proves the checker can fail.

### Reflection

Choose one aprender contract. Write its precondition and postcondition, then say which assert would trip if the theorem were false.
