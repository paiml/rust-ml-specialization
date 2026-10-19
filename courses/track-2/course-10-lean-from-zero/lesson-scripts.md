# Lean from Zero (for Rust ML): lesson scripts

Draft. Each lesson is one video of at most 6 minutes. A beat animates the mechanism; the Animation line is the picture the narration must show. Contract: every lesson ends with a runnable `lake build` check (Lean) or a Playground-verified Rust snippet before it is recorded.

## Module 1 — Lean as a language you already half-know

### 1.1 Lean 4 in one toolchain

- **Narration:** Lean ships as a toolchain pinned by a lean-toolchain file; lake builds it, the way cargo builds a crate.
- **Animation:** Two terminals side by side: cargo build and lake build each walk the same dependency graph, nodes lighting green in order.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).

### 1.2 def and #eval: a function you can run

- **Narration:** A Lean def is a Rust fn. #eval runs it at once, so the editor shows the answer beside the code.
- **Animation:** A Rust fn add and a Lean def add are connected by lines; a value 2,3 flows through both and 5 lands on the infoview.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).

### 1.3 inductive types are enums

- **Narration:** An inductive type lists its constructors, exactly like enum variants, and a match covers each one.
- **Animation:** An enum Shape and an inductive Shape split into variants; a value drops into a match and routes to the right arm.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).

### 1.4 structure is struct

- **Narration:** A structure bundles named fields; the projection x.field reads one, as in Rust.
- **Animation:** A struct Point and a structure Point; a field access arrow extracts x from the value in both.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).

## Module 2 — Types are propositions

### 2.1 A proposition is a type

- **Narration:** In Lean, 2 + 2 = 4 is itself a type. A proof is a value of that type.
- **Animation:** A box labelled 2+2=4 is empty; a term slides in and the box turns solid. A box 2+2=5 stays empty whatever slides in.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).

### 2.2 theorem versus def

- **Narration:** A def computes a value; a theorem names a proof. Both are checked by the same kernel.
- **Animation:** Two pipelines into one kernel gate; the def emits a number, the theorem emits a stamp, both pass the gate.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).

### 2.3 implication and conjunction

- **Narration:** P implies Q is a function from proofs of P to proofs of Q; P and Q is a pair of proofs.
- **Animation:** A function machine takes a P-proof and outputs a Q-proof; a pair constructor holds two proofs side by side.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).

### 2.4 the kernel says no

- **Narration:** The kernel checks every term. A wrong proof is refused with the exact mismatch, not accepted on trust.
- **Animation:** A term enters the type checker, expected and actual types collide, the term is stamped REFUSED in red with the diff.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).

## Module 3 — Proving by tactic

### 3.1 intro and exact

- **Narration:** intro takes an assumption into context; exact closes the goal with a term of the goal's type.
- **Animation:** A goal panel shrinks: an arrow goal splits into context hypothesis plus smaller goal, then exact fills it and the panel empties.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).

### 3.2 rfl and simp

- **Narration:** rfl closes goals true by computation; simp rewrites with known lemmas until the goal closes.
- **Animation:** An expression tree is rewritten step by step, subtrees collapsing until both sides are identical.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).

### 3.3 omega for arithmetic

- **Narration:** omega decides linear integer arithmetic goals, which covers most bounds on indices and lengths.
- **Animation:** A number line with inequalities as shaded regions; the solver shows the regions have no counterexample point.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).

### 3.4 induction on lists

- **Narration:** To prove a list property, prove it for nil and prove that if it holds for xs it holds for x :: xs.
- **Animation:** A list grows one cell at a time; a proof obligation is carried along from nil to each longer list like a domino chain.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).

### 3.5 sorry is a visible hole

- **Narration:** sorry admits a goal without proof. The build warns and a file-level sorry count makes every hole countable.
- **Animation:** A file view where each sorry is a red marker; a counter in the corner goes 3, 2, 1, 0 as holes are filled.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).

## Module 4 — From a Rust contract to a Lean theorem

### 4.1 a contract in Rust

- **Narration:** A provable contract is a precondition, a postcondition and an invariant, enforced by assert at runtime.
- **Animation:** A Rust fn is wrapped by three gates; a value passing in is checked at entry, at exit and across the loop.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).

### 4.2 the same contract as a Lean theorem

- **Narration:** State the contract as a theorem over all inputs, not the few a test tries.
- **Animation:** The three gates from 4.1 become a single quantifier sweep over the whole input line instead of sample dots.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).

### 4.3 discharge it and bind it back

- **Narration:** Prove the theorem, then bind it to the Rust assert so a failure names the contract it breaks.
- **Animation:** A proven theorem stamp is linked by a wire to the Rust assert line; a falsifier input trips the assert and the wire lights.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).

### 4.4 the ground-truth falsifier

- **Narration:** The ground-truth corpus keeps exactly three intentional sorry markers so a checker that reports zero is known broken.
- **Animation:** A checker scans the corpus and must report three; one run reports zero and is flagged as the broken checker.
- **Check:** a named contract asserted at the end of the demo; `lake build` green, zero `sorry` (except 3.5 and 4.4 which count them on purpose).
