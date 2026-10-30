//! Real HTTP queue and AI batch boundaries for the source Emergency Blow.
//! Hull/water geometry is explicitly authored; no real Virginia fit is claimed.
use super::*;
use harpoon_core::EntityId;
use serde_json::Value;

const ACTOR: EntityId = EntityId(1);

fn fixture() -> Game {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json"))
        .expect("explicit consolidated database");
    let mut hull = db.platforms["us_virginia"].clone();
    hull.id = "scenario_assumption_emergency_protocol_hull".into();
    hull.name = "Authored emergency protocol hull".into();
    hull.max_depth_m = 500.;
    hull.max_speed_kts = 34.;
    hull.weapons.clear();
    hull.sensors.clear();
    db.platforms.insert(hull.id.clone(), hull);
    let sc: Scenario = serde_json::from_value(json!({
        "id":"emergency-protocol", "name":"Authored source emergency protocol geometry",
        "seed":20261024, "duration_s":5000,
        "context":{"rules_profile":"hv2026-rts-v4",
            "radar_environment":{"sea_state":0,"precipitation":"None"},
            "navigation_2026":{"source":"scenario_assumption: protocol mechanism fixture","units":{
                "OWN":{"source":"scenario_assumption: explicit own movement fit","size":"C",
                    "merchant":false,"passenger_liner":false,"cpp":false,"waterjet":false,
                    "undamaged_max_kts":34,"propulsion":"Nuclear","readiness":"Underway",
                    "submarine":{"source":"scenario_assumption: controlled hull and water",
                        "maximum_depth_m":500,"surface_max_kts":12,"water_depth_m":1000,"very_small":false}}
            }}},
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "units":[
            {"side":"Blue","class":"scenario_assumption_emergency_protocol_hull","name":"OWN",
                "pos":[0,0],"depth_m":300,"speed_kts":20,"fire_on":false},
            {"side":"Red","class":"us_ford","name":"REMOTE","pos":[1000,0],"fire_on":false}
        ]
    })).unwrap();
    let mut w = load_scenario(sc, db).expect("valid explicit source mechanism fixture");
    for e in w.entities.values_mut() {
        e.sensors.clear();
        e.fire_on = false;
    }
    assert!(harpoon_core::rules::navigation2026::submarine::configured_depth(&w, ACTOR));
    assert_eq!(w.entity(ACTOR).unwrap().depth_m, 300.);
    let mut g = Game::new(w.db.clone());
    for slot in &mut g.tactical {
        slot.enabled = false;
    }
    g.recording = Some(persistence::Recording::new(&w));
    g.world = Some(w);
    g
}

fn blow(unit: EntityId, target: f64) -> Value {
    json!({"cmd":"navigation","unit":unit,"action":{"type":"emergency_blow","depth_m":target}})
}

fn apply_queued(g: &mut Game) {
    let commands = std::mem::take(&mut g.inbox);
    g.step(&commands);
}

struct HttpFixture {
    state: AppState,
    endpoint: String,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for HttpFixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
async fn http_fixture(game: Game) -> HttpFixture {
    let db = game.world.as_ref().unwrap().db.clone();
    let state = AppState {
        session_revision: Arc::new(AtomicU64::new(0)),
        advance_plans: Arc::new(Mutex::new(persistence::AdvancePlans::default())),
        game: Arc::new(Mutex::new(game)),
        scenario_dir: Arc::new(PathBuf::from("scenarios")),
        update_tx: Arc::new(tokio::sync::broadcast::channel(8).0),
        db: Arc::new(db),
        agents: Arc::new(agent::Runtime::new()),
        director: Arc::new(Mutex::new(director::State::default())),
        director_seq: Arc::new(AtomicU64::new(0)),
    };
    let app = Router::new()
        .route("/api/command", post(command_handler))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/api/command", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    HttpFixture {
        state,
        endpoint,
        server,
    }
}
async fn request(h: &HttpFixture, side: &str, command: Value) -> (u16, String) {
    let endpoint = h.endpoint.clone();
    let body = json!({"side":side,"command":command,"session_revision":0});
    tokio::task::spawn_blocking(move || match ureq::post(&endpoint).send_json(body) {
        Ok(response) => (response.status(), response.into_string().unwrap()),
        Err(ureq::Error::Status(code, response)) => (code, response.into_string().unwrap()),
        Err(error) => panic!("real HTTP request failed before its command handler: {error}"),
    })
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emergency_http_invalid_blow_is_refused_before_queue_and_manual_takeover() {
    let h = http_fixture(fixture()).await;
    for target in [0., 25., 100., 200., 300., 400.] {
        let before =
            serde_json::to_value(h.state.game.lock().unwrap().world.as_ref().unwrap()).unwrap();
        let (status, body) = request(&h, "Blue", blow(ACTOR, target)).await;
        assert_eq!(
            status, 400,
            "invalid target {target} must fail before any reservation; {body}"
        );
        let g = h.state.game.lock().unwrap();
        assert!(
            g.inbox.is_empty(),
            "neither implicit manual command nor rejected Blow may be queued"
        );
        let w = g.world.as_ref().unwrap();
        assert_eq!(before, serde_json::to_value(w).unwrap());
        assert!(!observe(w, Side::Blue, 0, 0).units[0].manual_control);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emergency_http_valid_blow_reserves_once_and_human_can_continue_commanding() {
    let h = http_fixture(fixture()).await;
    let (status, body) = request(&h, "Blue", blow(ACTOR, 50.)).await;
    assert_eq!(
        status, 200,
        "legal own source Blow must reach the queue: {body}"
    );
    {
        let mut g = h.state.game.lock().unwrap();
        assert_eq!(
            g.inbox.len(),
            2,
            "one implicit takeover and one actual action"
        );
        assert_eq!(g.inbox.iter().filter(|(_, c)| matches!(c, Command::SetUnitAutomation { unit, enabled:false } if *unit == ACTOR)).count(), 1);
        apply_queued(&mut g);
        let w = g.world.as_ref().unwrap();
        let own = observe(w, Side::Blue, 0, 100);
        assert!(own.units[0].manual_control);
        assert_eq!(
            own.units[0]
                .navigation
                .as_ref()
                .unwrap()
                .source_rules
                .as_ref()
                .unwrap()["depth_operation"]["kind"],
            "EmergencyBlow"
        );
        assert!(w.entity(ACTOR).unwrap().depth_m < 300.);
        assert!(
            !w.over,
            "the protocol fixture must remain in play after the first command"
        );
    }
    let (status, body) = request(
        &h,
        "Blue",
        json!({"cmd":"set_course","unit":ACTOR,"course_deg":40}),
    )
    .await;
    assert_eq!(
        status, 200,
        "a human retains authority over their reserved unit: {body}"
    );
    let mut g = h.state.game.lock().unwrap();
    assert_eq!(
        g.inbox.len(),
        1,
        "an already manual unit needs no second takeover command"
    );
    apply_queued(&mut g);
    assert!(observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0).units[0].manual_control);
    assert!(g
        .recording
        .as_ref()
        .unwrap()
        .verify(g.world.as_ref().unwrap())
        .is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emergency_http_foreign_actor_errors_do_not_reveal_secret_hull_or_depth() {
    let a = http_fixture(fixture()).await;
    let mut other = fixture();
    {
        let w = other.world.as_mut().unwrap();
        let e = w.entity_mut(ACTOR).unwrap();
        e.name = "PRIVATE_CHANGED_HULL".into();
        e.depth_m = 100.;
        e.speed_kts = 5.;
    }
    let b = http_fixture(other).await;
    let first = request(&a, "Red", blow(ACTOR, 50.)).await;
    let second = request(&b, "Red", blow(ACTOR, 50.)).await;
    assert_eq!(first.0, 400);
    assert_eq!(
        first, second,
        "foreign ownership must be checked before hidden source state"
    );
    assert!(!first.1.contains("PRIVATE_CHANGED_HULL"));
    for h in [&a, &b] {
        let g = h.state.game.lock().unwrap();
        assert!(g.inbox.is_empty());
        assert!(!observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0).units[0].manual_control);
    }
}

#[test]
fn emergency_ai_batch_rejects_invalid_source_action_without_new_commands_or_task() {
    let mut g = fixture();
    let obs = observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0);
    let invalid: Command = serde_json::from_value(blow(ACTOR, 100.)).unwrap();
    let (candidates, mut rejected) = agent::validate_commands(&obs, Side::Blue, &[invalid]);
    let (accepted, errors) = g.supervise_ai(Side::Blue, candidates);
    rejected.extend(errors);
    assert!(
        accepted.is_empty(),
        "source-invalid Blow must not survive the actual AI batch preflight"
    );
    assert!(!rejected.is_empty());
    assert!(g.inbox.is_empty());
    assert!(g
        .ai_supervision
        .as_ref()
        .is_none_or(|policies| !policies[0].tasks.contains_key(&ACTOR)));
    assert!(!observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0).units[0].manual_control);

    let legal: Command = serde_json::from_value(blow(ACTOR, 50.)).unwrap();
    let obs = observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0);
    let (candidates, rejected) = agent::validate_commands(&obs, Side::Blue, &[legal.clone()]);
    assert!(
        rejected.is_empty(),
        "a legal explicit proposal is not an autonomous decision policy"
    );
    let (accepted, rejected) = g.supervise_ai(Side::Blue, candidates);
    assert!(rejected.is_empty(), "{rejected:?}");
    assert_eq!(accepted.len(), 1);

    g.world
        .as_mut()
        .unwrap()
        .entity_mut(ACTOR)
        .unwrap()
        .ext
        .insert("control.manual".into(), 1.);
    let obs = observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0);
    let (accepted, rejected) = agent::validate_commands(&obs, Side::Blue, &[legal]);
    assert!(
        accepted.is_empty(),
        "new navigation actions cannot bypass the existing AI manual gate"
    );
    assert!(!rejected.is_empty());
}

fn start_real_ai_blow_without_manual_takeover(g: &mut Game) {
    let obs = observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0);
    let legal: Command = serde_json::from_value(blow(ACTOR, 50.)).unwrap();
    let (candidates, rejected) = agent::validate_commands(&obs, Side::Blue, &[legal]);
    assert!(rejected.is_empty(), "legal AI fixture: {rejected:?}");
    let (accepted, rejected) = g.supervise_ai(Side::Blue, candidates);
    assert!(rejected.is_empty(), "legal AI fixture: {rejected:?}");
    assert_eq!(accepted.len(), 1);
    g.inbox
        .extend(accepted.into_iter().map(|c| (Side::Blue, c)));
    apply_queued(g);
    let own = observe(g.world.as_ref().unwrap(), Side::Blue, 0, 100);
    assert!(
        !own.units[0].manual_control,
        "an AI action does not reserve the unit for the human"
    );
    assert_eq!(
        own.units[0]
            .navigation
            .as_ref()
            .unwrap()
            .source_rules
            .as_ref()
            .unwrap()["depth_operation"]["kind"],
        "EmergencyBlow",
        "the protocol negative case must start from a real core operation"
    );
    assert!(own.units[0].depth_m < 300. && own.units[0].depth_m > 200.);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emergency_http_set_depth_cannot_reserve_control_when_it_breaks_a_live_three_band_commitment(
) {
    let mut g = fixture();
    start_real_ai_blow_without_manual_takeover(&mut g);
    let current = g.world.as_ref().unwrap().entity(ACTOR).unwrap().depth_m;
    let h = http_fixture(g).await;
    let before =
        serde_json::to_value(h.state.game.lock().unwrap().world.as_ref().unwrap()).unwrap();
    let (status, body) = request(
        &h,
        "Blue",
        json!({"cmd":"set_depth","unit":ACTOR,"depth_m":current}),
    )
    .await;
    assert_eq!(status, 400, "core will reject this premature cancel, so the public queue must reject it before taking manual control: {body}");
    let g = h.state.game.lock().unwrap();
    assert!(g.inbox.is_empty());
    let w = g.world.as_ref().unwrap();
    assert_eq!(before, serde_json::to_value(w).unwrap());
    assert!(!observe(w, Side::Blue, 0, 0).units[0].manual_control);
}

#[test]
fn emergency_ai_set_depth_cannot_survive_preflight_when_it_breaks_a_live_commitment() {
    let mut g = fixture();
    start_real_ai_blow_without_manual_takeover(&mut g);
    let current = g.world.as_ref().unwrap().entity(ACTOR).unwrap().depth_m;
    let before = serde_json::to_value(g.world.as_ref().unwrap()).unwrap();
    let obs = observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0);
    let (candidates, mut rejected) = agent::validate_commands(
        &obs,
        Side::Blue,
        &[Command::SetDepth {
            unit: ACTOR,
            depth_m: current,
        }],
    );
    let (accepted, errors) = g.supervise_ai(Side::Blue, candidates);
    rejected.extend(errors);
    assert!(
        accepted.is_empty(),
        "AI cannot queue a normal depth order which core rejects for cancelling the active Blow"
    );
    assert!(!rejected.is_empty());
    assert!(g.inbox.is_empty());
    assert!(g
        .ai_supervision
        .as_ref()
        .is_none_or(|policies| !policies[0].tasks.contains_key(&ACTOR)));
    let w = g.world.as_ref().unwrap();
    assert_eq!(
        before["entities"],
        serde_json::to_value(w).unwrap()["entities"],
        "AI review may emit notes, but cannot change the actual submarine or its control state"
    );
    assert_eq!(
        before["rules"]["navigation"],
        serde_json::to_value(w).unwrap()["rules"]["navigation"],
        "the rejected AI cancellation cannot reset source operation progress"
    );
    assert!(!observe(w, Side::Blue, 0, 0).units[0].manual_control);
}

#[test]
fn emergency_fsm_observes_real_auto_blow_holds_all_actor_commands_and_preserves_world_continuation()
{
    let mut g = fixture();
    let mut staff =
        tactical::TacticalStaff::with_db(Side::Blue, g.world.as_ref().unwrap().db.clone());
    let normal = observe(g.world.as_ref().unwrap(), Side::Blue, 0, 100);
    let normal_orders = staff.decide(&normal);
    assert!(normal_orders.iter().any(|c| c.unit_id() == ACTOR), "the control observation is an active ordinary source submarine, not an over/empty/manual fixture");
    assert!(
        !normal_orders.iter().any(|c| matches!(
            c,
            Command::Navigation {
                action: harpoon_core::rules::navigation2026::Action::EmergencyBlow { .. },
                ..
            }
        )),
        "making the source action discoverable does not authorize autonomous initiation"
    );
    staff.take_notes();

    // Standing orders are installed through the actual command stream. A
    // source hold must not quietly replace fire permission or the plotted route.
    g.inbox.push((
        Side::Blue,
        Command::SetWeaponsFree {
            unit: ACTOR,
            free: true,
        },
    ));
    g.inbox.push((
        Side::Blue,
        Command::Goto {
            unit: ACTOR,
            waypoints: vec![harpoon_core::geom::Vec2::new(10., 10.)],
        },
    ));
    apply_queued(&mut g);
    start_real_ai_blow_without_manual_takeover(&mut g);
    let w = g.world.as_ref().unwrap();
    let obs = observe(w, Side::Blue, 0, 100);
    assert!(!obs.over);
    assert!(!obs.units[0].manual_control);
    assert!(obs.units[0].fire_on);
    assert_eq!(obs.units[0].navigation.as_ref().unwrap().waypoints.len(), 1);
    let op = obs.units[0]
        .navigation
        .as_ref()
        .unwrap()
        .source_rules
        .as_ref()
        .unwrap()["depth_operation"]
        .clone();
    assert_eq!(op["kind"], "EmergencyBlow");
    assert_eq!(op["target_m"], 50.);
    let from_depth = w.entity(ACTOR).unwrap().depth_m;
    let assigned = w.entity(ACTOR).unwrap().assigned;

    let orders = staff.decide(&obs);
    assert!(orders.iter().all(|c| c.unit_id() != ACTOR),
        "an actual ongoing own Blow must hold all actor output, including the earlier global engagement-policy loop: {}",
        serde_json::to_string(&orders).unwrap());
    let notes = staff.take_notes();
    let hold = notes
        .iter()
        .find(|n| n.unit == ACTOR && n.action == "source_emergency_blow_hold")
        .expect("state recognition must produce a distinct structured source hold note");
    assert!(!hold.zh.is_empty() && !hold.en.is_empty());
    assert!(
        hold.en.contains(&format!("T{}", obs.tick + 30)),
        "the hold must name its next controller review; this is policy, not a source depth rate"
    );

    // The critical proof is actual output and full physical continuation, not
    // a string saying that the staff is holding. The same seed/source world
    // with no replacement command must continue identically for thirty seconds.
    let mut actual = w.clone();
    let mut empty = w.clone();
    let first: Vec<_> = orders.into_iter().map(|c| (Side::Blue, c)).collect();
    for tick in 0..30 {
        harpoon_core::sim::tick(&mut actual, if tick == 0 { &first } else { &[] });
        harpoon_core::sim::tick(&mut empty, &[]);
    }
    assert_eq!(
        serde_json::to_value(&actual).unwrap(),
        serde_json::to_value(&empty).unwrap()
    );
    assert!(actual.entity(ACTOR).unwrap().depth_m < from_depth);
    assert!(actual.entity(ACTOR).unwrap().fire_on);
    assert_eq!(actual.entity(ACTOR).unwrap().assigned, assigned);
    assert_eq!(actual.entity(ACTOR).unwrap().waypoints.len(), 1);
    let after = observe(&actual, Side::Blue, 0, 100);
    let next = &after.units[0]
        .navigation
        .as_ref()
        .unwrap()
        .source_rules
        .as_ref()
        .unwrap()["depth_operation"];
    assert_eq!(next["kind"], "EmergencyBlow");
    assert_eq!(next["target_m"], 50.);
    assert!(next["worked_seconds"].as_f64().unwrap() > op["worked_seconds"].as_f64().unwrap());
}

#[test]
fn emergency_fsm_respects_real_manual_takeover_without_cancelling_the_existing_blow() {
    let mut g = fixture();
    start_real_ai_blow_without_manual_takeover(&mut g);
    g.inbox.push((
        Side::Blue,
        Command::SetUnitAutomation {
            unit: ACTOR,
            enabled: false,
        },
    ));
    apply_queued(&mut g);
    let w = g.world.as_ref().unwrap();
    let obs = observe(w, Side::Blue, 0, 100);
    assert!(!obs.over);
    assert!(obs.units[0].manual_control);
    let op = obs.units[0]
        .navigation
        .as_ref()
        .unwrap()
        .source_rules
        .as_ref()
        .unwrap()["depth_operation"]
        .clone();
    assert_eq!(op["kind"], "EmergencyBlow");
    let mut staff = tactical::TacticalStaff::with_db(Side::Blue, w.db.clone());
    let orders = staff.decide(&obs);
    assert!(
        orders.iter().all(|c| c.unit_id() != ACTOR),
        "the normal manual gate also covers the new ongoing source state"
    );
    let mut actual = w.clone();
    let mut empty = w.clone();
    let first: Vec<_> = orders.into_iter().map(|c| (Side::Blue, c)).collect();
    for tick in 0..30 {
        harpoon_core::sim::tick(&mut actual, if tick == 0 { &first } else { &[] });
        harpoon_core::sim::tick(&mut empty, &[]);
    }
    assert_eq!(
        serde_json::to_value(&actual).unwrap(),
        serde_json::to_value(&empty).unwrap()
    );
    let own = observe(&actual, Side::Blue, 0, 100);
    assert!(own.units[0].manual_control);
    let next = &own.units[0]
        .navigation
        .as_ref()
        .unwrap()
        .source_rules
        .as_ref()
        .unwrap()["depth_operation"];
    assert_eq!(next["target_m"], op["target_m"]);
    assert!(next["worked_seconds"].as_f64().unwrap() > op["worked_seconds"].as_f64().unwrap());
}
