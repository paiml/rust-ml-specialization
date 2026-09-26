//! Digests: a file's sha256, and a directory tree's hash (sorted relative
//! paths, each line `path\0sha256\n`, hashed again), so renaming, adding or
//! editing any fixture changes the tree hash.

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_bytes(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

pub fn tree_hash(root: &Path) -> io::Result<String> {
    let mut files = Vec::new();
    walk(root, root, &mut files)?;
    files.sort();
    let mut h = Sha256::new();
    for rel in files {
        let digest = sha256_file(&root.join(&rel))?;
        h.update(rel.to_string_lossy().as_bytes());
        h.update([0]);
        h.update(digest.as_bytes());
        h.update(b"\n");
    }
    Ok(hex(&h.finalize()))
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(root, &path, out)?;
        } else {
            out.push(path.strip_prefix(root).expect("under root").to_path_buf());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vector() {
        assert_eq!(
            sha256_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn tree_hash_moves_with_any_change() {
        let d = std::env::temp_dir().join(format!("dk-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::write(d.join("a.diff"), "one").unwrap();
        std::fs::write(d.join("sub/b.diff"), "two").unwrap();
        let h1 = tree_hash(&d).unwrap();
        assert_eq!(h1, tree_hash(&d).unwrap(), "tree hash is deterministic");
        std::fs::write(d.join("sub/b.diff"), "TWO").unwrap();
        let h2 = tree_hash(&d).unwrap();
        assert_ne!(h1, h2, "content change must move the hash");
        std::fs::rename(d.join("a.diff"), d.join("c.diff")).unwrap();
        assert_ne!(h2, tree_hash(&d).unwrap(), "rename must move the hash");
        std::fs::remove_dir_all(&d).unwrap();
    }
}
