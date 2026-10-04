//! The Rust asserts beyond the shapes (spec §5.1). pv sees structure and
//! relations between two named properties of one node; it cannot see a
//! digest's truth, a pid's origin, a timestamp's clock or whether a schedule
//! really overlapped. Each check here owns one of D19's survive rows, so every
//! defect the shapes cannot see has a named owner. Each returns `Err(why)`;
//! none panics, so a failure is a Red verdict with its reason.

use crate::ident::Ident;
use crate::{CHECKER_FILE, WRITER_FILE};
use std::path::Path;

/// One item's four timestamps in one schedule, in ms since that schedule's
/// start, all read from the orchestrator's one monotonic clock at the sink.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Times {
    pub writer_start: u64,
    pub writer_end: u64,
    pub checker_start: u64,
    pub checker_end: u64,
}

impl Times {
    fn intervals(&self) -> [(u64, u64); 2] {
        [
            (self.writer_start, self.writer_end),
            (self.checker_start, self.checker_end),
        ]
    }
}

/// Two half-open request intervals are in flight at once.
fn overlap(a: (u64, u64), b: (u64, u64)) -> bool {
    a.0 < b.1 && b.0 < a.1
}

/// `one_load` (s02): one spawn, and the process seen at the end is the one
/// seen at spawn (same pid, same start ticks), running the pinned binary.
pub fn one_load(spawns: u32, start: &Ident, end: &Ident, pinned: &Path) -> Result<(), String> {
    if spawns != 1 {
        return Err(format!("one_load: {spawns} server spawns, not 1"));
    }
    if (start.pid, start.start_ticks) != (end.pid, end.start_ticks) {
        return Err(format!(
            "one_load: pid {} ticks {} at spawn, pid {} ticks {} at the end",
            start.pid, start.start_ticks, end.pid, end.start_ticks
        ));
    }
    let canon = |p: &Path| std::fs::canonicalize(p).ok();
    let want = canon(pinned).ok_or_else(|| format!("one_load: pinned binary {pinned:?} absent"))?;
    for id in [start, end] {
        if canon(&id.exe).as_ref() != Some(&want) {
            return Err(format!(
                "one_load: /proc/{}/exe is {:?}, not the pinned binary",
                id.pid, id.exe
            ));
        }
    }
    Ok(())
}

/// `disjoint_writes` (s05, FALSIFY-D18-005): the registry says the writer
/// opened exactly `out/answers.json` and the checker exactly `out/verdicts.json`.
pub fn disjoint_writes(writer: &[String], checker: &[String]) -> Result<(), String> {
    if writer != [WRITER_FILE] {
        return Err(format!("disjoint_writes: writer opened {writer:?}"));
    }
    if checker != [CHECKER_FILE] {
        return Err(format!("disjoint_writes: checker opened {checker:?}"));
    }
    Ok(())
}

/// sha256 over `out/answers.json ‖ out/verdicts.json`, read from disk now.
pub fn digest_from_disk(root: &Path) -> Result<String, String> {
    let mut bytes = Vec::new();
    for rel in [WRITER_FILE, CHECKER_FILE] {
        let b = std::fs::read(root.join(rel)).map_err(|e| format!("{rel}: {e}"))?;
        bytes.extend_from_slice(&b);
    }
    Ok(demo_kit::sha::sha256_bytes(&bytes))
}

/// Digests are recomputed (s01): the recorded digest equals the bytes on disk.
pub fn digest_recomputed(root: &Path, recorded: &str) -> Result<(), String> {
    let now = digest_from_disk(root)?;
    if now != recorded {
        return Err(format!(
            "digest: recorded {recorded}, the files on disk hash to {now}"
        ));
    }
    Ok(())
}

/// Each item's decision, parsed from the checker's file, in item order.
pub fn decisions_from_disk(root: &Path) -> Result<[String; 4], String> {
    let text = std::fs::read_to_string(root.join(CHECKER_FILE))
        .map_err(|e| format!("{CHECKER_FILE}: {e}"))?;
    let mut out: [String; 4] = Default::default();
    for (slot, id) in out.iter_mut().zip(crate::item_ids()) {
        *slot = crate::agents::field_for(&text, id, "decision")
            .ok_or_else(|| format!("{CHECKER_FILE}: no decision for {id}"))?;
        if !matches!(slot.as_str(), "accept" | "reject") {
            return Err(format!("{CHECKER_FILE}: {id} decided {slot:?}"));
        }
    }
    Ok(out)
}

/// Decisions come from the checker's file (s03).
pub fn decisions_from_checker(root: &Path, recorded: &[String; 4]) -> Result<(), String> {
    let read = decisions_from_disk(root)?;
    if &read != recorded {
        return Err(format!(
            "decisions: recorded {recorded:?}, {CHECKER_FILE} says {read:?}"
        ));
    }
    Ok(())
}

/// One clock (s04): within a schedule each item's chain is ordered, each role
/// handles items in increasing i (item i+1 starts no earlier than item i
/// ended), and no stamp is past that schedule's elapsed time.
pub fn one_clock(items: &[Times; 4], elapsed_ms: u64) -> Result<(), String> {
    for (i, t) in items.iter().enumerate() {
        let chain = [t.writer_start, t.writer_end, t.checker_start, t.checker_end];
        if chain.windows(2).any(|w| w[0] > w[1]) {
            return Err(format!("one_clock: q{} is out of order: {t:?}", i + 1));
        }
        if t.checker_end > elapsed_ms {
            return Err(format!(
                "one_clock: q{} ends at {} ms, after the schedule's {elapsed_ms} ms",
                i + 1,
                t.checker_end
            ));
        }
    }
    for (i, w) in items.windows(2).enumerate() {
        if w[1].writer_start < w[0].writer_end || w[1].checker_start < w[0].checker_end {
            return Err(format!(
                "one_clock: q{} starts before q{} ended",
                i + 2,
                i + 1
            ));
        }
    }
    Ok(())
}

/// `pipelined_overlaps`, the one statement of it (spec §5.1; §2 M7 s06 refers
/// here): for at least one i ∈ {1, 2, 3}, the checker's request on q(i) and
/// the writer's request on q(i+1) are both in flight at once —
/// `checker_start(q i) < writer_end(q i+1)` and
/// `writer_start(q i+1) < checker_end(q i)`, on the same clock. In the
/// sequential schedule no two requests overlap.
pub fn pipelined_overlaps(seq: &[Times; 4], pipe: &[Times; 4]) -> Result<(), String> {
    let all: Vec<(u64, u64)> = seq.iter().flat_map(Times::intervals).collect();
    for (a, x) in all.iter().enumerate() {
        if let Some(y) = all[a + 1..].iter().find(|y| overlap(*x, **y)) {
            return Err(format!(
                "pipelined_overlaps: the sequential schedule overlapped, {x:?} and {y:?}"
            ));
        }
    }
    let overlapped = pipe
        .windows(2)
        .any(|w| w[0].checker_start < w[1].writer_end && w[1].writer_start < w[0].checker_end);
    if !overlapped {
        return Err(
            "pipelined_overlaps: the pipelined schedule never had a checker and the next writer in flight at once"
                .into(),
        );
    }
    Ok(())
}
