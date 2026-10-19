const test=require('node:test');
const assert=require('node:assert/strict');
const M=require('../web/replay-model.js');

function fixture(){
  const unit=(id,side,x)=>({id,side,pos:{x,y:0},name:side,kind:'Ship'});
  const frame=(tick,id,side,x,contacts=[])=>({tick,units:[unit(id,side,x)],contacts,over:false,result:null});
  return {format:'harpoon-observation-replay',version:1,id:'test',title:'Test battle',frames:{
    Blue:[frame(0,1,'Blue',0),frame(10,1,'Blue',10,[{id:1,estimate:{x:90,y:0}}])],
    Red:[frame(2,2,'Red',100,[{id:1,estimate:{x:5,y:0}}]),frame(12,2,'Red',120)]},
    events:[{seq:1,tick:0,event:'scenario_started',vis:'All'},
      {seq:2,tick:5,event:'detection',vis:{Side:'Red'}},
      {seq:3,tick:6,event:'impact',vis:{Side:'Blue'}},
      {seq:4,tick:7,event:'rules_event',vis:'Referee'},
      {seq:5,tick:12,event:'scenario_ended',vis:'All'}]};
}

test('side views never merge enemy truth or opposite-side private events',()=>{
  const r=M.normalize(fixture()),blue=M.project(r,8,'Blue'),red=M.project(r,8,'Red');
  assert.deepEqual(blue.units.map(u=>u.id),[1]);assert.deepEqual(red.units.map(u=>u.id),[2]);
  assert.deepEqual(blue.events.map(e=>e.seq),[1,3]);assert.deepEqual(red.events.map(e=>e.seq),[1,2]);
  assert.deepEqual(blue.contacts,[]);assert.equal(red.contacts[0].id,1);
});
test('god view uses both last own-state samples and includes referee events only up to cursor',()=>{
  const r=M.normalize(fixture()),p=M.project(r,8,'God');
  assert.deepEqual(p.units.map(u=>u.pos.x),[0,100]);assert.deepEqual(p.snapshots,{Blue:0,Red:2});
  assert.deepEqual(p.events.map(e=>e.seq),[1,2,3,4]);assert.deepEqual(p.contacts,[]);
});
test('seeking backwards does not retain future contacts, positions, events or outcome',()=>{
  const input=fixture();input.frames.Blue[1].over=true;input.frames.Blue[1].result={winner:'Blue'};
  const r=M.normalize(input);assert.equal(M.project(r,12,'Blue').result.winner,'Blue');
  const past=M.project(r,1,'God');assert.deepEqual(past.units.map(u=>u.id),[1]);
  assert.equal(past.result,null);assert.deepEqual(past.events.map(e=>e.seq),[1]);
  assert.deepEqual(past.snapshots,{Blue:0,Red:null});
});
test('shared contact numbers are scoped to the selected side, including trails',()=>{
  const r=M.normalize(fixture());
  assert.deepEqual(M.trail(r,12,'Blue',1,true).map(p=>p.x),[90]);
  assert.deepEqual(M.trail(r,12,'Red',1,true).map(p=>p.x),[5]);
  assert.deepEqual(M.trail(r,5,'Blue',1,true),[]);
  assert.deepEqual(M.trail(r,8,'God',1).map(p=>p.x),[0]);
});
test('sample stepping handles asynchronous samples and both time boundaries',()=>{
  const r=M.normalize(fixture());assert.equal(M.neighbor(r,0,-1),0);
  assert.equal(M.neighbor(r,2,1),10);assert.equal(M.neighbor(r,10,-1),2);
  assert.equal(M.neighbor(r,12,1),12);assert.equal(M.project(r,999,'Red').tick,12);
});
test('untrusted imports reject hidden-truth contacts, wrong-side units and invalid event visibility',()=>{
  for(const mutate of [
    r=>r.frames.Blue[0].units[0].side='Red',r=>r.frames.Blue[1].contacts[0].about=2,
    r=>r.frames.Blue[0].units[0].pos.x=NaN,r=>r.frames.Red=[],
    r=>r.events[0].vis='Blue',r=>r.events[0].tick=100,r=>r.frames.Blue[0].tick=-1
  ]){const r=fixture();mutate(r);assert.throws(()=>M.normalize(r),/INVALID_REPLAY/);}
});
test('import/export round trip preserves exact frames and event visibility',()=>{
  const r=M.normalize(fixture()),encoded=JSON.parse(JSON.stringify(M.serializable(r)));
  assert.ok(!('ticks' in encoded));assert.ok(!('end' in encoded));
  const reopened=M.normalize(encoded);assert.deepEqual(M.project(reopened,8,'God'),M.project(r,8,'God'));
});
