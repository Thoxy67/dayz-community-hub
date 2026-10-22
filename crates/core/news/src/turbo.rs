//! React Router's turbo-stream, read into JSON.
//!
//! dayz.com is a React Router site: a route's `.data` address answers its loader's data
//! as a turbo-stream. The first line is a JSON array of values; a value refers to the
//! others by index: an object is `{"_<key index>": <value index>}`, an array a list of
//! indices, a negative index one of a few constants. Later lines resolve promises, which
//! the news routes do not use: they are ignored.

use serde_json::{Map, Value};

/// How deep a value may nest, and how many values one decode may produce: shared
/// references can make a small stream expand without end.
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 2_000_000;

/// The first line of a turbo-stream, as JSON; `None` when it is not one.
pub fn decode(stream: &str) -> Option<Value> {
    let first = stream.lines().next()?;
    let values: Vec<Value> = serde_json::from_str(first).ok()?;
    let mut budget = MAX_NODES;
    hydrate(&values, 0, 0, &mut budget)
}

fn hydrate(values: &[Value], index: i64, depth: usize, budget: &mut usize) -> Option<Value> {
    if index < 0 {
        // Holes, NaN, the infinities, -0, null and undefined: none is news.
        return Some(Value::Null);
    }
    if depth > MAX_DEPTH || *budget == 0 {
        return None;
    }
    *budget -= 1;
    let v = values.get(usize::try_from(index).ok()?)?;
    Some(match v {
        Value::Array(items) => {
            if let Some(Value::String(tag)) = items.first() {
                // A typed value: a date is its time, the rest are not needed.
                return Some(match (tag.as_str(), items.get(1)) {
                    ("D", Some(Value::Number(n))) => Value::Number(n.clone()),
                    _ => Value::Null,
                });
            }
            let mut out = Vec::with_capacity(items.len());
            for i in items {
                out.push(hydrate(values, i.as_i64()?, depth + 1, budget)?);
            }
            Value::Array(out)
        }
        Value::Object(fields) => {
            let mut out = Map::new();
            for (k, i) in fields {
                let key_index = k.strip_prefix('_')?.parse::<usize>().ok()?;
                let Some(Value::String(key)) = values.get(key_index) else {
                    return None;
                };
                out.insert(
                    key.clone(),
                    hydrate(values, i.as_i64()?, depth + 1, budget)?,
                );
            }
            Value::Object(out)
        }
        other => other.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn objects_arrays_and_constants() {
        let s = r#"[{"_1":2,"_3":4,"_5":-5},"a",[6,6],"b",true,"c","x"]
P1:[]"#;
        assert_eq!(
            decode(s),
            Some(json!({"a": ["x", "x"], "b": true, "c": null}))
        );
    }

    #[test]
    fn dates_are_their_time() {
        assert_eq!(
            decode(r#"[["D",1700000000000]]"#),
            Some(json!(1_700_000_000_000_i64))
        );
    }

    #[test]
    fn hostile_streams_do_not_panic() {
        for s in [
            "",
            "not json",
            "[]",
            "[[0]]",
            r#"[{"_9":0}]"#,
            r#"[{"x":1},2]"#,
            r#"[{"_0":0}]"#,
            "[[1,1],[2,2],[3,3],[4,4],[5,5],[6,6],[7,7],[8,8],[9,9],[10,10],[11,11],[12,12],[13,13],[14,14],[15,15],[16,16],[17,17],[18,18],[19,19],[20,20],[21,21],[22,22],[23,23],[24,24],[25,25],[25,25]]",
        ] {
            let _ = decode(s);
        }
    }
}
