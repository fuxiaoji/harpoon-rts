const test=require('node:test'),assert=require('node:assert/strict');
const tiles=require('../web/satellite-tiles.js');
const theater={west:114,south:16,reference_lat:25,tile_layer:tiles.layer};
const k=60*Math.cos(25*Math.PI/180);
test('WGS84 tiles preserve coordinates, cross fixed image bounds and align adjacent edges',()=>{
 const toScreen=(x,y)=>({x:x*2,y:800-y*2});
 const list=tiles.layout(theater,toScreen,1200,800);assert.ok(list.length>1&&list.length<=128);
 for(const t of list){const span=180/2**t.z;assert.ok(Math.abs(t.x-(-180+t.col*span-114)*k*2)<1e-8);assert.ok(t.width>0&&t.height>0);}
 const a=list[0],b=list.find(t=>t.col===a.col+1&&t.row===a.row);assert.ok(Math.abs(a.x+a.width-b.x)<1e-8);
 const outside=tiles.layout(theater,(x,y)=>({x:(x-3000)*2,y:800-y*2}),1200,800);assert.ok(outside.length>0);
});
test('detail follows zoom and invalid projection never starts tile requests',()=>{
 const low=tiles.layout(theater,(x,y)=>({x,y:800-y}),1000,800);
 const high=tiles.layout(theater,(x,y)=>({x:x*32,y:800-y*32}),1000,800);assert.ok(high[0].z>low[0].z);
 assert.deepEqual(tiles.layout(theater,(x,y)=>({x:0,y:0}),1000,800),[]);
});
test('failed imagery leaves the already drawn offline canvas intact and caps concurrent/cache work',()=>{
 let created=0,draws=0;global.Image=class {constructor(){created++;}set src(v){this.url=v;queueMicrotask(()=>this.onerror());}};
 const ctx={drawImage(){draws++;}};
 assert.equal(tiles.draw(ctx,{...theater,tile_layer:'unknown'},()=>({x:0,y:0}),100,100),null);
 const report=tiles.draw(ctx,theater,(x,y)=>({x:x*2,y:800-y*2}),1200,800,()=>{});
 assert.equal(report.ready,0);assert.equal(draws,0);assert.ok(created<=6);assert.ok(tiles.status.cached<=256);
});
test('successive distant pans evict old tiles rather than growing the image cache',async()=>{
 let created=0;global.Image=class {constructor(){created++;}set src(v){queueMicrotask(()=>this.onload());}};
 const ctx={drawImage(){}};
 for(let i=0;i<20;i++){
   const longitude=-150+i*15,offset=(longitude-theater.west)*k;
   tiles.draw(ctx,theater,(x,y)=>({x:(x-offset)*8,y:700-y*8}),1000,700,()=>{});
   await new Promise(resolve=>setImmediate(resolve));
   assert.ok(tiles.status.cached<=256);assert.equal(tiles.status.active,0);
 }
 assert.ok(created>256);
});
