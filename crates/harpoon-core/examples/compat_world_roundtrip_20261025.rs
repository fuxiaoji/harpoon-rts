//! Proposed native diagnostic. Root must register/build/run; child never does.
//! Reads historical saves only. No source activation, no writes, no ticking.
//! Structural comparison is not a substitute for server's typed Payload FNV.
use harpoon_core::World;
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn differences(old: &Value, typed: &Value, path: &str, out: &mut Vec<Value>) {
    match (old, typed) {
        (Value::Object(a), Value::Object(b)) => {
            let keys: BTreeSet<_> = a.keys().chain(b.keys()).collect();
            for key in keys {
                let at = format!("{path}/{key}");
                match (a.get(key), b.get(key)) {
                    (Some(x), Some(y)) => differences(x, y, &at, out),
                    (None, Some(y)) => {
                        out.push(json!({"path":at,"change":"added_after_typed_roundtrip","new":y}))
                    }
                    (Some(x), None) => out
                        .push(json!({"path":at,"change":"removed_after_typed_roundtrip","old":x})),
                    _ => unreachable!(),
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                out.push(json!({"path":path,"change":"array_length","old":a.len(),"new":b.len()}));
            }
            for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
                differences(x, y, &format!("{path}/{i}"), out);
            }
        }
        _ if old != typed => out.push(json!({"path":path,"change":"value","old":old,"new":typed})),
        _ => {}
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<_> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        return Err("usage: compat_world_roundtrip_candidate <historical-save.json> [...]".into());
    }
    let mut changed = false;
    for path in paths {
        let original = std::fs::read(&path)?;
        let save: Value = serde_json::from_slice(&original)?;
        let mut checks = vec![];
        for (label, raw) in [
            ("payload.world", &save["payload"]["world"]),
            (
                "payload.recording.initial",
                &save["payload"]["recording"]["initial"],
            ),
        ] {
            let typed: World = serde_json::from_value(raw.clone())?;
            let after = serde_json::to_value(&typed)?;
            let mut diff = vec![];
            differences(raw, &after, label, &mut diff);
            changed |= !diff.is_empty();
            checks.push(json!({
                "world":label,
                "tick":typed.tick,
                "typed_roundtrip_structure_equal":diff.is_empty(),
                "differences":diff,
                "source_validation_without_re_adjudication":typed.rules.validate_tables(),
            }));
        }
        assert_eq!(
            std::fs::read(&path)?,
            original,
            "historical original is immutable"
        );
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "path":path,"stored_payload_checksum":save["checksum_fnv64"],
                "native_world_roundtrip":checks,"original_bytes_unchanged":true,
                "scope":"Native World structural diagnosis only. Does not compute private typed server Payload checksum or claim load/replay acceptance."
            }))?
        );
    }
    if changed {
        return Err(
            "historical World changed after native typed roundtrip; exact paths printed above"
                .into(),
        );
    }
    Ok(())
}
