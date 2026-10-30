//! Shared transport descriptions. Legality and source completeness remain engine decisions.
use serde_json::{json, Map, Value};

fn number() -> Value {
    json!({"type":"number"})
}
fn nonnegative() -> Value {
    json!({"type":"number","minimum":0})
}
fn integer(min: u64, max: Option<u64>) -> Value {
    let mut v = json!({"type":"integer","minimum":min});
    if let Some(max) = max {
        v["maximum"] = max.into();
    }
    v
}
fn unit() -> Value {
    integer(1, None)
}
fn contact() -> Value {
    integer(0, None)
}
fn text() -> Value {
    json!({"type":"string"})
}
fn boolean() -> Value {
    json!({"type":"boolean"})
}
fn choice(values: &[&str]) -> Value {
    json!({"type":"string","enum":values})
}
fn array(items: Value) -> Value {
    json!({"type":"array","items":items})
}
fn optional(value: Value) -> Value {
    json!({"anyOf":[value,{"type":"null"}]})
}
fn point() -> Value {
    // Vec2 serializes canonically as an object; old saves also accept a pair.
    json!({"type":["object","array"],"properties":{"x":number(),"y":number()},"required":["x","y"],"additionalProperties":false,"items":number(),"minItems":2,"maxItems":2,"description":"Nautical miles, +x east, +y north"})
}
fn object(fields: Vec<(&str, Value)>, optional_fields: &[&str]) -> Value {
    let mut p = Map::new();
    let mut required = Vec::new();
    for (name, value) in fields {
        p.insert(name.into(), value);
        if !optional_fields.contains(&name) {
            required.push(name);
        }
    }
    json!({"type":"object","properties":p,"required":required,"additionalProperties":false})
}
fn variant(tag: &str, name: &str, fields: Vec<(&str, Value)>, optional_fields: &[&str]) -> Value {
    let mut fields = fields;
    fields.insert(0, (tag, json!({"const":name})));
    object(fields, optional_fields)
}
fn actions(rows: Vec<(&str, Vec<(&str, Value)>, Vec<&str>)>) -> Value {
    json!({"type":"object","oneOf":rows.into_iter().map(|(name,fields,optional)|variant("type",name,fields,&optional)).collect::<Vec<_>>()})
}
pub fn mission_schema() -> Value {
    json!({"type":"object","oneOf":[
        variant("mission","cap",vec![("station",point()),("radius_nm",nonnegative()),("follow",boolean())],&["follow"]),
        variant("mission","strike",vec![("contact",contact())],&[]),
        variant("mission","asw",vec![("area",point()),("radius_nm",nonnegative())],&[]),
        variant("mission","recon",vec![("point",point())],&[]),
        variant("mission","rtb",vec![],&[])
    ]})
}
fn action_schema(group: &str) -> Value {
    let mut rows: Vec<(&str, Vec<(&str, Value)>, Vec<&str>)> = Vec::new();
    match group {
        "asw" => {
            let mut pair_physicals=array(integer(0,None));pair_physicals["minItems"]=json!(1);pair_physicals["maxItems"]=json!(2);pair_physicals["uniqueItems"]=json!(true);
            rows.push(("projector_pair_pattern",vec![("contact",contact()),("weapon",text()),("physicals",pair_physicals),("phase",choice(&["planned","reaction"]))],vec![]));
            rows.push(("projector_pattern",vec![("contact",contact()),("weapon",text()),("physical",integer(0,None)),("phase",choice(&["planned","reaction"]))],vec![]));
            let allocations=array(object(vec![("physical",integer(0,None)),("charges",integer(1,Some(65535)))],&[]));
            rows.push(("depth_charge_pattern",vec![("contact",contact()),("weapon",text()),("charges",integer(1,Some(65535))),("depth_band",integer(0,None)),("distribution",choice(&["straight","dispersed"])),("allocations",allocations.clone())],vec![]));
            rows.push(("depth_charge_evasion",vec![("evading",boolean())],vec![]));
            rows.push(("reload_depth_charges",vec![("weapon",text()),("allocations",allocations)],vec![]));
            let mut bands=array(integer(0,None));bands["minItems"]=json!(2);bands["maxItems"]=json!(2);
            rows.push(("depth_charge_depth_report",vec![("attack",text()),("bands",bands)],vec![]));
            rows.push(("depth_charge_select_depth",vec![("attack",text()),("depth_band",integer(0,None))],vec![]));
            rows.push(("air_depth_charge_pattern",vec![("contact",contact()),("weapon",text()),("charges",integer(1,Some(65535))),("depth_band",optional(integer(0,None)))],vec!["depth_band"]));
            let mut air_bands=array(integer(0,None));air_bands["minItems"]=json!(3);air_bands["maxItems"]=json!(3);
            rows.push(("air_depth_charge_depth_report",vec![("attack",text()),("bands",air_bands)],vec![]));
            rows.push(("air_depth_charge_select_depth",vec![("attack",text()),("depth_band",integer(0,None))],vec![]));
        }
        "search_and_rescue" => {
            rows.push(("begin",vec![("group",text()),("automatic",boolean())],vec![]));
            for name in ["manual","automatic","recover_due","cancel"] {rows.push((name,vec![],vec![]));}
        }
        "communications" => {
            rows.push(("send_visual_signal",vec![("board",choice(&["port","starboard"])),("medium",choice(&["flag_hoist","flashing_light"])),("purpose",choice(&["formation","maneuvering","administrative"])),("text",text()),("receivers",array(unit()))],vec![]));
            rows.push(("send_warning",vec![("contact",contact()),("sensor",text()),("fit",text()),("receivers",array(unit()))],vec![]));
            rows.push(("send_air_solution",vec![("contact",contact()),("sensor",text()),("fit",text()),("receivers",array(unit()))],vec![]));
            rows.push(("set_automatic",vec![("fit",text()),("enabled",boolean())],vec![]));
            rows.push(("abandon_alert",vec![("contact",contact())],vec![]));
            rows.push(("send_solution",vec![("contact",contact()),("sensor",text()),("fit",text()),("receivers",array(unit())),("weapon",text()),("speed_index",optional(integer(0,None)))],vec!["speed_index"]));
            let reference=object(vec![("sender",unit()),("id",integer(1,None))],&[]);
            rows.push(("combine_reports",vec![("contact",contact()),("own_sensor",optional(text())),("messages",array(reference.clone())),("aim_source",optional(reference))],vec!["own_sensor","aim_source"]));
        }
        "unmanned" => {
            rows.push(("launch_uuv",vec![("launcher",text())],vec![]));
            rows.push(("recover_uuv",vec![("launcher",text()),("vehicle",unit())],vec![]));
            let link=choice(&["line_of_sight","satellite","acoustic_modem"]);
            rows.push(("switch_mode",vec![("mode",choice(&["hitl","autonomous"]))],vec![]));
            rows.push(("take_control",vec![("controller",unit()),("element",text()),("link",link.clone())],vec![]));
            rows.push(("assign_autonomous",vec![("controller",unit()),("element",text()),("link",link.clone())],vec![]));
            rows.push(("set_link",vec![("link",link.clone())],vec![]));
            rows.push(("plot_mission",vec![("waypoints",array(point())),("speed_kts",optional(nonnegative()))],vec!["speed_kts"]));
            rows.push(("request_radio",vec![("vehicle",unit()),("link",link)],vec![]));
            rows.push(("exchange_basic_data",vec![("contact",optional(contact())),("sensor",optional(text()))],vec!["contact","sensor"]));
        }
        "nuclear" => {
            rows.push(("set_yield",vec![("weapon",text()),("yield_kt",integer(1,None))],vec![]));
            rows.push(("authorize",vec![("weapon",text()),("enabled",boolean())],vec![]));
        }
        "battery" => {
            for name in ["start_charging","stop_charging","raise_snorkel","retract_masts"] {rows.push((name,vec![],vec![]));}
            rows.push(("set_aip_operating",vec![("enabled",boolean())],vec![]));
            rows.push(("set_power_mode",vec![("mode",choice(&["diesel","battery"]))],vec![]));
        }
        "antiship" => {
            rows.push((
                "plot",
                vec![
                    ("contact", contact()),
                    ("weapon", text()),
                    ("course_deg", number()),
                    ("speed_kts", nonnegative()),
                    ("enable_run_nm", nonnegative()),
                    ("waypoints", array(point())),
                    ("popup", boolean()),
                ],
                vec![],
            ));
        }
        "navigation" => {
            rows.push(("ram",vec![("contact",contact()),("speed_kts",optional(nonnegative()))],vec!["speed_kts"]));
            rows.push(("cancel_ram",vec![],vec![]));
            rows.push(("battery",vec![("action",action_schema("battery"))],vec![]));
            rows.push(("underway", vec![], vec![]));
            rows.push(("emergency_blow", vec![("depth_m",nonnegative())], vec![]));
            for name in ["astern", "crash_back", "coasting", "quiet", "heave_to"] {
                rows.push((name, vec![("enabled", boolean())], vec![]));
            }
            rows.push((
                "hard_rudder",
                vec![
                    ("course_deg", number()),
                    ("direction", choice(&["port", "starboard"])),
                ],
                vec![],
            ));
            rows.push(("rudder_recovery", vec![("automatic", boolean())], vec![]));
            rows.push(("free_rudder", vec![], vec![]));
        }
        "aviation" => {
            rows = vec![
                ("water_takeoff",vec![],vec![]),
                ("water_land",vec![],vec![]),
                ("respot", vec![], vec![]),
                (
                    "alert",
                    vec![("class", text()), ("count", integer(0, Some(255)))],
                    vec![],
                ),
                (
                    "throttle",
                    vec![(
                        "setting",
                        choice(&["Cruise", "Fmp", "Reheat", "Loiter", "Hover"]),
                    )],
                    vec![],
                ),
                (
                    "refuel",
                    vec![
                        ("receiver", unit()),
                        (
                            "method",
                            choice(&[
                                "Boom",
                                "WesternProbe",
                                "SovietProbe",
                                "ZA",
                                "BuddyWestern",
                                "BuddySoviet",
                            ]),
                        ),
                    ],
                    vec![],
                ),
                ("hifr", vec![("ship", unit())], vec![]),
                (
                    "jettison",
                    vec![("weapon", text()), ("rounds", integer(0, Some(65535)))],
                    vec![],
                ),
            ]
        }
        "air_combat" => {
            rows.push(("strafe_attack",vec![("contact",contact()),("delivery",choice(&["Strafe"])),("aimpoint",text())],vec![]));
            rows.push(("cluster_attack",vec![("contact",contact()),("weapon",text()),("rounds",integer(1,Some(65535))),("delivery",choice(&["Glide","Level","Lob","Strafe"])),("pattern",text()),("coverage",text())],vec![]));
            rows.push(("rocket_attack",vec![("contact",contact()),("weapon",text()),("rounds",integer(1,Some(65535))),("aimpoint",text())],vec![]));
            rows.push(("guided_surface_attack",vec![("weapon",text()),("rounds",integer(1,Some(65535))),("briefing",text())],vec![]));
            for name in ["declare", "target"] {
                rows.push((name, vec![("contact", contact())], vec![]));
            }
            for name in ["attack", "defend", "escape", "non_maneuvering", "cancel_surface_attack"] {
                rows.push((name, vec![], vec![]));
            }
            rows.push((
                "surface_attack",
                vec![
                    ("contact", contact()),
                    ("weapon", text()),
                    ("rounds", integer(0, Some(65535))),
                    ("delivery", choice(&["Glide", "Level", "Lob", "Strafe"])),
                    ("aimpoint", text()),
                ],
                vec![],
            ));
        }
        "detection" => {
            rows.push(("drop_sonobuoy",vec![("model",text()),("depth_m",nonnegative()),("radio_channel",integer(0,Some(65535)))],vec![]));
            rows.push(("listen_sonobuoys",vec![("buoys",json!({"type":"array","items":unit(),"uniqueItems":true})),("purpose",choice(&["active","search","localization"]))],vec!["purpose"]));
            rows.push(("laser",vec![("action",json!({"oneOf":[variant("type","emission",vec![("enabled",boolean())],&[])]}))],vec![]));
            rows.push(("wide_mad",vec![("field",text()),("enabled",boolean())],vec![]));
            rows.push(("soks",vec![("enabled",boolean())],vec![]));
            rows.push(("ultra_quiet", vec![("enabled", boolean())], vec![]));
            rows.push(("masts", vec![("count", integer(0, Some(255)))], vec![]));
            for name in ["tow", "dip"] {
                rows.push((
                    name,
                    vec![
                        ("sensor", text()),
                        ("deploy", boolean()),
                        ("depth_m", nonnegative()),
                    ],
                    vec![],
                ));
            }
        }
        "electronic_warfare" => {
            rows = vec![
                (
                    "deploy",
                    vec![
                        ("fit", text()),
                        ("at", optional(point())),
                        ("radar_signature", optional(integer(0, None))),
                        ("course_deg", optional(number())),
                        ("speed_kts", optional(nonnegative())),
                        ("depth_m", optional(nonnegative())),
                    ],
                    vec![
                        "at",
                        "radar_signature",
                        "course_deg",
                        "speed_kts",
                        "depth_m",
                    ],
                ),
                ("sink", vec![("object", unit())], vec![]),
                ("mobile_speed",vec![("object",unit()),("speed_kts",nonnegative())],vec![]),
                ("floating_tow",vec![("object",unit()),("geometry",text())],vec![]),
                ("floating_untow",vec![("object",unit())],vec![]),
                ("start_chaff_barrier",vec![("fit",text())],vec![]),
                ("stop_chaff_barrier",vec![],vec![]),
                (
                    "mobile_course",
                    vec![("object", unit()), ("course_deg", number())],
                    vec![],
                ),
            ]
        }
        "torpedo" => {
            rows = vec![
                (
                    "plot",
                    vec![
                        ("contact", contact()),
                        ("weapon", text()),
                        ("speed_index", integer(0, None)),
                        ("depth_m", nonnegative()),
                        ("enable_run_nm", nonnegative()),
                        ("bearing_deg", optional(number())),
                        ("spread_deg", array(number())),
                        ("cut_wire", boolean()),
                    ],
                    vec![],
                ),
                (
                    "reload",
                    vec![("tube", integer(0, None)), ("weapon", text())],
                    vec![],
                ),
                ("cut_wire", vec![("torpedo", unit())], vec![]),
                (
                    "wire_order",
                    vec![
                        ("torpedo", unit()),
                        ("speed_index", integer(0, None)),
                        ("depth_m", nonnegative()),
                        ("bearing_deg", number()),
                        ("enable_run_nm", nonnegative()),
                    ],
                    vec![],
                ),
                (
                    "hardkill",
                    vec![("contact", contact()), ("system", text())],
                    vec![],
                ),
            ]
        }
        "mines" => {
            rows.push(("lay", vec![("plan_id", text())], vec![]));
            rows.push(("hunt", vec![("contact_id", text())], vec![]));
            rows.push((
                "stream",
                vec![("gear", array(text())), ("partner", optional(unit()))],
                vec!["partner"],
            ));
            rows.push(("turn", vec![("course_deg", number())], vec![]));
            for name in ["cancel_lay", "cancel_hunt", "finish_pass", "jettison"] {
                rows.push((name, vec![], vec![]));
            }
        }
        "ground" => {
            let mut methods = vec![choice(&["Ship", "Map", "Blind"])];
            for (name, field) in [
                ("Aerial", "aircraft"),
                ("Ground", "observer"),
                ("Beacon", "observer"),
            ] {
                methods.push(object(
                    vec![(name, object(vec![(field, unit())], &[]))],
                    &[],
                ));
            }
            rows.push(("repair_runway", vec![("runway", unit())], vec![]));
            rows.push(("pump_dock", vec![("service", text()), ("flooded", boolean())], vec![]));
            rows.push(("gci_vector", vec![("service", text()), ("aircraft", unit()), ("course_deg", number())], vec![]));
            rows.push(("alongside_resupply", vec![("service", text()), ("rounds", integer(1, Some(65535)))], vec![]));
            rows.push(("cancel_alongside_resupply", vec![("service", text())], vec![]));
            rows.push((
                "spot",
                vec![("contact", contact()), ("method", json!({"oneOf":methods}))],
                vec![],
            ));
        }
        _ => unreachable!("unregistered command action schema"),
    }
    actions(rows)
}
pub fn command_schema() -> Value {
    let mut rows: Vec<Value> = Vec::new();
    let mut add = |name: &str, mut fields: Vec<(&str, Value)>, optional_fields: &[&str]| {
        fields.insert(0, ("unit", unit()));
        rows.push(variant("cmd", name, fields, optional_fields));
    };
    add("set_unit_automation", vec![("enabled", boolean())], &[]);
    add("set_course", vec![("course_deg", number())], &[]);
    add("set_speed", vec![("speed_kts", nonnegative())], &[]);
    add("set_depth", vec![("depth_m", nonnegative())], &[]);
    add("set_altitude", vec![("altitude_m", nonnegative())], &[]);
    add("set_throttle", vec![("setting", text())], &[]);
    add("communications",vec![("action",action_schema("communications"))],&[]);
    add("unmanned",vec![("action",action_schema("unmanned"))],&[]);
    add("nuclear",vec![("action",action_schema("nuclear"))],&[]);
    add("search_and_rescue",vec![("action",action_schema("search_and_rescue"))],&[]);
    add("asw",vec![("action",action_schema("asw"))],&[]);
    for name in [
        "navigation",
        "antiship",
        "aviation",
        "air_combat",
        "detection",
        "electronic_warfare",
        "mines",
        "ground",
        "torpedo",
    ] {
        add(name, vec![("action", action_schema(name))], &[]);
    }
    add(
        "set_array",
        vec![
            ("sensor", text()),
            ("deploy", boolean()),
            ("depth_m", nonnegative()),
        ],
        &[],
    );
    add(
        "aam_hoj",
        vec![("weapon", text()), ("enabled", boolean())],
        &[],
    );
    add(
        "damage_control",
        vec![(
            "action",
            choice(&[
                "flood_magazines",
                "purge_aviation",
                "reinforce_dc",
                "begin_repairs",
                "repairs_auto",
                "repairs_manual",
                "repair_due",
            ]),
        )],
        &[],
    );
    add(
        "set_torpedo_run",
        vec![("weapon", text()), ("deep", boolean())],
        &[],
    );
    add("goto", vec![("waypoints", array(point()))], &[]);
    for name in [
        "stop",
        "clear_station",
        "clear_target",
        "deploy_chaff",
        "recover_aircraft",
        "return_to_base",
    ] {
        add(name, vec![], &[]);
    }
    add(
        "follow",
        vec![
            ("leader", unit()),
            ("bearing_rel_deg", number()),
            ("distance_nm", nonnegative()),
        ],
        &[],
    );
    for name in ["set_sonar", "set_radar"] {
        add(name, vec![("active", boolean())], &[]);
    }
    add(
        "set_jamming",
        vec![("mode", text()), ("contacts", array(contact()))],
        &["contacts"],
    );
    add(
        "set_engagement_policy",
        vec![
            ("air_only", boolean()),
            ("max_in_flight", integer(0, Some(255))),
        ],
        &[],
    );
    add(
        "retask_aircraft",
        vec![("mission", mission_schema()), ("manual", boolean())],
        &["manual"],
    );
    add("set_weapons_free", vec![("free", boolean())], &[]);
    add("set_auto_chaff", vec![("on", boolean())], &[]);
    add("assign_target", vec![("contact", contact())], &[]);
    add(
        "fire",
        vec![
            ("contact", contact()),
            ("weapon", text()),
            ("rounds", integer(1, Some(255))),
        ],
        &[],
    );
    let engagement = object(
        vec![
            ("air_only", boolean()),
            ("max_in_flight", integer(0, Some(255))),
        ],
        &[],
    );
    let profile = object(
        vec![
            ("waypoints", array(point())),
            ("altitude_m", number()),
            ("weapons_free", boolean()),
            ("manual_strike", boolean()),
            ("engagement", optional(engagement)),
        ],
        &["waypoints", "manual_strike", "engagement"],
    );
    add(
        "launch_aircraft",
        vec![
            ("class", text()),
            ("count", integer(1, Some(255))),
            ("mission", mission_schema()),
            ("profile", optional(profile)),
        ],
        &["profile"],
    );
    let names: Vec<Value> = rows
        .iter()
        .map(|r| r["properties"]["cmd"]["const"].clone())
        .collect();
    json!({"type":"object","description":"Own-unit orders. Source configuration, ammunition, clocks and contact legality are checked by the engine; transport discovery does not imply a rule is complete. set_unit_automation.enabled=false reserves a unit for player control; true releases it.","properties":{"cmd":{"type":"string","enum":names}},"required":["cmd"],"oneOf":rows})
}
