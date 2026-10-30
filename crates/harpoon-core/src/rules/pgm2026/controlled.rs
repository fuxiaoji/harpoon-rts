//! Shared registry/dispatch for source-controlled air ordnance families.
//! Each family owns its declarations and physical guidance evidence; parent
//! PGM owns the common release, placement, finite stock and damage pipeline.
pub mod command_sarh;
pub mod eoir1;
use super::*;

/// A scenario author qualifies an unobstructed local optical volume. The
/// printed rule supplies the LOS consequence, not a universal terrain model.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LosConfiguration {
    pub source: String,
    pub minimum_aircraft_altitude_m: f64,
    pub maximum_ray_range_nm: f64,
    pub no_intervening_terrain: bool,
}
impl LosConfiguration {
    pub fn validate(&self) -> Result<(),String> {
        if !self.source.starts_with("scenario_assumption:")
            || !self.minimum_aircraft_altitude_m.is_finite()
            || self.minimum_aircraft_altitude_m <= 0.
            || !self.maximum_ray_range_nm.is_finite() || self.maximum_ray_range_nm <= 0.
            || !self.no_intervening_terrain {
            Err("9.6.8 controlled optical volume requires explicit authored bounds/clear-terrain qualification".into())
        } else { Ok(()) }
    }
}

pub fn tables() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| json!({"command_sarh":command_sarh::tables(),"eoir1":eoir1::tables(),
        "los_execution":{"source":"English2026 9.6.8 continuous LOS; existing5.8 cloud geometry; scenario-declared terrain and smoke shape",
        "policy":"Require explicit authored controlled_pgm_los_2026 clearance bounds; an actual ray outside them is pending. Sample after movement/casualties with no global scan; intervening chart without height qualification is pending, survival alone never means clear LOS; smoke intersects the actual sensor-target ray in three dimensions."}}))
}
pub(super) fn external_store_weight(class: &str, weapon: &str) -> Option<f64> {
    eoir1::external_store_weight(class, weapon)
        .or_else(|| command_sarh::external_store_weight(class, weapon))
}
pub(super) fn configure(w: &World, state: &mut super::State) -> Result<(), String> {
    if let Some(scene) = &w.context.controlled_pgm_los_2026 { scene.validate()?; }
    command_sarh::configure(w, state)?;
    eoir1::configure(w, state)
}
pub(super) fn resolve_deferred(w: &mut World) {
    command_sarh::resolve_deferred(w);
    eoir1::resolve_deferred(w);
}

/// Referee-only continuous optical ray using the existing, cached source cloud
/// procedure. A coastline is not an airborne terrain elevation model.
pub(in crate::rules::pgm2026) fn sample_aircraft_los(
    w: &mut World, actor: EntityId, target: EntityId,
) -> Result<Value, String> {
    let from = w.entity(actor).cloned().ok_or("9.6.8 firing-aircraft body unavailable")?;
    let to = w.entity(target).cloned().ok_or("9.6.8 guidance target body unavailable")?;
    if !from.alive || from.kind != PlatformKind::Aircraft || from.altitude_m <= 0. {
        return Err("9.6.8 firing controller no longer airborne/available; post-casualty control consequence not source qualified".into());
    }
    let scene=w.context.controlled_pgm_los_2026.as_ref()
        .ok_or("9.6.8 optical clearance volume not declared; aircraft survival alone is not LOS")?;
    scene.validate()?;
    if from.altitude_m < scene.minimum_aircraft_altitude_m
        || from.pos.range_to(to.pos) > scene.maximum_ray_range_nm {
        return Err("9.6.8 actual ray left its authored clearance volume; LOS remains unqualified".into());
    }
    if w.context.terrain.as_ref().is_some_and(|chart| chart.blocked(from.pos, to.pos)) {
        return Err("9.6.8 intervening terrain has no qualified airborne elevation/clearance profile; LOS pending".into());
    }
    let env = w.rules.detection.as_ref().and_then(|s| s.config.environment.clone())
        .ok_or("9.6.8 explicit optical environment required for continuous LOS")?;
    if env.source.trim().is_empty() || env.visibility_percent == 0
        || (w.weather.overcast > 0. && env.cloud.is_none()) {
        return Err("9.6.8 optical/weather visibility is not qualified by an explicit ray environment".into());
    }
    let (gap, multiplier, cloud_d100) = crate::rules::detection2026::cloud_effect(
        w.rules.detection.as_mut().unwrap(), w.seed, w.tick,
        actor, target, from.altitude_m, to.altitude_m, env.cloud.as_ref(),
    )?;
    Ok(json!({"at_tick":w.tick,"clear":gap,"from":from.pos,"to":to.pos,
        "from_altitude_m":from.altitude_m,"to_altitude_m":to.altitude_m,
        "environment_source":env.source,"cloud_d100":cloud_d100,"cloud_multiplier":multiplier,
        "clearance_volume_source":scene.source,
        "policy":"actual allocated ray; no detector/FCS scan repeated"}))
}

/// Author-declared finite smoke cylinder versus an actual 3-D sensor ray.
/// The source supplies the LOS consequence, not these scenario dimensions.
pub(in crate::rules::pgm2026) fn ray_intersects_smoke(
    from: Vec2, from_m: f64, to: Vec2, to_m: f64,
    center: Vec2, radius_nm: f64, bottom_m: f64, top_m: f64,
) -> Result<bool, String> {
    if ![from.x,from.y,to.x,to.y,center.x,center.y,from_m,to_m,radius_nm,bottom_m,top_m]
        .into_iter().all(f64::is_finite) || radius_nm <= 0. || top_m <= bottom_m {
        return Err("9.6.8 declared smoke/ray geometry invalid".into());
    }
    let dx=to.x-from.x;let dy=to.y-from.y;
    let ox=from.x-center.x;let oy=from.y-center.y;
    let a=dx*dx+dy*dy;let b=2.*(ox*dx+oy*dy);
    let c=ox*ox+oy*oy-radius_nm*radius_nm;
    let (mut enter,mut leave)=(0_f64,1_f64);
    if a==0. { if c>0. { return Ok(false); } }
    else {
        let discriminant=b*b-4.*a*c;
        if discriminant<0. { return Ok(false); }
        let root=discriminant.sqrt();
        enter=enter.max((-b-root)/(2.*a));
        leave=leave.min((-b+root)/(2.*a));
    }
    let dz=to_m-from_m;
    if dz==0. { if from_m<bottom_m || from_m>top_m {return Ok(false);} }
    else {
        let first=(bottom_m-from_m)/dz;let second=(top_m-from_m)/dz;
        enter=enter.max(first.min(second));leave=leave.min(first.max(second));
    }
    Ok(enter<=leave)
}
