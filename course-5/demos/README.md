# Course 5 demos — Agentic Patterns: Parallel Subagent ML with Rust

Every demo here is a test that someone can also record: a Rust program with
pinned inputs, machine-checked assertions, and a receipt.

| crate | what it is |
|---|---|
| `demo-kit` | the harness: exact pins, weights/fixture digests, preflight, a resident-server guard that kills its whole process group on drop, verdicts, receipts |
| `xtask` | `cargo run -p xtask -- verify` (pins + provable-contract lint for every demo) and `cargo run -p xtask -- card <demo>` (recording card) |

## Rules every demo follows

- `demo.toml` is the single source for the test, the receipt and the recording card.
- Pins are exact (`=0.69.3`). A floor such as `>=0.69.3` is refused.
- A demo is **Green** only when every assertion held and no preflight reason fired.
  A refused verb makes it `NotRun{Refused(<verb>)}`, never Green.
- `src/main.rs` opens with a `//!` docstring naming its `Provable contract:`,
  ends `main` with an assertion, and prints `contract: <name> OK`.
- Receipts are written to `$RFML5_RECEIPTS`, which must be outside this repository.

## Run

```bash
cargo test --workspace
cargo run -p xtask -- verify
```
