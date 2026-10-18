//! Database volume checker / consolidator.
//!
//! Usage:
//!   cargo run --release -p harpoon-server --example dbcheck -- [--only FILE] [--emit PATH]
//!
//! - Loads the built-in database, then every `database/volumes/*.json`
//!   (files starting with `_` are skipped).
//! - `--only FILE` validates a single volume in isolation (built-in + that
//!   file only) — used by data-entry agents while other volumes are in
//!   flight.
//! - `--emit PATH` writes the merged database as one JSON file.
//!
//! Exits nonzero on parse errors, referential-integrity errors, or a volume
//! that adds zero platforms.

use harpoon_core::db::Database;

fn load_volume(path: &std::path::Path) -> Result<Database, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    Database::from_json(&txt).map_err(|e| format!("parse {}: {e}\n  first 400 chars: {}", path.display(), {
        let mut head = txt.as_str();
        if head.len() > 400 {
            head = &head[..400];
        }
        head.to_string()
    }))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let only = args.iter().position(|a| a == "--only").and_then(|i| args.get(i + 1).cloned());
    let emit = args.iter().position(|a| a == "--emit").and_then(|i| args.get(i + 1).cloned());
    let dir = std::path::PathBuf::from("database/volumes");

    let base = Database::built_in();
    let base_counts = (base.platforms.len(), base.weapons.len(), base.sensors.len());
    eprintln!(
        "built-in: {} platforms, {} weapons, {} sensors",
        base_counts.0, base_counts.1, base_counts.2
    );

    let mut db = base.clone();
    let mut volumes: Vec<std::path::PathBuf> = match &only {
        Some(f) => vec![std::path::PathBuf::from(f)],
        None => {
            let mut v: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
                .map(|rd| {
                    rd.flatten()
                        .map(|e| e.path())
                        .filter(|p| {
                            p.extension().map(|x| x == "json").unwrap_or(false)
                                && !p.file_name().map(|n| n.to_string_lossy().starts_with('_')).unwrap_or(false)
                        })
                        .collect()
                })
                .unwrap_or_default();
            v.sort();
            v
        }
    };
    volumes.sort();

    if volumes.is_empty() {
        eprintln!("no volume files found (dir {}, only={:?})", dir.display(), only);
        std::process::exit(2);
    }

    let mut failures = 0;
    for path in &volumes {
        match load_volume(path) {
            Ok(vol) => {
                let (p, w, s) = (vol.platforms.len(), vol.weapons.len(), vol.sensors.len());
                if p == 0 {
                    eprintln!("FAIL {}: zero platforms", path.display());
                    failures += 1;
                    continue;
                }
                // Referential integrity within the volume against merged-so-far.
                if let Err(e) = db.merged(&vol).validate() {
                    eprintln!("FAIL {}: {e}", path.display());
                    failures += 1;
                    continue;
                }
                db = db.merged(&vol);
                eprintln!(
                    "OK {}: +{} platforms, +{} weapons, +{} sensors",
                    path.display(),
                    p,
                    w,
                    s
                );
            }
            Err(e) => {
                eprintln!("FAIL {e}");
                failures += 1;
            }
        }
    }

    if failures > 0 {
        eprintln!("\nDBCHECK FAIL — {failures} volume(s) with errors");
        std::process::exit(1);
    }

    eprintln!(
        "\nDBCHECK PASS — merged: {} platforms, {} weapons, {} sensors",
        db.platforms.len(),
        db.weapons.len(),
        db.sensors.len()
    );

    if let Some(out) = emit {
        let json = serde_json::to_string_pretty(&db).expect("serialize");
        std::fs::write(&out, json).expect("write consolidated db");
        eprintln!("wrote consolidated database to {out}");
    }
}
