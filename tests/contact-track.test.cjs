const test=require('node:test');
const assert=require('node:assert/strict');
const {describe}=require('../web/contact-track.js');
test('missed updates show coasting before the engine stale timeout',()=>{
  assert.equal(describe({last_seen_s:7,stale:false}).coasting,false);
  const c=describe({last_seen_s:37,stale:false});
  assert.equal(c.coasting,true);
  assert.equal(c.label,'外推 · 37秒前');
  assert.match(c.explanation,/尚未确认目标仍存活/);
  assert.equal(describe({last_seen_s:8},'en').label,'COAST · 8s ago');
});
test('lost tracks never claim a kill from hidden entity truth',()=>{
  assert.equal(describe({last_seen_s:0,stale:true},'en').label,'LOST · 0s ago');
  assert.equal(describe({last_seen_s:-9}).age,0);
  assert.deepEqual(describe({last_seen_s:37,alive:true}),describe({last_seen_s:37,alive:false}));
});
