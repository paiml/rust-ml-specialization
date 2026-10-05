# Graded assignment: a fail-closed parity sweep

Implement the five functions marked `todo!()` in `src/lib.rs`. The doc comment
on each one is its specification, and each has a named **Provable contract**.

```bash
cargo test --offline            # visible tests: a quick check
cargo run --offline -- 1        # check part 1's contract (or 2..5, or `all`)
```

A part scores when, and only when:

1. the grader's hidden tests for that part all pass,
2. `cargo run -- <part>` exits 0, and
3. it prints exactly `contract: <name> OK`.

Only `src/lib.rs` is graded. The grader uses its own `main.rs`, so editing
`main.rs` changes nothing. Read the rules at the top of `lib.rs`: a submission
that prints, exits, or touches the environment, files, processes or network is
refused before it is built.

The hidden tests check exact answers, not only the properties `main.rs`
checks. A function that returns a constant can satisfy `main.rs` and still
score zero.
