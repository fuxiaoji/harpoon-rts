//! Independent JSONL research bridge to the complete production naval core.
//! No combat, detection, damage, database, victory or FSM rule is replaced here.
//! Run from a frozen harpoon-rts tree. Normal replies contain only player-side
//! observations. dump/load/replay are referee operations, never model input.
#[allow(dead_code)]
#[path = "../src/ai.rs"]
mod ai;
#[allow(dead_code)]
#[path = "../src/tactical.rs"]
mod tactical;

use ai::{CommandStaff, StaffNote};
use harpoon_core::{commands::AirMission, observe::UnitObs, Command, Database, EntityId, Event,
    Observation, Side, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::{BTreeMap, BTreeSet}, io::{self, BufRead, Write}};

const PROTOCOL: &str = "full-naval-realtime-v1";
const MAX_COMMANDS: usize = 16;
const MAX_ADVANCE: i64 = 3600;
const INTERNAL_EVENTS: usize = 4096;

#[derive(Clone, Serialize, Deserialize)]
struct RuntimeState {
    world: World,
    /// Production Entity serialization omits this death/cleanup clock. It is
    /// authoritative for future evolution, so research checkpoints carry every
    /// entity's value explicitly and include it in runtime equality/hashing.
    last_emit_ticks: BTreeMap<EntityId,i64>,
    staffs: [tactical::TacticalStaff; 2],
    player_supervisor: tactical::supervision::Supervisor,
    script_staff: tactical::TacticalStaff,
}

impl RuntimeState {
    fn sync_runtime_fields(&mut self) {
        self.last_emit_ticks=self.world.entities.iter().map(|(id,e)|(*id,e.last_emit_tick)).collect();
    }
    fn attach_database(&mut self) -> Result<(),String> {
        if self.last_emit_ticks.len()!=self.world.entities.len() ||
            !self.world.entities.keys().all(|id|self.last_emit_ticks.contains_key(id)) {
            return Err("checkpoint last_emit_ticks does not exactly cover World entities".into());
        }
        for (id,e) in &mut self.world.entities {e.last_emit_tick=self.last_emit_ticks[id];}
        for staff in &mut self.staffs { staff.restore_database(self.world.db.clone()); }
        self.script_staff.restore_database(self.world.db.clone());
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct StepRecord {
    tick_before: i64,
    decision: bool,
    submitted: Vec<Command>,
    player_orders: Vec<Command>,
    maintenance_orders: Vec<Command>,
    fsm_orders: Vec<(Side, Command)>,
    player_notes: Vec<StaffNote>,
    maintenance_notes: Vec<StaffNote>,
    fsm_notes: Vec<(Side, Vec<StaffNote>)>,
    applied: Vec<(Side, Command)>,
}

#[derive(Clone, Serialize, Deserialize)]
struct ScriptRecord { tick: i64, commands: Vec<Command>, notes: Vec<StaffNote> }

#[derive(Clone, Serialize, Deserialize)]
struct Session {
    protocol: String,
    player_side: Side,
    fsm_self: bool,
    allow_script_plan: bool,
    task_lease_s: i64,
    initial: RuntimeState,
    state: RuntimeState,
    steps: Vec<StepRecord>,
    script_calls: Vec<ScriptRecord>,
    all_events: Vec<Event>,
}

fn fnv64<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serializable research state");
    format!("{:016x}", bytes.iter().fold(0xcbf29ce484222325u64,
        |h,b| (h ^ *b as u64).wrapping_mul(0x100000001b3)))
}

fn equal<T: Serialize>(left: &T, right: &T) -> bool {
    serde_json::to_value(left).unwrap() == serde_json::to_value(right).unwrap()
}

fn load_database() -> Result<Database, String> {
    let mut db = Database::built_in();
    let mut paths = std::fs::read_dir("database/volumes").map_err(|e| e.to_string())?
        .map(|entry| entry.map(|e|e.path())).collect::<Result<Vec<_>,_>>()
        .map_err(|e|e.to_string())?;
    paths.retain(|p|p.extension().is_some_and(|s|s=="json") &&
        !p.file_name().unwrap().to_string_lossy().starts_with('_'));
    paths.sort();
    for path in paths {
        let volume = Database::from_json(&std::fs::read_to_string(path).map_err(|e|e.to_string())?)
            .map_err(|e|e.to_string())?;
        db = db.merged(&volume);
    }
    Ok(db)
}

impl Session {
    fn from_world(mut world: World, side: Side, fsm_self: bool, allow_script_plan: bool) -> Self {
        world.log_commands = true;
        let db = world.db.clone();
        let state = RuntimeState {
            last_emit_ticks:world.entities.iter().map(|(id,e)|(*id,e.last_emit_tick)).collect(),
            world,
            staffs: [tactical::TacticalStaff::with_db(Side::Blue,db.clone()),
                tactical::TacticalStaff::with_db(Side::Red,db.clone())],
            player_supervisor: Default::default(),
            script_staff: tactical::TacticalStaff::with_db(side,db),
        };
        let all_events = state.world.events.iter().cloned().collect();
        Self { protocol: PROTOCOL.into(), player_side: side, fsm_self, allow_script_plan,task_lease_s:600,
            initial: state.clone(), state, steps:vec![], script_calls:vec![], all_events }
    }

    fn init(request: &Value) -> Result<Self,String> {
        let path = request["scenario_path"].as_str().ok_or("scenario_path is required")?;
        let mut scenario = harpoon_core::scenario_from_json(
            &std::fs::read_to_string(path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        if let Some(seed) = request.get("seed") {scenario.seed=seed.as_u64().ok_or("seed must be u64")?;}
        let side=Side::parse(request["player_side"].as_str().unwrap_or("blue"))
            .ok_or("player_side must be blue or red")?;
        if let Some(profile)=request.get("rules_profile") {
            scenario.context.rules_profile=Some(serde_json::from_value(profile.clone()).map_err(|e|e.to_string())?);
        }
        let world = harpoon_core::load_scenario(scenario,load_database()?).map_err(|e|e.to_string())?;
        let mut session=Self::from_world(world,side,request["fsm_self"].as_bool().unwrap_or(false),
            request["allow_script_plan"].as_bool().unwrap_or(false));
        session.task_lease_s=request.get("task_lease_s").map(|v|v.as_i64().ok_or("task_lease_s must be integer"))
            .transpose()?.unwrap_or(600);
        if !(30..=7200).contains(&session.task_lease_s) {return Err("task_lease_s must be 30..7200".into());}
        Ok(session)
    }

    /// A decision flag means a newly delivered valid model decision (including
    /// an explicit hold), not the absence of a response while the world ticks.
    fn step(&mut self, submitted: Vec<Command>, decision: bool) -> Result<(),String> {
        if self.state.world.over {return Err("game is over".into());}
        let tick = self.state.world.tick;
        let side = self.player_side;
        let db = self.state.world.db.clone();
        let since = self.state.world.event_seq;
        // Both observations are collected before either controller emits notes
        // or orders. No side can observe the other controller's current intent.
        let observations = [harpoon_core::observe(&self.state.world,Side::Blue,0,INTERNAL_EVENTS),
            harpoon_core::observe(&self.state.world,Side::Red,0,INTERNAL_EVENTS)];
        let obs = &observations[side.index()];
        let mut player_notes = vec![];
        let mut player_orders = vec![];
        if decision {
            let supervisor=&mut self.state.player_supervisor;
            player_notes.extend(supervisor.observe(obs,&db));
            player_notes.extend(supervisor.review_current(obs,&db));
            let (orders,notes)=supervisor.guard(obs,&db,submitted.clone());
            player_orders=orders; player_notes.extend(notes);
            // Research executor factor only: extend tasks genuinely reviewed
            // now. This never revives an invalid track/empty/low-reserve task.
            for task in supervisor.tasks.values_mut().filter(|t|t.reviewed_at==tick) {
                task.expires_at=tick+self.task_lease_s;
            }
        } else if !submitted.is_empty() {return Err("commands require decision:true".into());}
        let mut maintenance_orders=vec![];
        let mut maintenance_notes=vec![];
        if !self.fsm_self && tick%30==0 {
            let supervisor=&mut self.state.player_supervisor;
            maintenance_notes.extend(supervisor.observe(obs,&db));
            let (orders,notes)=supervisor.maintenance(obs,&db);
            maintenance_orders=orders; maintenance_notes.extend(notes);
        }
        let mut fsm_orders=vec![];
        let mut fsm_notes=vec![];
        if tick%30==0 {
            for fsm_side in [Side::Blue,Side::Red] {
                if fsm_side==side && !self.fsm_self {continue;}
                let staff=&mut self.state.staffs[fsm_side.index()];
                let orders=staff.decide(&observations[fsm_side.index()]);
                fsm_orders.extend(orders.into_iter().map(|c|(fsm_side,c)));
                fsm_notes.push((fsm_side,staff.take_notes()));
            }
        }
        let mut applied:Vec<_>=player_orders.iter().chain(&maintenance_orders)
            .cloned().map(|c|(side,c)).collect();
        applied.extend(fsm_orders.clone());
        let event_cap=self.state.world.event_cap;
        // Capture every event, even if one dense combat tick exceeds the normal
        // World tail. Then restore exactly the original bounded-tail semantics.
        self.state.world.event_cap=usize::MAX;
        tactical::record_notes(&mut self.state.world,side,player_notes.clone());
        tactical::record_notes(&mut self.state.world,side,maintenance_notes.clone());
        for (owner,notes) in &fsm_notes {
            tactical::record_notes(&mut self.state.world,*owner,notes.clone());
        }
        harpoon_core::sim::tick(&mut self.state.world,&applied);
        let new_events:Vec<_>=self.state.world.events.iter().filter(|e|e.seq>since).cloned().collect();
        self.all_events.extend(new_events);
        self.state.world.event_cap=event_cap;
        let excess=self.state.world.events.len().saturating_sub(event_cap);
        self.state.world.events.drain(0..excess);
        self.state.sync_runtime_fields();
        self.steps.push(StepRecord { tick_before:tick, decision, submitted, player_orders,
            maintenance_orders,fsm_orders,player_notes,maintenance_notes,fsm_notes,applied });
        Ok(())
    }

    fn script_plan(&mut self) -> Result<Value,String> {
        if !self.allow_script_plan {return Err("script_plan is disabled for real-model sessions".into());}
        if self.state.world.over {return Err("game is over".into());}
        let obs=harpoon_core::observe(&self.state.world,self.player_side,0,INTERNAL_EVENTS);
        let mut commands=self.state.script_staff.decide(&obs);
        commands.truncate(MAX_COMMANDS);
        let notes=self.state.script_staff.take_notes();
        let record=ScriptRecord{tick:self.state.world.tick,commands,notes};
        let output=json!({"ok":true,"protocol":PROTOCOL,"diagnostic_only":true,
            "controller":"production TacticalStaff (not LLM)","tick":record.tick,
            "commands":record.commands,"notes":record.notes});
        self.script_calls.push(record);
        Ok(output)
    }

    fn public_view(&self, since_seq:u64, event_limit:usize, include_db:bool) -> Value {
        let obs=harpoon_core::observe(&self.state.world,self.player_side,since_seq,event_limit);
        let summary=self.state.player_supervisor.summary(&obs);
        let options=tactical::assessment::brief(&obs,&self.state.world.db,&summary);
        let mut out=json!({"ok":true,"protocol":PROTOCOL,"observation":obs,
            "supervision":summary,"action_options":options,
            "controller_contract":{"opponent":"production TacticalStaff","cadence_s":30,
                "self_fsm":self.fsm_self,"script_plan_enabled":self.allow_script_plan,
                "standing_task_review_s":self.task_lease_s,"native_standing_task_review_s":600,
                "maintenance_s":30,"max_commands":MAX_COMMANDS,
                "valid_hold_requires_decision_true":true,"orders_apply_at":"next simulation tick",
                "core_self_defence_and_existing_tasks_continue":true}});
        if include_db {out["public_db"]=public_database(&obs,&self.state.world.db);}
        out
    }

    fn advance(&mut self, request:&Value) -> Result<Value,String> {
        let seconds=request.get("seconds").map(|v|v.as_i64().ok_or("seconds must be integer"))
            .transpose()?.unwrap_or(1);
        if !(1..=MAX_ADVANCE).contains(&seconds) {return Err("seconds must be 1..3600".into());}
        let raw=request.get("commands").map(|v|v.as_array().ok_or("commands must be an array"))
            .transpose()?.cloned().unwrap_or_default();
        if raw.len()>MAX_COMMANDS {return Err("at most 16 commands per decision".into());}
        let decision=request["decision"].as_bool().unwrap_or(!raw.is_empty());
        if !decision && !raw.is_empty() {return Err("commands require decision:true".into());}
        if self.fsm_self && (decision || !raw.is_empty()) {
            return Err("fsm_self diagnostic cannot mix external decisions".into());
        }
        let obs=harpoon_core::observe(&self.state.world,self.player_side,0,INTERNAL_EVENTS);
        let mut accepted=vec![];
        let mut rejected=vec![];
        for (index,value) in raw.iter().enumerate() {
            let parsed=serde_json::from_value::<Command>(value.clone()).map_err(|e|e.to_string());
            match parsed.and_then(|c|validate_command(&obs,self.player_side,&c).map(|_|c)) {
                Ok(command)=>accepted.push(command),
                Err(reason)=>rejected.push(json!({"index":index,"reason":reason,"command":value})),
            }
        }
        let start=self.state.world.tick;
        let first=self.steps.len();
        for i in 0..seconds {
            if self.state.world.over {break;}
            self.step(if i==0 {accepted.clone()} else {vec![]},decision && i==0)?;
        }
        let mut out=self.public_view(request["since_seq"].as_u64().unwrap_or(0),
            request["event_limit"].as_u64().unwrap_or(256).min(4096) as usize,
            request["include_db"].as_bool().unwrap_or(false));
        // Only the player's validation and executor decisions are public.
        out["feedback"]=json!({"tick_before":start,"advanced":self.state.world.tick-start,
            "accepted_by_validation":accepted,"rejected":rejected,
            "player_guarded_orders":self.steps.get(first).map(|s|&s.player_orders),
            "player_guard_notes":self.steps.get(first).map(|s|&s.player_notes),
            "maintenance_orders":self.steps[first..].iter().map(|s|json!({"tick_before":s.tick_before,
                "commands":s.maintenance_orders,"notes":s.maintenance_notes})).filter(|v|
                    !v["commands"].as_array().unwrap().is_empty() || !v["notes"].as_array().unwrap().is_empty()).collect::<Vec<_>>()});
        Ok(out)
    }

    fn validate(&self,request:&Value)->Result<Value,String> {
        let raw=request.get("commands").and_then(Value::as_array).ok_or("commands must be an array")?;
        if raw.len()>MAX_COMMANDS {return Err("at most 16 commands per decision".into());}
        let obs=harpoon_core::observe(&self.state.world,self.player_side,0,INTERNAL_EVENTS);
        let mut accepted=vec![];let mut rejected=vec![];
        for (index,value) in raw.iter().enumerate() {
            match serde_json::from_value::<Command>(value.clone()).map_err(|e|e.to_string())
                .and_then(|c|validate_command(&obs,self.player_side,&c).map(|_|c)) {
                Ok(command)=>accepted.push(command),
                Err(reason)=>rejected.push(json!({"index":index,"reason":reason,"command":value})),
            }
        }
        Ok(json!({"ok":true,"protocol":PROTOCOL,"tick":obs.tick,
            "decision_valid":!self.state.world.over && (raw.is_empty() || !accepted.is_empty()),
            "accepted":accepted,"rejected":rejected,"validation_only":true}))
    }

    fn restored(mut self) -> Result<Self,String> {
        if self.protocol!=PROTOCOL {return Err("checkpoint protocol mismatch".into());}
        self.initial.attach_database()?; self.state.attach_database()?;
        Ok(self)
    }

    fn replay(&self) -> Result<Value,String> {
        let mut rerun=Self {protocol:self.protocol.clone(),player_side:self.player_side,
            fsm_self:self.fsm_self,allow_script_plan:self.allow_script_plan,
            task_lease_s:self.task_lease_s,
            initial:self.initial.clone(),state:self.initial.clone(),steps:vec![],script_calls:vec![],
            all_events:self.initial.world.events.iter().cloned().collect()};
        rerun.state.attach_database()?;
        let mut script_index=0;
        for (index,step) in self.steps.iter().enumerate() {
            while script_index<self.script_calls.len() && self.script_calls[script_index].tick==rerun.state.world.tick {
                rerun.script_plan()?;
                if !equal(&rerun.script_calls[script_index],&self.script_calls[script_index]) {
                    return Err(format!("script plan mismatch at {script_index}"));
                }
                script_index+=1;
            }
            if rerun.state.world.tick!=step.tick_before {return Err(format!("tick mismatch at step {index}"));}
            rerun.step(step.submitted.clone(),step.decision)?;
            if !equal(rerun.steps.last().unwrap(),step) {return Err(format!("command/note mismatch at step {index}"));}
        }
        while script_index<self.script_calls.len() && self.script_calls[script_index].tick==rerun.state.world.tick {
            rerun.script_plan()?;
            if !equal(&rerun.script_calls[script_index],&self.script_calls[script_index]) {
                return Err(format!("script plan mismatch at {script_index}"));
            }
            script_index+=1;
        }
        if script_index!=self.script_calls.len() {return Err("script log outside replay timeline".into());}
        if !equal(&rerun.state,&self.state) {return Err("full World/controller/supervisor mismatch".into());}
        if !equal(&rerun.all_events,&self.all_events) {return Err("complete event log mismatch".into());}
        Ok(json!({"ok":true,"protocol":PROTOCOL,"referee_only":true,"passed":true,
            "tick":self.state.world.tick,"steps":self.steps.len(),"script_calls":self.script_calls.len(),
            "events":self.all_events.len(),"world_fnv64":fnv64(&self.state.world),
            "runtime_fnv64":fnv64(&self.state),"result":self.state.world.result,
            "world_and_both_fsm_and_player_supervisor_equal":true,
            "serde_skipped_entity_clocks_equal":true}))
    }
}

/// This copies only public class definitions referenced by currently observed
/// friendly units and their ready/cycling deck fits. No hidden enemy inventory.
fn public_database(obs:&Observation,db:&Database)->Value {
    let mut classes:BTreeSet<_>=obs.units.iter().map(|u|u.class.clone()).collect();
    for u in &obs.units {if let Some(deck)=&u.deck {
        classes.extend(deck.ready.iter().map(|(class,_)|class.clone()));
        classes.extend(deck.cycling.iter().map(|(class,_,_)|class.clone()));
    }}
    let platforms:BTreeMap<_,_>=classes.iter().filter_map(|id|db.platforms.get(id).map(|p|(id,p))).collect();
    let mut weapons:BTreeSet<_>=obs.units.iter().flat_map(|u|u.weapons.iter().map(|w|w.weapon.clone())).collect();
    let mut sensors=BTreeSet::new();
    for p in platforms.values() {
        weapons.extend(p.weapons.iter().map(|w|w.weapon.clone()));
        sensors.extend(p.sensors.iter().cloned());
    }
    let weapons:BTreeMap<_,_>=weapons.iter().filter_map(|id|db.weapons.get(id).map(|w|(id,w))).collect();
    let sensors:BTreeMap<_,_>=sensors.iter().filter_map(|id|db.sensors.get(id).map(|s|(id,s))).collect();
    json!({"basis":"public data of own observed classes/deck fits only","platforms":platforms,
        "weapons":weapons,"sensors":sensors})
}

fn checkpoint(request:&Value)->Result<Session,String> {
    let value=if let Some(path)=request["checkpoint_path"].as_str() {
        serde_json::from_str::<Value>(&std::fs::read_to_string(path).map_err(|e|e.to_string())?)
            .map_err(|e|e.to_string())?
    } else {request.get("checkpoint").cloned().ok_or("checkpoint or checkpoint_path is required")?};
    let state=value.get("checkpoint").unwrap_or(&value).clone();
    serde_json::from_value::<Session>(state).map_err(|e|e.to_string())?.restored()
}

fn dispatch(current:&mut Option<Session>,request:&Value)->Result<Value,String> {
    match request["op"].as_str().ok_or("op is required")? {
        "init"=>{let s=Session::init(request)?;let view=s.public_view(0,256,true);*current=Some(s);Ok(view)},
        "load"=>{let s=checkpoint(request)?;let view=s.public_view(0,256,true);*current=Some(s);Ok(view)},
        "replay"=>{if request.get("checkpoint").is_some() || request.get("checkpoint_path").is_some() {
            checkpoint(request)?.replay()
        } else {current.as_ref().ok_or("init or load first")?.replay()}},
        "advance"=>current.as_mut().ok_or("init or load first")?.advance(request),
        "validate"=>current.as_ref().ok_or("init or load first")?.validate(request),
        "script_plan"=>current.as_mut().ok_or("init or load first")?.script_plan(),
        "observe"=>Ok(current.as_ref().ok_or("init or load first")?.public_view(
            request["since_seq"].as_u64().unwrap_or(0),request["event_limit"].as_u64().unwrap_or(256).min(4096) as usize,
            request["include_db"].as_bool().unwrap_or(false))),
        "dump"=>{
            let s=current.as_ref().ok_or("init or load first")?;
            let hash=fnv64(s);
            if let Some(path)=request["path"].as_str() {
                let file=std::fs::File::create(path).map_err(|e|e.to_string())?;
                serde_json::to_writer(io::BufWriter::new(file),s).map_err(|e|e.to_string())?;
                Ok(json!({"ok":true,"protocol":PROTOCOL,"referee_only":true,"path":path,
                    "checkpoint_fnv64":hash,"world_fnv64":fnv64(&s.state.world),
                    "tick":s.state.world.tick,"steps":s.steps.len(),"events":s.all_events.len(),
                    "rules_profile":s.state.world.rules.profile,"result":s.state.world.result}))
            } else {Ok(json!({"ok":true,"protocol":PROTOCOL,"referee_only":true,
                "checkpoint_fnv64":hash,"checkpoint":s}))}
        },
        other=>Err(format!("unknown op {other}")),
    }
}

// Snapshot of production agent.rs::validate_command. No hidden World fields
// are consulted; physics/cooldowns/ranges remain checked by the full core.
fn validate_command(obs: &Observation, side: Side, command: &Command) -> Result<(), String> {
    if obs.exercise.as_ref().is_some_and(|e|e.locked) {return Err("exercise alert lock: active orders are unavailable".into());}
    let own = |id: EntityId| -> Result<&UnitObs, String> {
        let u = obs
            .units
            .iter()
            .find(|u| u.id == id && u.side == side)
            .ok_or_else(|| format!("单位 #{} 不属于本方或已不在态势中", id.0))?;
        Ok(u)
    };
    let contact = |id: harpoon_core::types::ContactId| -> Result<(), String> {
        if obs.contacts.iter().any(|c| c.id == id) {
            Ok(())
        } else {
            Err(format!("接触 #{} 不在本方态势中", id.0))
        }
    };
    let finite = |v: f64, name: &str| -> Result<(), String> {
        if v.is_finite() {
            Ok(())
        } else {
            Err(format!("{name} 必须是有限数值"))
        }
    };
    match command {
        // The research protocol freezes v2; later rule-version commands are
        // excluded without altering its action set or experimental results.
        Command::Asw{..}|Command::Aviation{..}|Command::AirCombat{..}|Command::SetArray{..}|Command::Mines{..}|Command::Detection{..}|Command::ElectronicWarfare{..}|Command::Torpedo{..}|Command::Navigation{..}|Command::AamHoj{..}|Command::Ground{..}|Command::Antiship{..}|Command::Communications{..}|Command::Unmanned{..}|Command::Nuclear{..}|Command::SearchAndRescue{..}=>return Err("v4命令不属于本实验冻结动作集".into()),
        Command::SetUnitAutomation{..}=>{return Err("model cannot change higher-level control ownership".into());}
        Command::SetCourse { unit, course_deg } => {
            own(*unit)?;
            finite(*course_deg, "course_deg")?;
        }
        Command::SetSpeed { unit, speed_kts } => {
            own(*unit)?;
            finite(*speed_kts, "speed_kts")?;
            if !(0.0..=2000.0).contains(speed_kts) {
                return Err("speed_kts 必须在 0..2000".into());
            }
        }
        Command::SetDepth { unit, depth_m } => {
            own(*unit)?;
            finite(*depth_m, "depth_m")?;
            if !(0.0..=1000.0).contains(depth_m) {
                return Err("depth_m 必须在 0..1000；0表示浮航".into());
            }
        }
        Command::DamageControl{unit,action}=>{
            let u=own(*unit)?;
            if !matches!(u.kind,harpoon_core::types::PlatformKind::Ship|harpoon_core::types::PlatformKind::Sub){return Err("损管需要本方舰船或潜艇".into());}
            if !matches!(action.as_str(),"flood_magazines"|"purge_aviation"|"reinforce_dc"){return Err("未知损管行动".into());}
            if action=="purge_aviation"&&u.deck.is_none(){return Err("航空库惰化需要航空舰船".into());}
        }
        Command::SetTorpedoRun{unit,weapon,..}=>{
            let u=own(*unit)?;if !u.weapons.iter().any(|m|m.weapon==*weapon){return Err("该单位没有此鱼雷".into());}
        }
        Command::SetAltitude { unit, altitude_m } => {
            own(*unit)?;
            finite(*altitude_m, "altitude_m")?;
            if !(15.0..=30000.0).contains(altitude_m) {
                return Err("altitude_m 必须在 15..30000".into());
            }
        }
        Command::Goto { unit, waypoints } => {
            own(*unit)?;
            if waypoints.is_empty() || waypoints.len() > 24 {
                return Err("waypoints 数量必须在 1..24".into());
            }
            for p in waypoints {
                finite(p.x, "waypoint.x")?;
                finite(p.y, "waypoint.y")?;
            }
        }
        Command::Stop { unit } => {
            own(*unit)?;
        }
        Command::Follow { unit, leader, bearing_rel_deg, distance_nm } => {
            own(*unit)?;
            own(*leader)?;
            if unit == leader {
                return Err("不能跟随自己".into());
            }
            finite(*bearing_rel_deg, "bearing_rel_deg")?;
            finite(*distance_nm, "distance_nm")?;
            if !(0.0..=100.0).contains(distance_nm) {
                return Err("distance_nm 必须在 0..100".into());
            }
        }
        Command::ClearStation { unit } => {
            own(*unit)?;
        }
        Command::SetSonar { unit, .. } => {
            own(*unit)?;
        }
        Command::SetRadar { unit, .. } => { own(*unit)?; }
        Command::SetThrottle {unit,setting} => {let u=own(*unit)?;if !harpoon_core::rules::flight_profiles::supported(&u.class)||!["cruise","fmp","reheat","loiter"].contains(&setting.as_str()){return Err("机型没有已核验的油门数据或油门模式无效".into());}}
        Command::SetJamming {unit,mode,contacts} => {
            let u=own(*unit)?;let cap=harpoon_core::rules::electronic_warfare::capability(&u.class).ok_or("没有已核验的进攻干扰挂载")?;
            if !["off","standoff","escort","spot"].contains(&mode.as_str()) || mode=="standoff" && !cap.standoff {return Err("该挂载不支持请求的干扰模式".into());}
            if mode=="spot" && (contacts.is_empty()||contacts.len()>cap.spot_targets) {return Err("点干扰目标数超过挂载能力".into());}
            for c in contacts {contact(*c)?;}
        }
        Command::SetEngagementPolicy {unit,max_in_flight,..} => {own(*unit)?;if *max_in_flight>32 {return Err("自动交战在飞预算必须在0..32".into());}}
        Command::RetaskAircraft {unit, mission, ..} => {
            if own(*unit)?.kind != harpoon_core::types::PlatformKind::Aircraft {return Err("任务重设只适用于飞机".into());}
            match mission {
                AirMission::Strike {contact:c} => contact(*c)?,
                AirMission::Cap {station:p,radius_nm,..} | AirMission::Asw {area:p,radius_nm} => {
                    finite(p.x,"point.x")?; finite(p.y,"point.y")?; finite(*radius_nm,"radius_nm")?;
                    if !(0.1..=100.0).contains(radius_nm) {return Err("radius_nm 必须在0.1..100".into());}
                }
                AirMission::Recon {point:p} => {finite(p.x,"point.x")?;finite(p.y,"point.y")?;}
                AirMission::Rtb => {}
            }
        }
        Command::SetWeaponsFree { unit, .. } => {
            own(*unit)?;
        }
        Command::SetAutoChaff { unit, .. } => {
            own(*unit)?;
        }
        Command::AssignTarget { unit, contact: c } => {
            own(*unit)?;
            contact(*c)?;
        }
        Command::ClearTarget { unit } => {
            own(*unit)?;
        }
        Command::Fire { unit, contact: c, weapon, rounds } => {
            let u = own(*unit)?;
            contact(*c)?;
            let mount = u
                .weapons
                .iter()
                .find(|w| &w.weapon == weapon)
                .ok_or_else(|| format!("单位没有武器 {weapon}"))?;
            if mount.ammo == 0 {
                return Err(format!("{weapon} 弹药耗尽"));
            }
            if *rounds == 0 || u16::from(*rounds) > mount.ammo {
                return Err(format!("rounds 必须在 1..{}", mount.ammo));
            }
        }
        Command::DeployChaff { unit } => {
            let u = own(*unit)?;
            if u.chaff == 0 {
                return Err("箔条已耗尽".into());
            }
        }
        Command::LaunchAircraft { unit, class, count, mission, profile } => {
            if let Some(p)=profile {
                if p.waypoints.len()>32 {return Err("profile.waypoints 最多32点".into());}
                for point in &p.waypoints {finite(point.x,"profile.waypoints.x")?;finite(point.y,"profile.waypoints.y")?;}
                finite(p.altitude_m,"profile.altitude_m")?;
                if !(15.0..=30000.0).contains(&p.altitude_m) {return Err("起飞高度必须在15..30000米".into());}
                if p.engagement.as_ref().is_some_and(|p|p.max_in_flight>32) {return Err("profile.engagement.max_in_flight 必须在0..32".into());}
            }
            let u = own(*unit)?;
            let deck = u.deck.as_ref().ok_or_else(|| "该单位没有飞行甲板".to_string())?;
            if *count == 0 || *count > 24 {
                return Err("count 必须在 1..24".into());
            }
            let ready = deck.ready.iter().find(|(c, n)| c == class && *n > 0);
            let cycling = deck.cycling.iter().any(|(c, _, _)| c == class);
            if ready.is_none() && !cycling {
                return Err(format!("甲板上没有可用机型 {class}"));
            }
            match mission {
                AirMission::Cap { station, radius_nm, .. } => {
                    finite(station.x, "station.x")?;
                    finite(station.y, "station.y")?;
                    finite(*radius_nm, "radius_nm")?;
                }
                AirMission::Strike { contact: c } => contact(*c)?,
                AirMission::Asw { area, radius_nm } => {
                    finite(area.x, "area.x")?;
                    finite(area.y, "area.y")?;
                    finite(*radius_nm, "radius_nm")?;
                }
                AirMission::Recon { point } => {
                    finite(point.x, "point.x")?;
                    finite(point.y, "point.y")?;
                }
                AirMission::Rtb => {}
            }
        }
        Command::RecoverAircraft { unit } => {
            let u = own(*unit)?;
            if u.deck.is_none() {
                return Err("该单位没有飞行甲板".into());
            }
        }
        Command::ReturnToBase {unit} => {
            if own(*unit)?.kind!=harpoon_core::types::PlatformKind::Aircraft {return Err("只有飞机可以单机返航".into());}
        }
    }
    Ok(())
}


fn main() {
    let stdin=io::stdin();let stdout=io::stdout();let mut output=io::BufWriter::new(stdout.lock());
    let mut current=None;
    for line in stdin.lock().lines() {
        let reply=match line {
            Ok(line)=>serde_json::from_str::<Value>(&line).map_err(|e|e.to_string())
                .and_then(|request|dispatch(&mut current,&request)),
            Err(e)=>Err(e.to_string()),
        }.unwrap_or_else(|error|json!({"ok":false,"protocol":PROTOCOL,"error":error}));
        if serde_json::to_writer(&mut output,&reply).is_err() || writeln!(&mut output).is_err() || output.flush().is_err() {break;}
    }
}

#[cfg(test)]
mod realtime_tests {
    use super::*;

    fn mini(fsm_self:bool) -> Session {
        let sc=harpoon_core::scenario_from_json(&json!({
            "id":"research-bridge-test","name":"Fictional bridge test","duration_s":1200,
            "weather":{"visibility_nm":12.0,"jam":[0.0,0.0]},"seed":17,
            "context":{"rules_profile":"rts-legacy-v1"},
            "victory":{"target_vp":0},"units":[
                {"side":"Blue","class":"ddg_aegis","name":"OWN", "pos":[0,0],"course_deg":90,"speed_kts":10,"fire_on":false},
                {"side":"Red","class":"mgb","name":"HIDDEN OPPONENT", "pos":[100,100],"course_deg":270,"speed_kts":10,"fire_on":false}
            ]}).to_string()).unwrap();
        Session::from_world(harpoon_core::load_scenario(sc,Database::built_in()).unwrap(),Side::Blue,fsm_self,false)
    }

    #[test]
    fn public_projection_has_no_enemy_world_or_controller() {
        let s=mini(false);let view=s.public_view(0,256,true);
        assert_eq!(view["observation"]["your_side"],"Blue");
        assert_eq!(view["observation"]["units"].as_array().unwrap().len(),1);
        let text=view.to_string();
        assert!(!text.contains("HIDDEN OPPONENT"));
        assert!(view.get("world").is_none() && view.get("checkpoint").is_none());
        assert!(!view["public_db"]["platforms"].as_object().unwrap().contains_key("mgb"));
        assert!(view.get("world_fnv64").is_none());
    }

    #[test]
    fn validate_current_ownership_contacts_and_ammo() {
        let mut s=mini(false);
        let reply=s.advance(&json!({"seconds":1,"decision":true,"commands":[
            {"cmd":"set_speed","unit":2,"speed_kts":5},
            {"cmd":"assign_target","unit":1,"contact":999},
            {"cmd":"fire","unit":1,"contact":999,"weapon":"ascm_std","rounds":0},
            {"cmd":"set_unit_automation","unit":1,"enabled":true},
            {"cmd":"set_speed","unit":1,"speed_kts":12}
        ]})).unwrap();
        assert_eq!(reply["feedback"]["rejected"].as_array().unwrap().len(),4);
        assert_eq!(reply["feedback"]["accepted_by_validation"].as_array().unwrap().len(),1);
        assert_eq!(s.state.world.tick,1);
        let mut obs=harpoon_core::observe(&s.state.world,Side::Blue,0,0);
        // Add a deliberately public synthetic contact for validator-only ammo
        // checks; no synthetic contact is inserted into the simulation.
        let contact:harpoon_core::observe::ContactObs=serde_json::from_value(json!({
            "id":9,"category":"Ship","class_name":null,"estimate":{"x":1,"y":1},
            "est_course_deg":0,"est_speed_kts":0,"quality":1,"bearing_only":false,
            "last_seen_s":1,"stale":false,"sensors":[],"own_in_flight":0,
            "bda_hits":0,"bda_damage":0,"bda_hp_fraction":null
        })).unwrap();
        obs.contacts.push(contact);
        let mount=obs.units[0].weapons.iter().find(|w|w.ammo>0).unwrap();
        assert!(validate_command(&obs,Side::Blue,&Command::Fire {unit:EntityId(1),
            contact:harpoon_core::ContactId(9),weapon:mount.weapon.clone(),rounds:0}).is_err());
    }

    #[test]
    fn commands_apply_once_next_tick_and_wait_never_reviews() {
        let mut s=mini(false);
        let reply=s.advance(&json!({"seconds":3,"decision":true,"commands":[
            {"cmd":"set_course","unit":1,"course_deg":23}
        ]})).unwrap();
        assert_eq!(reply["feedback"]["advanced"],3);
        assert_eq!(s.steps[0].tick_before,0);
        assert!(s.steps[0].decision && !s.steps[1].decision && !s.steps[2].decision);
        let logs:Vec<_>=s.state.world.command_log.iter().filter(|l|l.side==Side::Blue &&
            matches!(l.command,Command::SetCourse{..})).collect();
        assert_eq!(logs.len(),1);assert_eq!(logs[0].tick,1);
    }

    #[test]
    fn checkpoint_restore_and_full_controller_replay() {
        let mut full=mini(true);
        full.advance(&json!({"seconds":95})).unwrap();
        assert!(full.replay().unwrap()["passed"].as_bool().unwrap());
        let mut first=mini(true);
        first.advance(&json!({"seconds":31})).unwrap();
        let mut restored:Session=serde_json::from_value(serde_json::to_value(&first).unwrap()).unwrap();
        restored=restored.restored().unwrap();
        restored.advance(&json!({"seconds":64})).unwrap();
        assert!(equal(&full.state,&restored.state));
        assert!(equal(&full.all_events,&restored.all_events));
        assert_eq!(full.steps.iter().filter(|s|!s.fsm_notes.is_empty()).count(),4);
        assert!(restored.replay().unwrap()["passed"].as_bool().unwrap());
    }

    #[test]
    fn all_events_survive_a_small_world_ring_and_tampering_fails() {
        let mut s=mini(false);s.state.world.event_cap=2;s.initial=s.state.clone();
        // Radar emission toggles create genuine core command_receipt events.
        // Several separate ticks force eviction from the bounded World tail.
        for active in [false,true,false,true,false,true] {
            s.advance(&json!({"seconds":1,"decision":true,"commands":[
                {"cmd":"set_radar","unit":1,"active":active}
            ]})).unwrap();
        }
        assert!(s.state.world.events.len()<=2);
        assert!(s.all_events.len()>=5);
        assert!(s.all_events.windows(2).all(|events|events[1].seq==events[0].seq+1));
        assert!(s.all_events.len()>s.state.world.events.len());
        assert!(s.replay().is_ok());
        s.state.world.entities.get_mut(&EntityId(1)).unwrap().hp-=1.0;
        assert!(s.replay().is_err());
    }

    #[test]
    fn no_free_fsm_plan_in_real_model_mode_and_record_diagnostic_calls() {
        let mut s=mini(false);assert!(s.script_plan().is_err());
        s.allow_script_plan=true;
        let plan=s.script_plan().unwrap();
        assert!(plan["diagnostic_only"].as_bool().unwrap());
        assert!(plan["commands"].as_array().unwrap().len()<=MAX_COMMANDS);
        s.advance(&json!({"seconds":31,"decision":true,"commands":plan["commands"]})).unwrap();
        s.script_plan().unwrap();
        assert!(s.replay().is_ok());
    }

    #[test]
    fn lease_factor_changes_expiry_not_invalid_track_rules() {
        let mut native=mini(false);
        native.state.player_supervisor.tasks.insert(EntityId(1),tactical::supervision::Task {
            contact:None,expires_at:600,reviewed_at:0,route:true,air:false,jammer:false});
        let mut longer=native.clone();longer.task_lease_s=1800;
        native.step(vec![],true).unwrap();longer.step(vec![],true).unwrap();
        assert_eq!(native.state.player_supervisor.tasks[&EntityId(1)].expires_at,600);
        assert_eq!(longer.state.player_supervisor.tasks[&EntityId(1)].expires_at,1800);
        assert!(equal(&native.state.world,&longer.state.world));
        let mut obs=harpoon_core::observe(&native.state.world,Side::Blue,0,0);obs.tick=601;
        let db=native.state.world.db.clone();
        assert!(!native.state.player_supervisor.maintenance(&obs,&db).0.is_empty());
        assert!(longer.state.player_supervisor.maintenance(&obs,&db).0.is_empty());
        longer.state.player_supervisor.tasks.get_mut(&EntityId(1)).unwrap().contact=Some(harpoon_core::ContactId(999));
        assert!(!longer.state.player_supervisor.maintenance(&obs,&db).0.is_empty());
    }

    #[test]
    fn invalid_batch_does_not_advance_and_fsm_mode_cannot_mix_model() {
        let mut s=mini(false);
        assert!(s.advance(&json!({"seconds":0})).is_err());
        assert!(s.advance(&json!({"seconds":3601})).is_err());
        let too_many=vec![json!({"cmd":"stop","unit":1});17];
        assert!(s.advance(&json!({"seconds":1,"commands":too_many})).is_err());
        assert_eq!(s.state.world.tick,0);
        let mut dual=mini(true);
        assert!(dual.advance(&json!({"seconds":1,"decision":true})).is_err());
        assert_eq!(dual.state.world.tick,0);
    }

    #[test]
    fn validation_is_read_only_and_all_rejected_is_not_hold() {
        let s=mini(false);let before=fnv64(&s);
        let hold=s.validate(&json!({"commands":[]})).unwrap();
        assert_eq!(hold["decision_valid"],true);
        let bad=s.validate(&json!({"commands":[{"cmd":"not_a_command"},
            {"cmd":"set_speed","unit":2,"speed_kts":5}]})).unwrap();
        assert_eq!(bad["decision_valid"],false);
        assert_eq!(bad["rejected"].as_array().unwrap().len(),2);
        let partial=s.validate(&json!({"commands":[{"cmd":"not_a_command"},
            {"cmd":"set_speed","unit":1,"speed_kts":5}]})).unwrap();
        assert_eq!(partial["decision_valid"],true);
        assert_eq!(partial["accepted"].as_array().unwrap().len(),1);
        assert_eq!(before,fnv64(&s));
    }

    #[test]
    fn recently_sunk_clock_survives_serialization_and_controls_cleanup_at_120s() {
        let mut world=mini(false).state.world;
        world.tick=500;
        // Keep another red ship alive so normal victory cannot stop the test
        // before the genuine core's 120-second dead-entity retention expires.
        let mut reserve=world.entities[&EntityId(2)].clone();
        reserve.id=EntityId(3);reserve.name="RED RESERVE".into();
        world.entities.insert(reserve.id,reserve);world.next_entity=4;
        harpoon_core::damage::sink(&mut world,EntityId(2));
        assert_eq!(world.entities[&EntityId(2)].last_emit_tick,500);
        let mut original=Session::from_world(world,Side::Blue,false,false);
        original.advance(&json!({"seconds":1})).unwrap();
        let parsed:Session=serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
        // Prove that the ordinary World serialization actually loses the
        // clock, and that the research checkpoint restores the real value.
        assert_eq!(parsed.state.world.entities[&EntityId(2)].last_emit_tick,0);
        let mut restored=parsed.restored().unwrap();
        assert_eq!(restored.state.world.entities[&EntityId(2)].last_emit_tick,500);
        assert!(equal(&original.state,&restored.state));
        for seconds in [1,117] {
            original.advance(&json!({"seconds":seconds})).unwrap();
            restored.advance(&json!({"seconds":seconds})).unwrap();
            assert!(original.state.world.entities.contains_key(&EntityId(2)));
            assert!(equal(&original.state,&restored.state));
        }
        assert_eq!(original.state.world.tick,619);
        original.advance(&json!({"seconds":1})).unwrap();
        restored.advance(&json!({"seconds":1})).unwrap();
        assert!(!original.state.world.entities.contains_key(&EntityId(2)));
        assert!(equal(&original.state,&restored.state));
        assert!(equal(&original.all_events,&restored.all_events));
        assert!(restored.replay().is_ok());
        let mut missing=restored.clone();missing.state.last_emit_ticks.remove(&EntityId(1));
        assert!(missing.restored().is_err());
    }
}

