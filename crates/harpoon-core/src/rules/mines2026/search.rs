//! Independent physical mine echoes. A sonar return never supplies mine
//! classification, IFF, a mine model, or an ordinary weapon/FCS target.
use super::*;
use crate::rules::detection2026 as d;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchFit {
    pub source: String,
    pub platform_fit: String,
    pub sensor: String,
    /// Explicit author input for fictional equipment only. Never fills a
    /// missing Annex K cell for a named historical platform or sonar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_assumption: Option<AssumedSearchReceiver>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssumedSearchReceiver {
    pub source: String,
    pub generation: usize,
    pub frequency: d::Frequency,
    pub mount: d::SonarMount,
    pub active50_nm: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaseDepth {
    pub source: String,
    pub depth_m: f64,
}
/// Private save state; body is deliberately omitted by own_echoes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Echo {
    pub id: String,
    pub body: String,
    pub side: Side,
    pub first_seen: i64,
    pub last_seen: i64,
    pub reports: BTreeMap<String, EchoReport>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EchoReport {
    pub observer: EntityId,
    pub sensor: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub evidence_basis: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    pub first_seen: i64,
    pub at: i64,
    #[serde(flatten)]
    pub geometry: d::ReportGeometry,
}
/// Only fully verified published receivers may be registered automatically.
/// Blocked source rows and scenario assumptions never add database equipment.
pub fn configure_database(db: &mut crate::Database) {
    for profile in tables()["search"]["profiles"].as_object().unwrap().values() {
        if profile["verification"] != "verified_source" {
            continue;
        }
        let def: crate::db::SensorDef =
            serde_json::from_value(profile["sensor_definition"].clone()).unwrap();
        db.sensors.insert(def.id.clone(), def);
    }
}
fn profile(fit: &SearchFit, e: &crate::world::Entity) -> Option<Value> {
    if let Some(a) = &fit.scenario_assumption {
        // Assumptions must retain unmistakable author identities in both the
        // configuration and observations; they cannot shadow a published fit.
        if !fit.source.trim().starts_with("scenario_assumption:")
            || !a.source.trim().starts_with("scenario_assumption:")
            || !fit.platform_fit.starts_with("scenario_assumption:")
            // Existing spawn/SetSonar paths identify active sonar by sonar_a.
            // Retain that operational prefix and the explicit author namespace.
            || !fit.sensor.starts_with("sonar_a_scenario_assumption:")
            || fit.platform_fit != e.class
            || !a.active50_nm.is_finite()
            || a.active50_nm <= 0.
            || !(1..=7).contains(&a.generation)
            || a.mount != d::SonarMount::Keel
        {
            return None;
        }
        return Some(
            json!({"source":a.source,"evidence_basis":"scenario_assumption",
            "platform_class":fit.platform_fit,"sensor":fit.sensor,
            "capability":"dedicated_bottom_minehunting","generation":a.generation,
            "frequency":a.frequency,"mount":a.mount,"active50_nm":a.active50_nm,
            "direct_only":true}),
        );
    }
    let p = tables()["search"]["profiles"].get(&fit.platform_fit)?;
    (!fit.source.trim().is_empty()
        && p["verification"] == "verified_source"
        && p["platform_class"].as_str() == Some(e.class.as_str())
        && p["sensor"].as_str() == Some(fit.sensor.as_str())
        && p["capability"] == "dedicated_bottom_minehunting")
        .then(|| {
            let mut p = p.clone();
            p["evidence_basis"] = json!("verified_source");
            p
        })
}
pub(super) fn initialize(w: &mut World, c: &Configuration) -> Result<(), String> {
    let mut bindings = vec![];
    for (name, fits) in &c.search_fits {
        let units: Vec<_> = w
            .entities
            .values()
            .filter(|e| e.name == *name && !e.kind.is_projectile())
            .collect();
        if units.len() != 1 {
            return Err(
                "5.4.6.1 source mine-search fit must bind exactly one physical unit name".into(),
            );
        }
        let e = units[0];
        let mut sensors = BTreeSet::new();
        for fit in fits {
            if !sensors.insert(fit.sensor.clone()) {
                return Err(
                    "5.4.6.1 duplicate receiver fit cannot multiply mine search trials".into(),
                );
            }
            let Some(p) = profile(fit, e) else { continue };
            let receiver = d::Receiver::Sonar {
                source: p["source"].as_str().unwrap().into(),
                generation: p["generation"].as_u64().unwrap() as usize,
                passive_range_nm: None,
                active_range_nm: p["active50_nm"].as_f64(),
                frequency: serde_json::from_value(p["frequency"].clone()).unwrap(),
                mount: serde_json::from_value(p["mount"].clone()).unwrap(),
                active_torpedo_detection: false,
                high_speed_tow: false,
            };
            bindings.push((format!("{}:{}", e.class, fit.sensor), receiver));
        }
    }
    // This also exposes the actual source active emission to the ordinary AIR
    // path; a mine-search declaration must not create a silent active sonar.
    if let Some(state) = w.rules.detection.as_mut() {
        for (key, receiver) in bindings {
            if state.config.bindings.get(&key).is_some_and(|old| {
                serde_json::to_value(old).unwrap() != serde_json::to_value(&receiver).unwrap()
            }) {
                return Err(
                    "5.4.6.1 source mine-search receiver conflicts with explicit sonar binding"
                        .into(),
                );
            }
            state.config.bindings.insert(key, receiver);
        }
    }
    Ok(())
}
pub(super) fn own_echoes(s: &State, side: Side) -> Vec<Value> {
    let mut echoes: Vec<_> = s.echoes.values().filter(|e| e.side == side).collect();
    echoes.sort_by(|a, b| a.id.cmp(&b.id));
    echoes
        .into_iter()
        .map(|e| {
            json!({
                "id":e.id,"classified":false,"first_seen":e.first_seen,"last_seen":e.last_seen,
                "reports":e.reports.values().collect::<Vec<_>>()
            })
        })
        .collect()
}
fn blocked(
    s: &mut State,
    traces: &mut Vec<(EntityId, Value)>,
    observer: EntityId,
    sensor: &str,
    body: &str,
    now: i64,
    reason: String,
    period: i64,
) {
    let key = format!(
        "5.mine.blocked:{}:{sensor}:{body}:{}:{reason}",
        observer.0,
        now.div_euclid(period)
    );
    if s.diagnostics.insert(key) {
        traces.push((
            observer,
            json!({"sensor":sensor,"truth_body":body,"blocked":reason}),
        ));
    }
}
fn parameters(
    state: &d::State,
    c: &Configuration,
    b: &Body,
    e: &crate::world::Entity,
    p: &Value,
) -> Result<(i32, i32, i32, i32, f64, f64, f64), String> {
    if mine(&b.mine)?["kind"] != "bottom" {
        return Err(
            "5.4.6.1 this source profile currently implements bottom mine returns only".into(),
        );
    }
    let z = c
        .case_depths
        .get(&b.id)
        .filter(|z| !z.source.trim().is_empty())
        .ok_or("5.4.6.1 exact mine case depth has no source; no band midpoint assumed")?;
    if !z.depth_m.is_finite() || z.depth_m < 0. || depth_band(z.depth_m)? != b.case_band {
        return Err("5.4.6.1 exact case depth does not match source band".into());
    }
    let env = state
        .config
        .environment
        .as_ref()
        .filter(|v| !v.source.trim().is_empty())
        .ok_or("5.4.6.1 source water depth/sea/layer environment required")?;
    if !env.water_depth_m.is_finite()
        || env.water_depth_m <= 0.
        || (env.water_depth_m - z.depth_m).abs() > 1e-6
    {
        return Err(
            "5.4.6.1 bottom case depth must match the explicitly surveyed local bottom depth"
                .into(),
        );
    }
    if !e.depth_m.is_finite() || e.depth_m < 0. || e.depth_m > env.water_depth_m {
        return Err("5.4.6.1 invalid actual keel sonar depth".into());
    }
    let frequency: d::Frequency = serde_json::from_value(p["frequency"].clone()).unwrap();
    let relative = d::sonar_speed_modifier(true, 0., e.speed_kts, false)?
        .ok_or("5.4.6 own speed blinds mine-search receiver")?;
    let environment = d::sonar_environment(true, frequency, env)?;
    let layer = d::layer_modifier(frequency, false, e.depth_m, z.depth_m, env.layer)?;
    let strength = d::tables()["sonar"]["active_strength"][tables()["search"]
        ["mine_strength_size_index"]
        .as_u64()
        .unwrap() as usize]
        .as_i64()
        .unwrap() as i32;
    let modified = p["active50_nm"].as_f64().unwrap()
        * d::sonar_range_modifier(strength + relative + environment + layer)?;
    Ok((
        strength,
        relative,
        environment,
        layer,
        modified,
        d::sonar_selected_50(modified)?,
        e.pos.range_to(b.position),
    ))
}
pub(super) fn step(w: &mut World) {
    let Some(c) = w
        .context
        .mines_2026
        .clone()
        .filter(|c| !c.search_fits.is_empty())
    else {
        return;
    };
    let Some(mut ds) = w.rules.detection.take() else {
        return;
    };
    let Some(mut ms) = w.rules.mines.take() else {
        w.rules.detection = Some(ds);
        return;
    };
    let period = tables()["search"]["trial_period_s"].as_i64().unwrap();
    let window = w.tick.div_euclid(period);
    let units: Vec<_> = w
        .entities
        .values()
        .filter(|e| e.alive && e.kind == PlatformKind::Ship)
        .cloned()
        .collect();
    let bodies: Vec<_> = ms
        .bodies
        .values()
        .filter(|b| !b.neutralized && b.released_at <= w.tick)
        .cloned()
        .collect();
    let mut traces = vec![];
    for e in units {
        let Some(fits) = c.search_fits.get(&e.name) else {
            continue;
        };
        for fit in fits {
            let Some(p) = profile(fit, &e) else {
                blocked(
                    &mut ms,
                    &mut traces,
                    e.id,
                    &fit.sensor,
                    "",
                    w.tick,
                    "5.4.6.1 no verified minehunting platform/receiver fit".into(),
                    period,
                );
                continue;
            };
            let Some(def) = w.db.sensors.get(&fit.sensor) else {
                continue;
            };
            // The declaration cannot replace a real, working active receiver.
            if def.kind != crate::db::SensorKind::SonarActive
                || !e.sensors.iter().any(|s| s.sensor == fit.sensor && s.active)
                || e.systems.sonar <= 0.05
                || crate::sensors::sensor_damaged(&e, &fit.sensor)
                || e.ext.get("damage.power_lost").copied().unwrap_or(0.) > 0.
            {
                continue;
            }
            let mode_key = format!("{}:{}", e.id.0, p["source"].as_str().unwrap());
            if ds
                .modes
                .get(&mode_key)
                .is_some_and(|(win, active)| *win == window && !active)
            {
                continue;
            }
            ds.modes.insert(mode_key, (window, true));
            let mount: d::SonarMount = serde_json::from_value(p["mount"].clone()).unwrap();
            let generation = p["generation"].as_u64().unwrap() as usize;
            for b in &bodies {
                if !d::sonar_bearing_eligible(
                    mount,
                    generation,
                    crate::geom::angle_diff(e.course_deg, e.pos.bearing_to(b.position)),
                ) {
                    continue;
                }
                if !matches!(
                    ds.config
                        .propagation
                        .get(&format!("{}:{}", e.class, fit.sensor)),
                    None | Some(d::Propagation::Direct)
                ) {
                    blocked(
                        &mut ms,
                        &mut traces,
                        e.id,
                        &fit.sensor,
                        &b.id,
                        w.tick,
                        "5.4.6.1 this mine profile has no verified indirect propagation path"
                            .into(),
                        period,
                    );
                    continue;
                }
                let (strength, relative, environment, layer, modified, selected, range) =
                    match parameters(&ds, &c, b, &e, &p) {
                        Ok(v) => v,
                        Err(reason) => {
                            blocked(
                                &mut ms,
                                &mut traces,
                                e.id,
                                &fit.sensor,
                                &b.id,
                                w.tick,
                                reason,
                                period,
                            );
                            continue;
                        }
                    };
                // A separate receiver's successful report can alert this sonar
                // in following Tactical Turns. Same-turn truth is never shared.
                let alert = ms
                    .echoes
                    .values()
                    .filter(|x| x.side == e.side && x.body == b.id)
                    .flat_map(|x| x.reports.values())
                    .any(|r| {
                        (r.observer != e.id || r.sensor != fit.sensor)
                            && r.first_seen.div_euclid(period) < window
                    });
                let chance = d::sonar_probability(modified, range, alert).unwrap();
                let key = format!("5.mine:{}:{}:{}", e.id.0, fit.sensor, b.id);
                let (roll, reused) = d::trial(&mut ds, w.seed, w.tick, &key, period);
                let detected = roll as f64 <= chance;
                if !reused {
                    traces.push((e.id,json!({"sensor":fit.sensor,"truth_body":b.id,"source":p["source"],"evidence_basis":p["evidence_basis"],
                        "base50_nm":p["active50_nm"],"target_strength":strength,"relative_speed":relative,
                        "environment":environment,"layer":layer,"total":strength+relative+environment+layer,
                        "modified50_nm":modified,"selected50_nm":selected,"range_nm":range,
                        "alerted_other_receiver":alert,"probability_percent":chance,"d100":roll,"reused":false,
                        "detected":detected,"classified":false})));
                }
                if !detected {
                    continue;
                }
                let scan = d::ScanDecision {
                    source_point:None,
                    sonar_attempt:None,
                    detected: true,
                    known_category: None,
                    group: None,
                    bearing_only: false,
                    identify_class: false,
                    class_label: None,
                    range_known: true,
                    bearing_error_deg: None,
                    fix_quality: None,
                    trace: Value::Null,
                };
                let (geometry, _) = d::project_point_report(
                    w,
                    &mut ds,
                    &e,
                    b.position,
                    json!(b.id),
                    &format!("5.mine.bearing:{}:{}:{}", e.id.0, fit.sensor, b.id),
                    &fit.sensor,
                    &scan,
                );
                let ek = format!("{}:{}", e.side.index(), b.id);
                if !ms.echoes.contains_key(&ek) {
                    let mut sequence = ms
                        .echo_sequence
                        .get(&e.side.index())
                        .copied()
                        .unwrap_or_default();
                    let opaque = loop {
                        sequence += 1;
                        let candidate = format!("echo-{sequence}");
                        if !ms
                            .classified
                            .iter()
                            .any(|c| c.side == e.side && c.id == candidate)
                            && !ms
                                .echoes
                                .values()
                                .any(|x| x.side == e.side && x.id == candidate)
                        {
                            break candidate;
                        }
                    };
                    ms.echo_sequence.insert(e.side.index(), sequence);
                    ms.echoes.insert(
                        ek.clone(),
                        Echo {
                            id: opaque,
                            body: b.id.clone(),
                            side: e.side,
                            first_seen: w.tick,
                            last_seen: w.tick,
                            reports: BTreeMap::new(),
                        },
                    );
                }
                let echo = ms.echoes.get_mut(&ek).unwrap();
                echo.last_seen = w.tick;
                let rk = format!("{}:{}", e.id.0, fit.sensor);
                let first_seen = echo.reports.get(&rk).map_or(w.tick, |r| r.first_seen);
                echo.reports.insert(
                    rk,
                    EchoReport {
                        observer: e.id,
                        sensor: fit.sensor.clone(),
                        evidence_basis: p["evidence_basis"].as_str().unwrap().into(),
                        source: p["source"].as_str().unwrap().into(),
                        first_seen,
                        at: w.tick,
                        geometry,
                    },
                );
            }
        }
    }
    w.rules.detection = Some(ds);
    w.rules.mines = Some(ms);
    for (observer, detail) in traces {
        referee(w, Some(observer), "5.mine.scan", detail);
    }
}
