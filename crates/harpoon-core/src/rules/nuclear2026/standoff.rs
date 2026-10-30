//! §8.5.2 primary-target placement effects. Ordinary blast radii are used only
//! for other units around the actual original scatter Ground Zero.
use super::*;
use crate::rules::{asw_weapons2026 as asw, expanded_damage2026 as expanded};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag="effect",rename_all="snake_case")]
pub enum AswOutcome {Kill,Major,NoEffect,Pending {reason:String}}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Primary {
    target:EntityId,
    identity:String,
    d10:u8,
    chance:i32,
    outcome:AswOutcome,
}
/// The printed sentences have gaps/overlaps at some low modified values.
/// Execute only one uniquely applicable source branch, with no priority guess.
pub fn standoff_outcome(chance:i32,d10:u8)->Result<AswOutcome,String> {
    if !(1..=10).contains(&d10) {return Err("8.5.2 original Placement die must be D10".into());}
    let d=i64::from(d10);let chance=i64::from(chance);
    let kill=d<=chance;
    let major=d>chance&&d<=chance+3;
    let no_effect=d>2*chance;
    Ok(match (kill,major,no_effect) {
        (true,false,false)=>AswOutcome::Kill,
        (false,true,false)=>AswOutcome::Major,
        (false,false,true)=>AswOutcome::NoEffect,
        _=>AswOutcome::Pending {reason:"English2026 §8.5.2 placement+3 and twice-placement clauses overlap or leave this original die uncovered".into()},
    })
}
pub fn validate_standoff_nuclear(w:&World,shooter:EntityId,weapon:&str,yield_kt:u32)->Result<(),String> {
    if !configured(w,shooter,weapon) {return Err("8.5.2 exact nuclear payload/release snapshot fit absent".into());}
    release_gate(w,shooter,weapon)?;
    let fit=&w.rules.nuclear.as_ref().unwrap().fits[&shooter][weapon];
    if fit.medium!=Medium::Subsurface||fit.selected_yield_kt!=yield_kt {
        return Err("8.5.2 nuclear payload yield/medium differs from the selected actual warhead".into());
    }
    Ok(())
}
pub fn accept_standoff_nuclear(w:&mut World,projectile:EntityId)->Result<(),String> {
    if !active(w) {return Err("8.5.2 nuclear Full runtime absent".into());}
    let key=format!("14.1.8:asw:{}",projectile.0);
    if w.rules.nuclear.as_ref().unwrap().bursts.contains_key(&key) {return Ok(());}
    let flight=w.rules.asw_weapons.as_ref().and_then(|s|s.flights.get(&projectile)).cloned()
        .ok_or("8.5.2 no actual registered standoff Flight")?;
    let asw::Payload::Nuclear{yield_kt}=flight.fit.payload else {return Err("8.5.2 registered Flight is not nuclear".into());};
    let p=flight.placement.as_ref().filter(|p|p.tick==w.tick)
        .ok_or("8.5.2 original current placement evidence absent")?;
    let ground_zero=flight.payload_position.ok_or("8.5.2 original physical Ground Zero absent")?;
    if !flight.terminal {return Err("8.5.2 carrier has not reached its real terminal phase".into());}
    let body=w.entity(projectile).filter(|e|e.alive).ok_or("8.5.2 actual live carrier body absent")?;
    let payload=body.projectile.as_ref().ok_or("8.5.2 actual carrier delivery metadata absent")?;
    if body.pos!=ground_zero||payload.weapon!=flight.weapon||payload.launched_by!=flight.shooter
        ||payload.launched_by_side!=flight.side||payload.target!=Some(flight.target) {
        return Err("8.5.2 published placement does not match actual carrier/body/source identity".into());
    }
    let original=w.rules.nuclear.as_ref().unwrap().flights.get(&projectile)
        .ok_or("8.5.2 immutable release-time nuclear snapshot absent")?;
    if original.terminal||original.weapon!=flight.weapon||original.shooter!=flight.shooter
        ||original.side!=flight.side||original.warhead.medium!=Medium::Subsurface
        ||original.warhead.selected_yield_kt!=yield_kt {
        return Err("8.5.2 registered source Flight conflicts with its original nuclear snapshot".into());
    }
    observe_flight(w,projectile);
    let original=w.rules.nuclear.as_ref().unwrap().flights[&projectile].clone();
    if !original.armed {return Err("8.5.2 depth bomb is not source-qualified armed at release".into());}
    let outcome=standoff_outcome(p.chance,p.d10)?;
    let primary=Primary {target:flight.target,identity:p.identity.clone(),d10:p.d10,chance:p.chance,outcome:outcome.clone()};
    let burst=Burst {id:key.clone(),at:w.tick,pos:ground_zero,medium:Medium::Subsurface,altitude_m:0.,
        yield_kt,side:flight.side,shooter:flight.shooter,weapon:flight.weapon,
        cause:"8.5.2 actual carrier placement".into(),primary_asw:Some(primary),surface_sam_hit:None};
    let state=w.rules.nuclear.as_mut().unwrap();
    state.flights.get_mut(&projectile).unwrap().terminal=true;
    state.bursts.insert(key,burst);
    trace(w,flight.shooter,"8.5.2.nuclear_placement",json!({"projectile":projectile,"placement":p,
        "ground_zero":ground_zero,"miss_scatter":flight.miss_scatter,"primary_effect":outcome,
        "source":"English2026 PDF99/8-19; original placement and scatter, not ordinary radius substitution"}));
    Ok(())
}
pub(super) fn resolve_primary(w:&mut World,burst:&Burst)->Option<EntityId> {
    let primary=burst.primary_asw.as_ref()?;
    if !w.entity(primary.target).is_some_and(|t|t.alive&&t.kind==PlatformKind::Sub) {return Some(primary.target);}
    trace(w,primary.target,"8.5.2.primary_nuclear_effect",json!({"burst":burst.id,"placement_identity":primary.identity,
        "d10":primary.d10,"chance":primary.chance,"effect":primary.outcome}));
    match &primary.outcome {
        AswOutcome::Kill=>{
            expanded::record_catastrophic_loss(w,primary.target,&burst.id);
            crate::damage::sink_by(w,primary.target,burst.side);
        },
        AswOutcome::Major=>{
            let count=die(w,&format!("{}:major_count",burst.id),6);
            trace(w,primary.target,"8.5.2.nuclear_major_count",json!({"burst":burst.id,"d6":count,"column":"sub_major"}));
            for n in 0..count {
                if !w.entity(primary.target).is_some_and(|t|t.alive) {break;}
                let identity=format!("{}:major:{n}",burst.id);
                let d100=die(w,&format!("{identity}:column"),100);
                let Some((kind,_))=expanded::row("sub_major",d100) else {
                    w.rules.nuclear.as_mut().unwrap().pending.insert(identity,"14.5 major damage source column missing".into());
                    continue;
                };
                trace(w,primary.target,"8.5.2.nuclear_major_critical",json!({"identity":identity,"column":"sub_major","d100":d100,"kind":kind}));
                if let Err(reason)=expanded::source_critical(w,primary.target,&identity,&kind,expanded::CriticalContext {
                    source:"English2026 §8.5.2: original nuclear placement gives D6 major damage criticals".into(),
                    dp:0.,penetrated:true,underwater:true,airburst:false,weapon:Some(burst.weapon.clone())
                }) {w.rules.nuclear.as_mut().unwrap().pending.insert(identity,reason);}
            }
            if w.entity(primary.target).is_some_and(|t|t.alive&&t.hp<=0.) {crate::damage::sink_by(w,primary.target,burst.side);}
        },
        AswOutcome::NoEffect=>{},
        AswOutcome::Pending{reason}=>{w.rules.nuclear.as_mut().unwrap().pending.insert(format!("{}:primary",burst.id),reason.clone());},
    }
    Some(primary.target)
}
