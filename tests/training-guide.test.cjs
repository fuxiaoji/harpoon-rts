const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const { planFor, inputMatches, commandMatches, completedByCommand, placeCoach, sessionKey } = require('../web/training-guide.js');
const courses = JSON.parse(fs.readFileSync(require('node:path').join(__dirname, '../data/training/courses.json'), 'utf8')).courses;

test('all 55 objectives have actionable bilingual guidance including the initial briefing', () => {
  for (const course of courses) for (const step of course.steps) {
    const plan = planFor(step, { unit: 4, unitSelected: true, contactSelected: true, course: course.id });
    assert.ok(plan.length > 0);
    for (const cue of plan) {
      assert.notEqual(cue.id, 'details', `${course.id}/${step.id} needs specific guidance`);
      assert.ok(cue.selector);
      for (const lang of ['zh','en']) { assert.ok(cue.title[lang]); assert.ok(cue.body[lang]); }
    }
    if (step.id === 'brief') assert.equal(plan[0].selector, '[data-training="ack"]');
  }
});
test('speed is taught as input, acknowledged order and actual execution', () => {
  const step = courses.find(c=>c.id==='navigation').steps.find(s=>s.objective==='speed');
  const plan = planFor(step, {unit:9,unitSelected:true});
  assert.deepEqual(plan.map(c=>c.kind), ['input','command','execute']);
  assert.equal(inputMatches(plan[0], ''), false);
  assert.equal(inputMatches(plan[0], '4'), false);
  assert.equal(inputMatches(plan[0], '12'), true);
  assert.equal(commandMatches(plan[1], {ok:false,command:{cmd:'set_speed',unit:9,speed_kts:12}},9),false);
  assert.equal(commandMatches(plan[1], {ok:true,command:{cmd:'set_speed',unit:9,speed_kts:4}},9),false);
  assert.equal(commandMatches(plan[1], {ok:true,command:{cmd:'set_speed',unit:4,speed_kts:12}},9),false);
  assert.equal(commandMatches(plan[1], {ok:true,command:{cmd:'set_speed',unit:9,speed_kts:12}},9),true);
});
test('refreshing a command objective first guides lost unit and contact selections', () => {
  const step = courses.find(c=>c.id==='gunnery').steps.find(s=>s.objective==='gun_fire');
  const plan = planFor(step, {unit:7,unitSelected:false,contactSelected:false});
  assert.deepEqual(plan.map(c=>c.id), ['own','contact','weapon-value','fire-order','execute']);
  assert.equal(plan[0].selector, '[data-select-unit="7"]');
  assert.equal(inputMatches(plan[2], 'sam_long'), false);
  assert.equal(inputMatches(plan[2], 'gun_5in'), true);
});
test('checkpoint retry resets local action guidance while quiz feedback keeps it', () => {
  const session={attempt:3,step:2,retries:0,mistakes:0};
  assert.notEqual(sessionKey(session),sessionKey({...session,retries:1}));
  assert.equal(sessionKey(session),sessionKey({...session,mistakes:1}));
});
test('fast commands between instructor frames still move guidance to execution', () => {
  const step = courses.find(c=>c.id==='aviation').steps.find(s=>s.objective==='recall');
  const plan = planFor(step,{unit:1,unitSelected:true});
  assert.deepEqual(completedByCommand(plan,new Set(),{ok:true,command:{cmd:'recover_aircraft',unit:1}},1),['recall-order']);
});
test('a waypoint order cannot skip an earlier speed order in the assessment', () => {
  const step = courses.find(c=>c.id==='assessment').steps.find(s=>s.objective==='route');
  const plan = planFor(step,{unit:1,unitSelected:true,course:'assessment'});
  const result={ok:true,command:{cmd:'goto',unit:1,waypoints:[{x:1,y:0}]}};
  assert.deepEqual(completedByCommand(plan,new Set(),result,1),[]);
  assert.ok(completedByCommand(plan,new Set(['speed-order']),result,1).includes('route-order'));
  assert.equal(commandMatches(plan.find(c=>c.id==='route-order'),{ok:true,command:{cmd:'goto',unit:1,waypoints:[{x:9,y:0}]}},1),false);
});
test('coach stays in the viewport without covering its highlighted control', () => {
  for (const viewport of [{width:390,height:720},{width:1280,height:720},{width:1540,height:1040}]) {
    for (const target of [
      {left:12,top:20,width:100,height:36},
      {left:viewport.width-145,top:210,width:125,height:45},
      {left:viewport.width/2-45,top:viewport.height-75,width:90,height:35},
    ]) {
      target.right=target.left+target.width; target.bottom=target.top+target.height;
      const box=placeCoach(target,viewport,{width:340,height:280});
      assert.ok(box.left>=12 && box.top>=12);
      assert.ok(box.left+box.width<=viewport.width-12);
      assert.ok(box.top+box.height<=viewport.height-12);
      const overlapX=Math.min(target.right,box.left+box.width)-Math.max(target.left,box.left);
      const overlapY=Math.min(target.bottom,box.top+box.height)-Math.max(target.top,box.top);
      assert.ok(overlapX<=0 || overlapY<=0, JSON.stringify({target,box}));
    }
  }
});
