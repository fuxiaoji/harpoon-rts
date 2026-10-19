const test=require('node:test');
const assert=require('node:assert/strict');
const {bounds,geographic,local}=require('../web/theater.js');
test('image corners and game positions share the same geographic plane',()=>{
  const t={west:118,east:134,south:18,north:30,reference_lat:24};
  const b=bounds(t);
  const ne=geographic({x:b.width,y:b.height},t);
  assert.ok(Math.abs(ne.lon-134)<1e-10);assert.equal(ne.lat,30);
  for(const [lon,lat] of [[118,18],[121,23.7],[125,23.3],[134,30]]){
    const p=local(lon,lat,t),q=geographic(p,t);
    assert.ok(Math.abs(q.lon-lon)<1e-10);assert.ok(Math.abs(q.lat-lat)<1e-10);
  }
  assert.ok(Math.abs(b.width-876.99)<0.1);assert.equal(b.height,720);
});
