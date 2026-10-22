const test=require('node:test'),assert=require('node:assert/strict');
const {project}=require('../web/ai-supervision.js');
test('future releases remain planned and side-private, without inventing expenditure',()=>{
 const p=project({your_side:'Blue',tick:20,units:[{id:1,side:'Blue',kind:'Ship'},{id:2,side:'Red',kind:'Ship'}],ai_supervision:{observed_at:0,scheduled_releases:[{unit:1,contact:3,fire_at:60,deadline:180},{unit:2,contact:4,fire_at:60,deadline:180}]}});
 assert.equal(p.queued.length,1);assert.equal(p.observed_at,0);assert.equal(p.groups.length,0);assert.ok(!('released_rounds' in p));
});
test('joint health contains only friendly groups and counts documented air origins',()=>{
 const o={your_side:'Blue',tick:300,units:[{id:1,side:'Blue',kind:'Base'},{id:5,side:'Blue',kind:'Aircraft'},{id:6,side:'Blue',kind:'Aircraft'},{id:2,side:'Red',kind:'Ship'}],exercise:{joint_groups:[{side:'Blue',name:'own',role:'shore_aviation',members:[1]},{side:'Red',name:'foreign',role:'naval',members:[2]}]},ai_supervision:{known_air_origins:{5:1},actual_offensive_rounds_by_unit:{1:8,5:2,6:19}}};
 const p=project(o);assert.equal(p.groups.length,1);assert.equal(p.groups[0].linked_aircraft,1);assert.equal(p.groups[0].actual_rounds,10);assert.equal(p.airborne,2);
 assert.ok(!JSON.stringify(p).includes('foreign'));
});
test('task expiry and unknown attack outcome are distinct from losses or confirmed misses',()=>{
 const p=project({tick:300,your_side:'Red',units:[],ai_supervision:{tasks:{1:{expires_at:300},2:{expires_at:600}},attack_account:{1:{hold_until:600,observed_damage:0},2:{hold_until:200}},support_bindings:{9:{}},asw_assignments:{8:{}}}});
 assert.equal(p.expired,1);assert.equal(p.held_attacks,1);assert.equal(p.ew,1);assert.equal(p.asw,1);assert.ok(!('misses' in p));assert.ok(!('kills' in p));
});
test('legacy observations and opening origins require no inferred host relationships',()=>{
 assert.equal(project(null),null);assert.equal(project({your_side:'Blue',tick:0,units:[]}).tasks,0);
 const p=project({your_side:'Blue',tick:0,units:[{id:1,side:'Blue',kind:'Base'},{id:5,side:'Blue',kind:'Aircraft'}],exercise:{opening_flights:[{unit:5,home:1}],joint_groups:[{side:'Blue',name:'own',role:'shore_aviation',members:[1]}]}});
 assert.equal(p.groups[0].linked_aircraft,1);assert.equal(p.groups[0].actual_rounds,0);
});
