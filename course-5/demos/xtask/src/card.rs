//! Recording card: what the presenter needs in front of them to record a demo, derived
//! from `demo.toml` alone so the card cannot drift from the test.

use demo_kit::DemoManifest;
use std::fmt::Write;

pub fn render(m: &DemoManifest) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# Recording card — {}", m.id);
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "**{}** · lesson `{}` · host `{}`",
        m.title, m.lesson, m.host_class
    );
    let _ = writeln!(s);
    let _ = writeln!(s, "| pin | value |");
    let _ = writeln!(s, "|---|---|");
    let _ = writeln!(s, "| apr | `{}` |", m.apr);
    let _ = writeln!(s, "| agy | `{}` |", m.agy);
    if let Some(model) = &m.model {
        let _ = writeln!(s, "| model | `{}` sha256 `{}` |", model.name, model.sha256);
    }
    if let Some(fx) = &m.fixtures {
        let _ = writeln!(s, "| fixtures | `{}` tree `{}` |", fx.dir, fx.sha256);
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "## Before you press record");
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "- The demo is Green ×3 at this pin on `{}`.",
        m.host_class
    );
    let _ = writeln!(
        s,
        "- Export `RFML5_RECORDED=1` so this run writes the receipt that binds to the lesson."
    );
    let _ = writeln!(
        s,
        "- Screen {} · target ≤ {} s (PAIML ceiling 360 s).",
        m.record.resolution, m.record.target_duration_s
    );
    if let Some(t) = &m.record.terminal {
        let _ = writeln!(
            s,
            "- Terminal: {} cols, font {}, theme `{}`.",
            t.cols, t.font_pt, t.theme
        );
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "## Steps (type exactly these)");
    let _ = writeln!(s);
    for (i, st) in m.step.iter().enumerate() {
        let _ = writeln!(s, "{}. `{}`", i + 1, st.cmd);
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "## The run must assert");
    let _ = writeln!(s);
    for (k, v) in &m.assert {
        let _ = writeln!(s, "- `{k}` {v}");
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "## Narration may cite only these receipt fields");
    let _ = writeln!(s);
    if m.record.narration_may_cite.is_empty() {
        let _ = writeln!(s, "- (none — say no numbers)");
    }
    for f in &m.record.narration_may_cite {
        let _ = writeln!(s, "- `{f}`");
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "A Red or NotRun receipt means retake. A refusal appears on camera only labelled as a refusal.");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_carries_steps_and_citable_fields() {
        let m = DemoManifest::parse(
            "id = \"d13-reducer\"\ntitle = \"A deterministic reducer\"\nlesson = \"rfml5/2.2\"\napr = \"none\"\nhost_class = \"any\"\n[[step]]\ncmd = \"cargo run -p d13-reducer\"\n[assert]\nexit = 0\n[record]\ntarget_duration_s = 240\nresolution = \"1920x1080\"\nnarration_may_cite = [\"workers\", \"shuffles\"]\n",
        )
        .unwrap();
        let c = render(&m);
        assert!(c.contains("1. `cargo run -p d13-reducer`"));
        assert!(c.contains("- `shuffles`"));
        assert!(c.contains("≤ 240 s"));
        assert_eq!(c, render(&m), "card is deterministic");
    }
}
