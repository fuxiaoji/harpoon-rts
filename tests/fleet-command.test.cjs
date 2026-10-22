const test=require('node:test'),assert=require('node:assert/strict');
const {members,routeOrders,speedOrders}=require('../web/fleet-command.js');
const unit=(id,kind,side,x,y)=>({id,kind,side,pos:{x,y},course_deg:90});
test('Group orders use own selected mobile units and reject invalid destinations',()=>{
 const obs={your_side:'Blue',units:[unit(1,'Ship','Blue',0,0),unit(2,'Ship','Red',5,0),unit(3,'Missile','Blue',0,1),unit(4,'Base','Blue',0,2)]};
 assert.deepEqual(members(obs,[1,2,3,4]).map(u=>u.id),[1]);
 assert.deepEqual(routeOrders(obs,[1],{x:NaN,y:0}),[]);
 assert.deepEqual(routeOrders(obs,[2],{x:5,y:0}),[]);
});
test('One follower speed adjustment detaches it; selecting its leader preserves formation',()=>{
 const leader=unit(1,'Ship','Blue',0,0),follower={...unit(2,'Ship','Blue',0,5),navigation:{station:{leader:1,bearing_rel_deg:270,distance_nm:5}}},obs={your_side:'Blue',units:[leader,follower]};
 assert.deepEqual(speedOrders(obs,[2],12).map(c=>c.cmd),['clear_station','set_speed']);
 assert.deepEqual(speedOrders(obs,[1,2],12),[{cmd:'set_speed',unit:1,speed_kts:12}]);
 const aircraft={...unit(3,'Aircraft','Blue',0,0),mission:'cap'};obs.units.push(aircraft);
 assert.deepEqual(speedOrders(obs,[3],400).map(c=>c.cmd),['retask_aircraft','set_speed']);
 assert.deepEqual(speedOrders(obs,[1],NaN),[]);
});
test('Formation navigation clears stale stations and uses one leader instead of piling ships at a point',()=>{
 const obs={your_side:'Blue',units:[unit(1,'Ship','Blue',0,0),unit(2,'Ship','Blue',0,5)]};
 const orders=routeOrders(obs,[1,2],{x:100,y:100},true);
 assert.deepEqual(orders.filter(c=>c.cmd==='clear_station').map(c=>c.unit),[1,2]);
 assert.equal(orders.filter(c=>c.cmd==='goto').length,1);
 const follow=orders.find(c=>c.cmd==='follow');assert.equal(follow.leader,1);assert.equal(follow.distance_nm,5);assert.equal(follow.bearing_rel_deg,270);
 const separate=routeOrders(obs,[1,2],{x:100,y:100},false).filter(c=>c.cmd==='goto');
 assert.equal(separate[1].waypoints[0].y-separate[0].waypoints[0].y,5);
});
test('Plotted aircraft navigation explicitly replaces its CAP or ASW autopilot',()=>{
 const obs={your_side:'Blue',units:[unit(1,'Aircraft','Blue',0,0)]};
 const orders=routeOrders(obs,[1],{x:20,y:30});
 const task=orders.find(c=>c.cmd==='retask_aircraft');assert.equal(task.manual,true);assert.deepEqual(task.mission.point,{x:20,y:30});
 assert(orders.findIndex(c=>c.cmd==='retask_aircraft')<orders.findIndex(c=>c.cmd==='goto'));
});
