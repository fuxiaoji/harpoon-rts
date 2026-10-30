const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const {batteryText,ruleReport}=require('../web/combat-help.js');
const app = fs.readFileSync(path.join(__dirname, '../web/app.js'), 'utf8');
const part = (start, end) => app.slice(app.indexOf(start), app.indexOf(end, app.indexOf(start)));
const plain = value => JSON.parse(JSON.stringify(value));

function harness() {
  const unit = {id:1,side:'Blue',name:'Own boat',weapons:[{weapon:'exact_weapon',name:'Exact weapon',ammo:2}],
    battery_state:{charge_units:null,capacity_units:100,charging:false,snorkel_fitted:true,mast_raised:false,aip_fitted:true,aip_operational:false,aip_remaining_units:40,power_mode:'battery'},
    nuclear:{warheads:[{weapon:'exact_weapon',selected_yield_kt:5,allowed_yields_kt:[5,20],authorized:false}]},
    unmanned:{mode:null,controlled:[{unit:2,name:'Known unmanned vehicle',position_not_reported:true}]},
    communications:{scope:'fitted_communications',fits:[
      {fit:'voice',send:true,operational:true,receivers:[2]},
      {fit:'rt',send:true,operational:true,receivers:[2,999],link_kind:'real_time',automatic:false}],
      reports:[{contact:7,sensor:'own_sensor',observed_at:40}],messages:[]}};
  const commands = [];
  const state = {side:'Blue',gameGeneration:1,startingScenario:false,sourceControlDrafts:new Map(),communicationsDrafts:new Map(),
    observation:{tick:500,rules_profile:'hv2026-rts-v4',over:false,units:[unit,{id:2,side:'Blue',name:'Known unmanned vehicle',unmanned:{mode:'hitl',telemetry:{position_at:40,status_at:20}}}]}};
  const sandbox = {state,console,Map,Set,Number,String,Array,JSON,
    isObject:v=>v!==null&&typeof v==='object'&&!Array.isArray(v),
    finiteOrNull:v=>v===null||v===undefined||v===''?null:Number.isFinite(Number(v))?Number(v):null,
    battleText:(zh)=>zh,fmtNum:String,unitName:u=>u.name,
    selectedUnit:()=>unit,postCommand:async command=>{commands.push(plain(command));return true;}};
  vm.createContext(sandbox);
  vm.runInContext(part('  function sourceControlDraft(', '  function warningReportKey(')
    + part('  function warningReportKey(', '  function renderCommunications(')
    + part('  function extendedCommunicationCommand(', '  function renderTargetPanel('), sandbox);
  vm.runInContext('renderSourceControls = () => {}; renderCommunications = () => {};', sandbox);
  return {unit,state,commands,api:sandbox};
}

test('unknown source values stay unknown and editing drafts sends no command', () => {
  const {unit,commands,api}=harness();
  assert.equal(api.sourceValue(null),'未知');
  assert.equal(api.sourceValue(undefined),'未知');
  assert.equal(api.sourceValue(0),'0');
  api.sourceControlDraft(unit);
  api.updateSourceControlDraft(unit,'powerMode','diesel');
  api.warningDraft(unit);
  assert.deepEqual(commands,[]);
  assert.equal(unit.battery_state.charge_units,null);
});

test('rescue requests require an installed own fit and an actually reported opaque label', async () => {
  const {unit,api,commands}=harness();
  unit.survivors=[{label:'survivors-1',position:{x:1,y:2},observed_at:40}];
  api.updateSourceControlDraft(unit,'rescueGroup','survivors-1');
  assert.equal(api.sourceControlCommand(unit,'rescue_begin'),null);
  unit.expanded_damage={rescue_fit:{kind:'ship',capacity:10},rescue:null};
  api.updateSourceControlDraft(unit,'rescueGroup','private:lost:999');
  assert.equal(api.sourceControlCommand(unit,'rescue_begin'),null);
  api.updateSourceControlDraft(unit,'rescueGroup','survivors-1');
  await api.sendSourceControl('rescue_begin');
  assert.deepEqual(commands,[{cmd:'search_and_rescue',unit:1,action:{type:'begin',group:'survivors-1',automatic:true}}]);
  assert.equal(unit.expanded_damage.rescue,null,'queueing does not fabricate an active rescue');
});

test('recovery and cancellation require a reported rescue task and use no private physics', () => {
  const {unit,api}=harness();
  unit.expanded_damage={rescue_fit:{kind:'ship',capacity:10},rescue:null};
  assert.equal(api.sourceControlCommand(unit,'rescue_recover'),null);
  assert.equal(api.sourceControlCommand(unit,'rescue_cancel'),null);
  unit.expanded_damage.rescue={group:'survivors-1',picked_up:0};
  assert.deepEqual(plain(api.sourceControlCommand(unit,'rescue_recover')),{cmd:'search_and_rescue',unit:1,action:{type:'recover_due'}});
  assert.deepEqual(plain(api.sourceControlCommand(unit,'rescue_cancel')),{cmd:'search_and_rescue',unit:1,action:{type:'cancel'}});
});

test('alongside supply uses only the reported own assignment and preserves stock while queued', async () => {
  const {unit,api,commands}=harness();
  unit.ground={berth_resupply:{services:{own:{weapon:'exact_weapon',mount_index:0,stock:3,reserved_stock:0,capacity:4,reserved_capacity:0,job:null,pending:null}}}};
  api.updateSourceControlDraft(unit,'resupplyService','other-ship-service');
  assert.equal(api.sourceControlCommand(unit,'resupply_start'),null);
  api.updateSourceControlDraft(unit,'resupplyService','own');
  api.updateSourceControlDraft(unit,'resupplyRounds','2');
  const before=plain(unit);
  await api.sendSourceControl('resupply_start');
  assert.deepEqual(commands,[{cmd:'ground',unit:1,action:{type:'alongside_resupply',service:'own',rounds:2}}]);
  assert.deepEqual(unit,before,'queueing cannot fabricate an inventory transfer or a service job');
});

test('alongside drafts reject fractional, excessive and unknown stock or capacity', () => {
  const {unit,api}=harness();
  const row={weapon:'exact_weapon',mount_index:0,stock:3,reserved_stock:1,capacity:4,reserved_capacity:1,job:null,pending:null};
  unit.ground={berth_resupply:{services:{own:row}}};
  api.updateSourceControlDraft(unit,'resupplyService','own');
  for(const value of ['0','-1','1.5','2','65536','']) {
    api.updateSourceControlDraft(unit,'resupplyRounds',value);
    assert.equal(api.sourceControlCommand(unit,'resupply_start'),null,value);
  }
  api.updateSourceControlDraft(unit,'resupplyRounds','1');
  assert.ok(api.sourceControlCommand(unit,'resupply_start'));
  for(const field of ['stock','reserved_stock','capacity','reserved_capacity']) {
    const prior=row[field];row[field]=null;
    assert.equal(api.sourceControlCommand(unit,'resupply_start'),null,field);
    row[field]=prior;
  }
  row.weapon='unassigned';assert.equal(api.sourceControlCommand(unit,'resupply_start'),null);
});

test('an actual reported supply job permits cancellation without a second start or hidden fields', () => {
  const {unit,api,state}=harness();
  unit.ground={berth_resupply:{services:{own:{weapon:'exact_weapon',mount_index:0,stock:3,reserved_stock:2,capacity:4,reserved_capacity:2,job:{rounds:2,due:506},pending:null}}}};
  api.updateSourceControlDraft(unit,'resupplyService','own');
  assert.equal(api.sourceControlCommand(unit,'resupply_start'),null);
  assert.deepEqual(plain(api.sourceControlCommand(unit,'resupply_cancel')),{cmd:'ground',unit:1,action:{type:'cancel_alongside_resupply',service:'own'}});
  state.observation.over=true;assert.equal(api.sourceControlCommand(unit,'resupply_cancel'),null);
});

test('battery controls use the shared navigation battery action schema without optimistic state changes', async () => {
  const {unit,commands,api}=harness();
  api.updateSourceControlDraft(unit,'powerMode','diesel');
  assert.deepEqual(plain(api.sourceControlCommand(unit,'power_mode')),
    {cmd:'navigation',unit:1,action:{type:'battery',action:{type:'set_power_mode',mode:'diesel'}}});
  await api.sendSourceControl('raise_snorkel');
  assert.deepEqual(commands,[{cmd:'navigation',unit:1,action:{type:'battery',action:{type:'raise_snorkel'}}}]);
  assert.equal(unit.battery_state.mast_raised,false);
  assert.equal(api.sourceControlDraft(unit).feedback,'queued');
});

test('nuclear release requires an explicit weapon and separate authorization draft', () => {
  const {unit,api,commands}=harness();
  assert.equal(api.sourceControlCommand(unit,'nuclear_authorize'),null);
  api.updateSourceControlDraft(unit,'nuclearWeapon','exact_weapon');
  api.updateSourceControlDraft(unit,'nuclearYield','20');
  assert.deepEqual(plain(api.sourceControlCommand(unit,'nuclear_yield')),
    {cmd:'nuclear',unit:1,action:{type:'set_yield',weapon:'exact_weapon',yield_kt:20}});
  assert.equal(api.sourceControlCommand(unit,'nuclear_authorize').action.enabled,false);
  api.updateSourceControlDraft(unit,'nuclearAuthorize',true);
  assert.equal(api.sourceControlCommand(unit,'nuclear_authorize').action.enabled,true);
  api.updateSourceControlDraft(unit,'nuclearWeapon','exact_weapon');
  assert.equal(api.sourceControlDraft(unit).nuclearAuthorize,false,'choosing a weapon clears earlier release intent');
  api.updateSourceControlDraft(unit,'nuclearYield','999');
  assert.equal(api.sourceControlCommand(unit,'nuclear_yield'),null);
  assert.equal(unit.nuclear.warheads[0].authorized,false);
  assert.deepEqual(commands,[]);
});

test('unmanned mode request uses the known own roster and does not advance stale telemetry', async () => {
  const {unit,state,api,commands}=harness();
  api.updateSourceControlDraft(unit,'unmannedVehicle','2');
  api.updateSourceControlDraft(unit,'unmannedMode','autonomous');
  await api.sendSourceControl('unmanned_mode');
  assert.deepEqual(commands,[{cmd:'unmanned',unit:2,action:{type:'switch_mode',mode:'autonomous'}}]);
  assert.equal(state.observation.units[1].unmanned.mode,'hitl');
  assert.deepEqual(state.observation.units[1].unmanned.telemetry,{position_at:40,status_at:20});
  api.updateSourceControlDraft(unit,'unmannedVehicle','999');
  api.updateSourceControlDraft(unit,'unmannedMode','hitl');
  assert.equal(api.sourceControlCommand(unit,'unmanned_mode'),null);
});

test('TDL solution requires an explicit own report, source fit, recipients and available weapon', () => {
  const {unit,api}=harness();
  const {draft}=api.warningDraft(unit);
  assert.equal(api.extendedCommunicationCommand(unit,'send_solution'),null);
  draft.fit='rt';draft.report=api.warningReportKey(unit.communications.reports[0]);
  draft.receivers.add(2);draft.receivers.add(999);
  assert.equal(api.extendedCommunicationCommand(unit,'send_solution'),null,'no inferred weapon selection');
  draft.weapon='exact_weapon';
  assert.deepEqual(plain(api.extendedCommunicationCommand(unit,'send_solution')),
    {cmd:'communications',unit:1,action:{type:'send_solution',contact:7,sensor:'own_sensor',fit:'rt',receivers:[2],weapon:'exact_weapon'}});
  draft.fit='voice';
  assert.equal(api.extendedCommunicationCommand(unit,'send_solution'),null);
  assert.equal(api.warningDraft(unit).reports.length,1,'the original voice report remains available');
});

test('automatic sharing toggles only a known fitted data link on an explicit request', async () => {
  const {unit,commands,api}=harness();
  assert.equal(api.extendedCommunicationCommand(unit,'set_automatic','voice'),null);
  assert.equal(api.extendedCommunicationCommand(unit,'set_automatic','invented'),null);
  await api.sendExtendedCommunication('set_automatic','rt');
  assert.deepEqual(commands,[{cmd:'communications',unit:1,action:{type:'set_automatic',fit:'rt',enabled:true}}]);
  assert.equal(unit.communications.fits[1].automatic,false,'queued request does not change the reported sharing state');
});

test('own-side and common game locks reject draft commands before submission', () => {
  const {unit,state,api}=harness();
  unit.side='Red';
  assert.equal(api.sourceControlCommand(unit,'raise_snorkel'),null);
  assert.equal(api.extendedCommunicationCommand(unit,'set_automatic','rt'),null);
  unit.side='Blue';state.observation.exercise={locked:true};
  assert.equal(api.sourceControlCommand(unit,'raise_snorkel'),null);
  assert.equal(api.updateSourceControlDraft(unit,'powerMode','diesel'),false);
  state.observation.exercise=null;state.observation.over=true;
  assert.equal(api.sourceControlCommand(unit,'raise_snorkel'),null);
});

test('source controls retain explicit submission, telemetry provenance and touch-sized inputs', () => {
  const css=fs.readFileSync(path.join(__dirname,'../web/style.css'),'utf8');
  assert.match(app,/\['voice_warning_only','fitted_communications'\]\.includes/);
  assert.match(app,/reported\?\.telemetry \|\| \{\}/);
  assert.match(app,/状态时间.*telemetry\.status_at/);
  assert.match(app,/data-warning-solution/);
  assert.match(app,/data-source-authorization-label/);
  assert.doesNotMatch(part('  function sourceControlDraft(', '  function warningReportKey('),/sendCommandRaw\(/);
  assert.match(css,/\.source-controls \.cmd-btn,[\s\S]*?min-height: 44px/);
});

test('explicit v4 source battery overrides the unused legacy scalar in both languages', () => {
  const own={kind:'Sub',battery:null,battery_state:{charge_units:100,capacity_units:250,aip_fitted:true,aip_remaining_units:20}};
  for(const language of ['zh-CN','en']) {
    const text=batteryText(own,'hv2026-rts-v4',language);
    assert.match(text,/100.*250.*20/);
    assert.doesNotMatch(text,/尚未裁定|unadjudicated|历史电池|Historical battery/);
    assert.doesNotMatch(text,/无限|unlimited|半速|halves/);
  }
});

test('v4 source battery displays zero and partial unknown values without guessing a full charge', () => {
  for(const language of ['zh-CN','en']) {
    const knownZero=batteryText({kind:'Sub',battery:100,battery_state:{charge_units:0,capacity_units:250,aip_fitted:true,aip_remaining_units:0}},'hv2026-rts-v4',language);
    assert.match(knownZero,/0.*250.*0/);
    assert.doesNotMatch(knownZero,/未知|Unknown|100/);
    const partial=batteryText({kind:'Sub',battery:100,battery_state:{charge_units:null,capacity_units:250,aip_fitted:true,aip_remaining_units:null}},'hv2026-rts-v4',language);
    assert.match(partial,/未知|Unknown/);
    assert.match(partial,/250/);
    assert.doesNotMatch(partial,/100|电量 0|charge 0|AIP余量 0|AIP reserve 0/);
  }
});

test('pending source conditions do not erase known battery quantities or globally invalidate v4', () => {
  const own={kind:'Sub',battery_state:{charge_units:80,capacity_units:250,aip_fitted:false,pending_adjudications:{charge_denominator:'source pending'}}};
  assert.match(batteryText(own,'hv2026-rts-v4','zh-CN'),/80.*250.*部分来源条件仍待裁定/);
  assert.match(batteryText(own,'hv2026-rts-v4','en'),/80.*250.*some source conditions remain unadjudicated/);
  const event={domain:'asw',rule:'battery_low',detail:'legacy low battery'};
  assert.match(ruleReport(event).meaning,/battery_state/);
  assert.match(ruleReport(event,'en').meaning,/battery_state/);
  assert.doesNotMatch(ruleReport(event,'en').meaning,/V4 endurance remains unadjudicated/);
});
