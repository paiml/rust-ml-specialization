//! The RFC 6902 subset the mutant table uses: `add` (including an append with
//! a final `-`), `remove` and `replace`, over `serde_json` pointers.
//!
//! A patch that errors, or that leaves the record unchanged, is refused: a
//! mutant equal to the golden record would pass for a survivor while testing
//! nothing (spec §5.2, "every patch is non-vacuous").

use serde_json::Value;

/// Split an RFC 6901 pointer into its unescaped segments.
fn pointer(path: &str) -> Result<Vec<String>, String> {
    let rest = path
        .strip_prefix('/')
        .ok_or_else(|| format!("bad pointer {path:?}"))?;
    Ok(rest
        .split('/')
        .map(|s| s.replace("~1", "/").replace("~0", "~"))
        .collect())
}

fn index(seg: &str) -> Result<usize, String> {
    seg.parse().map_err(|_| format!("index {seg:?}"))
}

fn step<'a>(v: &'a mut Value, seg: &str) -> Result<&'a mut Value, String> {
    match v {
        Value::Array(a) => {
            let i = index(seg)?;
            a.get_mut(i)
                .ok_or_else(|| format!("index {i} out of range"))
        }
        Value::Object(o) => o.get_mut(seg).ok_or_else(|| format!("no key {seg:?}")),
        _ => Err(format!("cannot step into a scalar at {seg:?}")),
    }
}

/// The container that holds the last segment, and that segment.
fn parent<'a>(doc: &'a mut Value, p: &'a [String]) -> Result<(&'a mut Value, &'a str), String> {
    let (last, init) = p.split_last().ok_or("the root cannot be patched")?;
    let mut cur = doc;
    for seg in init {
        cur = step(cur, seg)?;
    }
    Ok((cur, last))
}

fn add(doc: &mut Value, p: &[String], value: Value) -> Result<(), String> {
    let (par, last) = parent(doc, p)?;
    match par {
        Value::Array(a) if last == "-" => a.push(value),
        Value::Array(a) => {
            let i = index(last)?;
            if i > a.len() {
                return Err(format!("index {i} out of range"));
            }
            a.insert(i, value);
        }
        Value::Object(o) => {
            o.insert(last.to_string(), value);
        }
        _ => return Err("cannot add into a scalar".into()),
    }
    Ok(())
}

fn replace(doc: &mut Value, p: &[String], value: Value) -> Result<(), String> {
    let (par, last) = parent(doc, p)?;
    *step(par, last)? = value;
    Ok(())
}

fn remove(doc: &mut Value, p: &[String]) -> Result<(), String> {
    let (par, last) = parent(doc, p)?;
    match par {
        Value::Array(a) => {
            let i = index(last)?;
            if i >= a.len() {
                return Err(format!("index {i} out of range"));
            }
            a.remove(i);
        }
        Value::Object(o) => {
            o.remove(last).ok_or_else(|| format!("no key {last:?}"))?;
        }
        _ => return Err("cannot remove from a scalar".into()),
    }
    Ok(())
}

/// Apply one operation object to `doc`.
pub fn apply_op(doc: &mut Value, op: &Value) -> Result<(), String> {
    let kind = op
        .get("op")
        .and_then(Value::as_str)
        .ok_or("op has no .op")?;
    let p = pointer(
        op.get("path")
            .and_then(Value::as_str)
            .ok_or("op has no .path")?,
    )?;
    let value = || op.get("value").cloned().ok_or("op has no .value");
    match kind {
        "add" => add(doc, &p, value()?),
        "replace" => replace(doc, &p, value()?),
        "remove" => remove(doc, &p),
        other => Err(format!("unsupported op {other:?}")),
    }
}

/// Apply every operation in `ops` to a copy of `golden`. An empty `ops` is
/// the identity arm and returns the golden record unchanged; any other patch
/// that errors or changes nothing is refused.
pub fn patched(golden: &Value, ops: &Value) -> Result<Value, String> {
    let ops = ops.as_array().ok_or("a patch is an array of operations")?;
    let mut doc = golden.clone();
    for op in ops {
        apply_op(&mut doc, op)?;
    }
    if !ops.is_empty() && doc == *golden {
        return Err("vacuous: the patch leaves the golden record unchanged".into());
    }
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn add_append_insert_remove_replace() {
        let g = json!({"a": [1, 2], "b": {"c": 1}});
        let ops = json!([
            {"op": "add", "path": "/a/-", "value": 3},
            {"op": "add", "path": "/a/0", "value": 0},
            {"op": "replace", "path": "/a/1", "value": 9},
            {"op": "remove", "path": "/b/c"},
            {"op": "add", "path": "/d", "value": "x"}
        ]);
        assert_eq!(
            patched(&g, &ops).unwrap(),
            json!({"a": [0, 9, 2, 3], "b": {}, "d": "x"})
        );
    }

    #[test]
    fn the_empty_patch_is_the_identity() {
        let g = json!({"a": 1});
        assert_eq!(patched(&g, &json!([])).unwrap(), g);
    }

    /// Spec §5.2: every patch is non-vacuous, and a patch that cannot apply is
    /// refused rather than skipped.
    #[test]
    fn vacuous_or_failing_patches_are_refused() {
        let g = json!({"a": 1, "l": [1]});
        let refused = [
            json!([{"op": "replace", "path": "/a", "value": 1}]),
            json!([{"op": "move", "from": "/a", "path": "/b"}]),
            json!([{"op": "replace", "path": "/x", "value": 1}]),
            json!([{"op": "replace", "path": "/x/y", "value": 1}]),
            json!([{"op": "add", "path": "/a/-", "value": 1}]),
            json!([{"op": "remove", "path": "/zz"}]),
            json!([{"op": "remove", "path": "/l/3"}]),
            json!([{"op": "add", "path": "", "value": 1}]),
            json!([{"op": "replace", "path": "/a"}]),
            json!({"op": "replace"}),
        ];
        for ops in refused {
            assert!(patched(&g, &ops).is_err(), "{ops}");
        }
    }

    #[test]
    fn pointer_segments_are_unescaped() {
        let g = json!({"a/b": 1, "m~n": 2});
        let ops = json!([
            {"op": "replace", "path": "/a~1b", "value": 3},
            {"op": "replace", "path": "/m~0n", "value": 4}
        ]);
        assert_eq!(patched(&g, &ops).unwrap(), json!({"a/b": 3, "m~n": 4}));
    }
}
