const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const crypto=require('node:crypto');
const M=require('../web/lobby-model.js');

test('launch always starts paused and resolves public id to actual scenario filename',()=>{
  assert.deepEqual(M.gameRequest({id:'tattletail_1991',file:'tattletail.json'}),{scenario:'tattletail',speed:0});
  assert.deepEqual(M.gameRequest({file:'dual_carrier_hotseat.json'},'0'),{scenario:'dual_carrier_hotseat',speed:0,seed:0});
});
test('AI opponent and assistant modes produce strict-double-blind match configs',()=>{
  assert.deepEqual(M.agentMatchConfigs({agentOpponent:true,assistantMode:'semi',matchSpeed:30}),[
    {side:'Red',enabled:true,autonomy:'execute',auto_run:false,auto_speed:30},
    {side:'Blue',enabled:true,autonomy:'advise',auto_run:false,auto_speed:30}
  ]);
  assert.deepEqual(M.agentMatchConfigs({agentOpponent:true,assistantMode:'full',matchSpeed:60}),[
    {side:'Red',enabled:true,autonomy:'execute',auto_run:true,auto_speed:60},
    {side:'Blue',enabled:true,autonomy:'execute',auto_run:true,auto_speed:60}
  ]);
  assert.deepEqual(M.agentMatchConfigs({assistantMode:'off',matchSpeed:15}),[]);
  assert.equal(M.agentMatchConfigs({assistantMode:'full',matchSpeed:1})[0].auto_speed,5);
  assert.equal(M.agentMatchStartsAtSpeed({agentOpponent:true,assistantMode:'semi'}),true);
  assert.equal(M.agentMatchStartsAtSpeed({agentOpponent:false,assistantMode:'semi'}),false);
  assert.equal(M.agentMatchStartsAtSpeed({agentOpponent:false,assistantMode:'full'}),true);
});
test('opponent mode: seat-aware sides, tactical PvE config and legacy checkbox mapping',()=>{
  assert.equal(M.opponent({opponent:'tactical'}),'tactical');
  assert.equal(M.opponent({agentOpponent:true}),'llm');
  assert.equal(M.opponent({}),'human');
  assert.equal(M.opponentValue('tactical'),'tactical');
  assert.equal(M.opponentValue('bogus','llm'),'llm');
  assert.equal(M.opponentValue('bogus'),'human');
  assert.equal(M.opponentSide('Blue'),'Red');
  assert.equal(M.opponentSide('Red'),'Blue');
  assert.deepEqual(M.tacticalAutopilotConfig({opponent:'tactical',seat:'Blue'}),{side:'Red',enabled:true});
  assert.deepEqual(M.tacticalAutopilotConfig({opponent:'tactical',seat:'Red'}),{side:'Blue',enabled:true});
  assert.equal(M.tacticalAutopilotConfig({opponent:'human',seat:'Blue'}),null);
  // 状态机 PvE 不依赖 LLM，新局保持暂停、由玩家掌握时间。
  assert.equal(M.agentMatchStartsAtSpeed({opponent:'tactical',seat:'Blue'}),false);
  assert.deepEqual(M.agentMatchConfigs({opponent:'tactical',assistantMode:'off'}),[]);
  assert.equal(M.agentMatchStartsAtSpeed({opponent:'llm',seat:'Red'}),true);
  assert.deepEqual(M.agentMatchConfigs({opponent:'llm',seat:'Red',assistantMode:'off',matchSpeed:15})[0].side,'Blue');
});
test('optional seeds preserve exact integers and reject truncation or precision loss',()=>{
  assert.equal(M.seedValue(''),undefined);assert.equal(M.seedValue(' 42 '),42);
  assert.equal(M.seedValue('9007199254740991'),Number.MAX_SAFE_INTEGER);
  for(const seed of ['1.2','-1','2e3','hello','9007199254740992'])assert.throws(()=>M.seedValue(seed),/INVALID_SEED/);
});
test('launch and replay paths cannot escape their resource directories',()=>{
  for(const file of ['../secrets.json','x/y.json','x.exe','x.json?test'])assert.throws(()=>M.gameRequest({file}),/INVALID_SCENARIO/);
  assert.ok(M.safeRecording({id:'twin-shields-20261019',file:'twin-shields-20261019.replay.json.gz'}));
  assert.ok(!M.safeRecording({id:'bad',file:'../secret'}));
});
test('lobby scenario summaries exactly match authored durations, counts and source hashes',()=>{
  const catalog=JSON.parse(fs.readFileSync(path.join(__dirname,'../web/lobby-catalog.json')));
  for(const [id,meta] of Object.entries(catalog.scenarios)){
    const bytes=fs.readFileSync(path.join(__dirname,'../scenarios',meta.file)),source=JSON.parse(bytes);
    assert.equal(source.id,id);assert.equal(meta.duration_s,source.duration_s);
    assert.equal(meta.initial_units,source.units.length);
    assert.equal(meta.source_sha256,crypto.createHash('sha256').update(bytes).digest('hex'));
    for(const side of ['Blue','Red'])assert.equal(meta.sides[side],source.units.filter(u=>u.side===side).length);
  }
});
