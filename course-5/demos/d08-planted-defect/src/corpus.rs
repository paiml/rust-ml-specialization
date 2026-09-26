//! `fixtures/diffs/MANIFEST.toml` <-> files: the planted-defect-corpus-v1
//! falsifier lives here. `check_bijection` proves every manifest entry has
//! exactly one file and every file has exactly one entry; `check_markers`
//! proves every planted file's declared line is exactly its declared text
//! (never a comment calling out the bug — a reviewer has to find it).

use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestFile {
    pub path: String,
    pub planted: bool,
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub marker_line: Option<u32>,
    #[serde(default)]
    pub marker_text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    #[serde(rename = "file")]
    pub files: Vec<ManifestFile>,
}

impl Manifest {
    pub fn planted(&self) -> impl Iterator<Item = &ManifestFile> {
        self.files.iter().filter(|f| f.planted)
    }

    pub fn clean(&self) -> impl Iterator<Item = &ManifestFile> {
        self.files.iter().filter(|f| !f.planted)
    }

    /// Deterministic (lexical-by-path) ordering, split into clean and
    /// planted lists, so a subset choice never depends on directory
    /// iteration order.
    pub fn ordered(&self) -> (Vec<&ManifestFile>, Vec<&ManifestFile>) {
        let mut clean: Vec<&ManifestFile> = self.clean().collect();
        let mut planted: Vec<&ManifestFile> = self.planted().collect();
        clean.sort_by(|a, b| a.path.cmp(&b.path));
        planted.sort_by(|a, b| a.path.cmp(&b.path));
        (clean, planted)
    }
}

pub fn load(fixtures_dir: &Path) -> Manifest {
    let text = std::fs::read_to_string(fixtures_dir.join("MANIFEST.toml"))
        .unwrap_or_else(|e| panic!("{}: {e}", fixtures_dir.join("MANIFEST.toml").display()));
    toml::from_str(&text).unwrap_or_else(|e| panic!("MANIFEST.toml: {e}"))
}

/// planted-defect-corpus-v1 (bijection half): every manifest entry names
/// exactly one file in `fixtures_dir`, and every file in `fixtures_dir`
/// (other than MANIFEST.toml itself) is named by exactly one entry.
pub fn check_bijection(fixtures_dir: &Path, m: &Manifest) -> Result<(), Vec<String>> {
    let mut problems = Vec::new();
    let mut on_disk: BTreeSet<String> = std::fs::read_dir(fixtures_dir)
        .unwrap_or_else(|e| panic!("{}: {e}", fixtures_dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n != "MANIFEST.toml")
        .collect();

    let mut declared: BTreeSet<String> = BTreeSet::new();
    for f in &m.files {
        if !declared.insert(f.path.clone()) {
            problems.push(format!(
                "{}: declared more than once in MANIFEST.toml",
                f.path
            ));
        }
        if !on_disk.remove(&f.path) {
            problems.push(format!(
                "{}: declared in MANIFEST.toml but not found on disk",
                f.path
            ));
        }
    }
    for leftover in on_disk {
        problems.push(format!(
            "{leftover}: on disk but not declared in MANIFEST.toml"
        ));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

/// planted-defect-corpus-v1 (marker half): every planted file has a class
/// and a marker, and `lines(file)[marker_line - 1] == marker_text` exactly.
pub fn check_markers(fixtures_dir: &Path, m: &Manifest) -> Result<(), Vec<String>> {
    let mut problems = Vec::new();
    for f in m.planted() {
        let class_ok = f.class.as_deref().is_some_and(|c| !c.is_empty());
        if !class_ok {
            problems.push(format!("{}: planted but has no defect class", f.path));
        }
        let (Some(line_no), Some(text)) = (f.marker_line, f.marker_text.as_deref()) else {
            problems.push(format!(
                "{}: planted but has no marker_line/marker_text",
                f.path
            ));
            continue;
        };
        if line_no == 0 || text.is_empty() {
            problems.push(format!("{}: marker_line/marker_text is empty", f.path));
            continue;
        }
        let full_path = fixtures_dir.join(&f.path);
        let contents = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(e) => {
                problems.push(format!("{}: {e}", full_path.display()));
                continue;
            }
        };
        let got = contents.lines().nth(line_no as usize - 1);
        if got != Some(text) {
            problems.push(format!(
                "{}: line {line_no} is {got:?}, MANIFEST.toml says {text:?}",
                f.path
            ));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/diffs")
    }

    #[test]
    fn corpus_is_a_bijection() {
        let m = load(&fixtures_dir());
        assert_eq!(m.files.len(), 20, "10 clean + 10 planted");
        assert_eq!(check_bijection(&fixtures_dir(), &m), Ok(()));
    }

    #[test]
    fn corpus_ten_and_ten() {
        let m = load(&fixtures_dir());
        assert_eq!(m.clean().count(), 10);
        assert_eq!(m.planted().count(), 10);
    }

    #[test]
    fn every_planted_marker_is_present_verbatim() {
        let m = load(&fixtures_dir());
        assert_eq!(check_markers(&fixtures_dir(), &m), Ok(()));
    }

    #[test]
    fn every_planted_defect_class_is_distinct() {
        let m = load(&fixtures_dir());
        let classes: BTreeSet<&str> = m.planted().filter_map(|f| f.class.as_deref()).collect();
        assert_eq!(classes.len(), 10, "10 distinct defect classes: {classes:?}");
    }

    /// A marker is never revealed as a comment in the diff: a reviewer has
    /// to find the bug from the code, not from a label calling it out.
    #[test]
    fn marker_text_is_never_a_comment_line() {
        let m = load(&fixtures_dir());
        for f in m.planted() {
            let text = f.marker_text.as_deref().unwrap_or("");
            let code = text.trim_start_matches(['+', '-', ' ']).trim_start();
            assert!(
                !code.starts_with("//"),
                "{}: marker_text {text:?} looks like a comment",
                f.path
            );
        }
    }

    /// planted-defect-corpus-v1 falsifier: deleting a planted file's marker
    /// line, in a temp copy, turns `check_markers` RED.
    #[test]
    fn removing_a_marker_line_turns_the_check_red() {
        let src = fixtures_dir();
        let m = load(&src);
        assert_eq!(
            check_markers(&src, &m),
            Ok(()),
            "the real corpus starts Green"
        );

        let victim = m
            .planted()
            .next()
            .expect("at least one planted file")
            .clone();
        let tmp = std::env::temp_dir().join(format!(
            "d08-corpus-mutation-{}-{}",
            std::process::id(),
            victim.path.replace('/', "_")
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        for entry in std::fs::read_dir(&src).unwrap() {
            let entry = entry.unwrap();
            let dest = tmp.join(entry.file_name());
            std::fs::copy(entry.path(), dest).unwrap();
        }

        let victim_path = tmp.join(&victim.path);
        let marker_line = victim.marker_line.unwrap();
        let original = std::fs::read_to_string(&victim_path).unwrap();
        let mutated: String = original
            .lines()
            .enumerate()
            .filter(|(i, _)| *i as u32 != marker_line - 1)
            .map(|(_, l)| format!("{l}\n"))
            .collect();
        assert_ne!(
            original, mutated,
            "the mutation must actually remove a line"
        );
        std::fs::write(&victim_path, mutated).unwrap();

        let mutated_manifest = load(&tmp);
        let result = check_markers(&tmp, &mutated_manifest);
        assert!(
            result.is_err(),
            "removing {}'s marker line must turn check_markers RED",
            victim.path
        );

        std::fs::remove_dir_all(&tmp).unwrap();
    }

    /// The tree hash pinned in d07's and d08's own `demo.toml` must be the
    /// real corpus's hash — a drifted pin is exactly what preflight's
    /// `FixturesMismatch` exists to catch, but this fails fast in `cargo test`.
    #[test]
    fn fixtures_pin_matches_the_real_corpus() {
        let hash = demo_kit::sha::tree_hash(&fixtures_dir()).unwrap();
        for demo in ["d07-quorum", "d08-planted-defect"] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(demo)
                .join("demo.toml");
            let dm = demo_kit::DemoManifest::load(&path).unwrap();
            let fx = dm
                .fixtures
                .unwrap_or_else(|| panic!("{demo}: demo.toml has no [fixtures]"));
            assert_eq!(fx.sha256, hash, "{demo}: fixtures pin is stale");
        }
    }
}
