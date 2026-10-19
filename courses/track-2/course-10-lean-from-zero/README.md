# Course 10 — Lean from Zero (for Rust ML)

> **Track 2** · Anchored in: Proof — the bridge to provable contracts

## Status

Outline and lesson scripts drafted (4 modules, 17 lessons, capstone; see `lesson-scripts.md`, `quiz.jsonl`, `key-terms-reflections.md`). Recordings are not yet made;
every lesson below needs a rich animation of the
mechanism it names (rmedia rule: text alone is never a beat).

## Structure

```text
Course → Module → Lesson (3–5 videos ≤ 6 min each) → Key Terms + Reflection
```

Each module ends with a **Critical Thinking Assessment** (quiz + role-play).
Course ends with one **Graded Quiz** (6 questions, 80% pass) and a Capstone.

## Modules

* **Module 1 — Lean as a language you already half-know.** Lean 4 toolchain
  (`lake`, `lean-toolchain`); definitions as Rust `fn` and `#eval` to run them;
  inductive types as Rust `enum`; structures as `struct`.
* **Module 2 — Types are propositions.** Propositions as types, proofs as
  terms; implication and conjunction; `theorem` vs `def`; the kernel refusing a
  wrong proof (animate the type checker rejecting a term).
* **Module 3 — Proving by tactic.** `intro`, `exact`, `rfl`, `simp`, `omega`,
  `induction`; proving a list function correct; why `sorry` is a visible hole
  (animate the sorry count of a file rising and falling).
* **Module 4 — From a Rust contract to a Lean theorem.** A provable contract
  (precondition, postcondition, invariant) written in Rust, restated as a Lean
  theorem, discharged, and bound back to the Rust assert; ground truth from the
  `lean-ground-truth` corpus (3 intentional `sorry` markers as the falsifier).

## Capstone

Pick one aprender kernel contract (for example a bounded-output or
monotonicity property), state it in Lean 4, prove it with no `sorry`, and bind
it to the Rust `assert!` that enforces it at runtime. Deliverable: a
LinkedIn-shareable repo whose `lake build` is green and whose sorry count is 0.

## Anchored In

Proof — the bridge to provable contracts

## Sibling Repos

* [paiml/aprender](https://github.com/paiml/aprender) — Pure-Rust ML framework
* See top-level [README.md](../../../README.md#foundation-repositories) for the full list
