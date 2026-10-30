use super::*;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct AlertState {
    grants: BTreeMap<String, Grant>,
    attempted: BTreeMap<String, i64>,
}
impl AlertState {
    pub(super) fn is_empty(&self) -> bool {
        self.grants.is_empty() && self.attempted.is_empty()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Grant {
    receiver: EntityId,
    target: EntityId,
    side: Side,
    contact: ContactId,
    eligible: i64,
    applied: Option<i64>,
    abandoned: bool,
    refs: BTreeSet<MessageRef>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SonarAttempt {
    pub observer: EntityId,
    pub target: EntityId,
    pub at: i64,
}
fn key(receiver: EntityId, target: EntityId) -> String {
    format!("{}:{}", receiver.0, target.0)
}
fn referee(w: &mut World, receiver: EntityId, detail: Value) {
    w.emit(
        EventKind::RulesEvent {
            domain: "communications2026".into(),
            rule: "6.3.9.tl_alert".into(),
            side: None,
            unit: Some(receiver),
            contact: None,
            detail: detail.to_string(),
        },
        Visibility::Referee,
    );
}
pub(super) fn delivered(w: &mut World, m: &Message) {
    if m.snapshot.apparent_category != Some(PlatformKind::Sub) || m.solution.is_some() {
        return;
    }
    let epoch = w.tick.div_euclid(180);
    let mut events = Vec::new();
    let Some(state) = w.rules.communications.as_mut() else {
        return;
    };
    for receiver in &m.receivers {
        if m.receiver_kinds.get(receiver) != Some(&LinkKind::TimeLate) {
            continue;
        }
        let k = key(*receiver, m.snapshot.report.target);
        let eligible = if state.alerts.attempted.get(&k) == Some(&epoch) {
            epoch + 1
        } else {
            epoch
        };
        let reference = MessageRef {
            sender: m.sender,
            id: m.id,
        };
        let duplicate = state.alerts.grants.contains_key(&k);
        let g = state.alerts.grants.entry(k).or_insert_with(|| Grant {
            receiver: *receiver,
            target: m.snapshot.report.target,
            side: m.side,
            contact: m.contact,
            eligible,
            applied: None,
            abandoned: false,
            refs: BTreeSet::new(),
        });
        g.refs.insert(reference);
        events.push((*receiver,json!({"state":if duplicate{"duplicate"}else{"pending"},"eligible_epoch":g.eligible,"applied_epoch":g.applied,"receiver":receiver,"target":g.target,"refs":g.refs,"source":"English2026 6.3.9 PDF74 and Acoustic FCS TDL Cue PDF72","policy":"RTS-COMMS-B1 once per receiver-target scenario; whole search epoch; ordinary OR"})));
    }
    for (receiver, detail) in events {
        referee(w, receiver, detail);
    }
}
/// Pure shared view for Direct/BB/CZ and pre-roll passive masking. The applied
/// epoch stays enabled through every cached per-second scan in that epoch.
pub fn sonar_alert(w: &World, receiver: EntityId, target: EntityId) -> bool {
    if !active(w) {
        return false;
    }
    let Some(state) = w.rules.communications.as_ref() else {
        return false;
    };
    if check_state(w, state).is_err() {
        return false;
    }
    let epoch = w.tick.div_euclid(180);
    state
        .alerts
        .grants
        .get(&key(receiver, target))
        .is_some_and(|g| {
            !g.abandoned
                && match g.applied {
                    Some(used) => used == epoch,
                    None => g.eligible <= epoch,
                }
        })
}
/// Called once after the entire native scan batch, with actual sonar trials
/// (including failed/zero-probability draws), never from readonly probability.
pub fn commit_sonar_attempts(w: &mut World, attempts: &[SonarAttempt]) {
    if !active(w) {
        return;
    }
    let epoch = w.tick.div_euclid(180);
    let Some(s) = w.rules.communications.as_ref() else {
        return;
    };
    if check_state(w, s).is_err() {
        return;
    }
    let attempts: BTreeSet<_> = attempts
        .iter()
        .filter(|a| a.at == w.tick)
        .map(|a| (a.observer, a.target))
        .collect();
    let mut events = Vec::new();
    let s = w.rules.communications.as_mut().unwrap();
    for (receiver, target) in attempts {
        let k = key(receiver, target);
        s.alerts.attempted.insert(k.clone(), epoch);
        if let Some(g) = s.alerts.grants.get_mut(&k) {
            if !g.abandoned && g.applied.is_none() && g.eligible <= epoch {
                g.applied = Some(epoch);
                events.push((receiver,json!({"state":"applied","epoch":epoch,"target":target,"refs":g.refs,"source":"6.3.9 one-time alerted sonar search","policy":"RTS-COMMS-B1; consume success/failure/zero-probability native trial; no extra die"})));
            }
        }
    }
    s.alerts.attempted.retain(|_, at| *at >= epoch - 1);
    for (receiver, detail) in events {
        referee(w, receiver, detail);
    }
}
pub(super) fn validate_abandon(
    w: &World,
    side: Side,
    actor: EntityId,
    contact: ContactId,
) -> Result<String, String> {
    exchange::validate_control(w, side, actor)?;
    let target = w.sides[side.index()]
        .contacts
        .get(&contact)
        .ok_or("6.3.9 own source contact label required")?
        .about;
    let k = key(actor, target);
    if !w
        .rules
        .communications
        .as_ref()
        .unwrap()
        .alerts
        .grants
        .contains_key(&k)
    {
        return Err("6.3.9 no received TL search opportunity".into());
    }
    Ok(k)
}
pub(super) fn abandon(
    w: &mut World,
    side: Side,
    actor: EntityId,
    contact: ContactId,
) -> Result<(), String> {
    let k = validate_abandon(w, side, actor, contact)?;
    w.rules
        .communications
        .as_mut()
        .unwrap()
        .alerts
        .grants
        .get_mut(&k)
        .unwrap()
        .abandoned = true;
    emit(
        w,
        side,
        actor,
        "6.3.9.alert_abandoned",
        json!({"contact":contact,"may_reissue":false}),
    );
    Ok(())
}
pub(super) fn own_alerts(w: &World, unit: EntityId) -> Value {
    let Some(s) = w.rules.communications.as_ref() else {
        return json!([]);
    };
    // No private target, probability, future detection result or enemy-liveness check.
    json!(s.alerts.grants.values().filter(|g|g.receiver==unit).map(|g|json!({"contact":g.contact,"contact_is_source_label":true,"state":if g.abandoned{"abandoned"}else if g.applied.is_some(){"used"}else{"pending"},"message_refs":g.refs})).collect::<Vec<_>>())
}
