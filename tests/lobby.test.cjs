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
