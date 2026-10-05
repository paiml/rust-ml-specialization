# Course 5 lab

Two Rust workspaces, standard library only, that run offline in the course's
lab environment or on your own machine.

- [`practice/`](practice/README.md): ungraded exercises ported from the
  demos (two agents and one model server, a workflow judged by named claims,
  fan-in and stop-the-line).
- [`assignment/`](assignment/README.md): the graded end-of-course assignment,
  a fail-closed parity sweep. You submit `src/lib.rs`.

Every program checks its Provable contracts with `assert!` before it prints
`contract: <name> OK`.
