# Lean from Zero (for Rust ML): key terms and reflections

One block per module. Each module ends with its reflection prompt.

## Module 1 — Lean as a language you already half-know

### Module 1 key terms

- **lake** — Lean's build tool; it reads the project manifest and builds modules in dependency order, as cargo does for crates.
- **lean-toolchain** — The file that pins the exact Lean version a project builds with.
- **def** — Declares a named value or function; `#eval` runs it immediately.
- **inductive type** — A type defined by listing its constructors; the Lean counterpart of a Rust enum.
- **structure** — Bundles named fields into one value; `p.x` reads a field, as in a Rust struct.
- **#eval** — Runs an expression immediately and shows the result beside the code.

### Module 1 reflection

Pick one Rust enum you wrote. Write the inductive type that matches it and say which match arm you would need to cover each constructor.

## Module 2 — Types are propositions

### Module 2 key terms

- **proposition** — A statement that can be true or false, represented in Lean as a type.
- **proof term** — A value whose type is a proposition; its existence is the proof.
- **theorem** — A declaration that names a proof of a proposition.
- **implication and conjunction** — P → Q is a function from proofs of P to proofs of Q; P ∧ Q is a pair of proofs.
- **kernel** — The small checker that accepts or refuses every term; it is the only thing you must trust.

### Module 2 reflection

Describe a bug that a Rust test missed because it only tried a few inputs. State the property as a Lean proposition and say what a proof would cover that the test did not.

## Module 3 — Proving by tactic

### Module 3 key terms

- **tactic** — A command that builds a proof term step by step by changing the goal.
- **intro and exact** — `intro` moves an assumption into context; `exact` closes the goal with a term of its type.
- **simp and omega** — `simp` rewrites with known lemmas and fails if it makes no progress; `omega` decides linear integer arithmetic.
- **rfl** — Closes a goal whose two sides compute to the same value.
- **induction** — Proves a property for all lists or numbers from a base case and a step case.
- **sorry** — Admits a goal without proof; every one is a visible, countable hole.

### Module 3 reflection

Take a small list function (reverse, length, map). Which tactic would you reach for first to prove it correct, and what is the base case?

## Module 4 — From a Rust contract to a Lean theorem

### Module 4 key terms

- **precondition** — What must hold on entry for the function to be correct.
- **postcondition** — What the function guarantees on exit.
- **invariant** — A property that holds on every pass through a loop.
- **falsifier** — A known-bad input or a planted hole that proves the checker can fail.

### Module 4 reflection

Choose one aprender contract. Write its precondition, postcondition and loop invariant, then say which assert would trip if the theorem were false.
