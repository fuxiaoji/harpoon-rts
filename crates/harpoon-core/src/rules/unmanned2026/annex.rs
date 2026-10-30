//! Annex B printed control-field qualification for real unmanned platforms
//! (Americas Aircraft 1 Dec 25: MQ-4C Triton B-65/66, MQ-25A Stingray B-67;
//! pages visually reviewed 2026-10-30). The annex opt-in certifies ONLY the
//! control facts printed on those pages: satellite control, the printed LOS
//! cap and the printed platform rows themselves. Operational choices (initial
//! mode, controller/element binding, loss behavior, satellite availability),
//! antenna geometry and endurance work assumptions stay explicitly
//! scenario-authored. Nothing here claims payload, equipment or endurance
//! qualification, and autonomy is not claimed because the pages do not print
//! it; `autonomous_capable=false` under the annex is a scenario limitation,
//! not a statement about real hardware.
use super::*;

pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../data/rules/unmanned-annex-2025.json"
        ))
        .expect("verified Annex B control-field source rows")
    })
}

fn control_value(class: &str, key: &str) -> Result<&'static Value, String> {
    tables()["platforms"]
        .get(class)
        .ok_or_else(|| format!("4-4 Annex B: no printed control-field row for class {class}"))?
        ["control"]
        .get(key)
        .ok_or_else(|| format!("4-4 Annex B: malformed source row for {class}"))
}

/// `annex_platform: None` keeps the legacy author fit unchanged. `Some(name)`
/// must equal the actual entity class, exist as a printed row, stay an air
/// vehicle, and only claim control facts the page actually prints.
pub fn validate(class: &str, fit: &super::VehicleFit) -> Result<(), String> {
    let Some(annex_platform) = fit.annex_platform.as_deref() else {
        return Ok(());
    };
    if annex_platform != class {
        return Err(format!(
            "4-4 Annex B: annex_platform {annex_platform} does not match actual class {class}"
        ));
    }
    control_value(class, "is_uav")?;
    if fit.domain != VehicleDomain::Air {
        return Err(format!(
            "4-4 Annex B: {class} is printed as an air vehicle; annex domain must be air"
        ));
    }
    if fit.satellite
        && !control_value(class, "satellite")?["value"]
            .as_bool()
            .unwrap_or(false)
    {
        return Err(format!(
            "4-4 Annex B: satellite control is not printed for {class}; annex fit cannot claim it"
        ));
    }
    let printed_los = control_value(class, "los_range_nm")?["value"].as_f64();
    match (fit.los_range_nm, printed_los) {
        (Some(_), None) => {
            return Err(format!(
                "4-4 Annex B: {class} prints no LOS control-link range; a LOS number cannot be assigned by name"
            ));
        }
        (Some(fitted), Some(cap)) if fitted > cap => {
            return Err(format!(
                "4-4 Annex B: fitted LOS {fitted} nm exceeds the printed {class} cap of {cap} nm"
            ));
        }
        _ => {}
    }
    if fit.initial_link == ControlLink::LineOfSight && printed_los.is_none() {
        return Err(format!(
            "4-4 Annex B: {class} prints no LOS control link; the LOS link cannot be used by name"
        ));
    }
    if fit.autonomous_capable {
        return Err(format!(
            "4-4 Annex B: autonomy is not printed for {class}; keep autonomous_capable=false as an explicit scenario limitation"
        ));
    }
    if fit.acoustic_modem {
        return Err(format!(
            "4-4 Annex B: acoustic modem is not printed for {class}; unknown hardware cannot be claimed as a new capability"
        ));
    }
    if fit.initial_link == ControlLink::AcousticModem {
        return Err(format!(
            "4-4 Annex B: acoustic modem link is not printed for {class}"
        ));
    }
    Ok(())
}
