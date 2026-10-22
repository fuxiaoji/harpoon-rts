/* Public chart overlay; static coastlines never use an opponent's state. */
(function(root){
  'use strict';
  let key='',terrain=null,loading=false;
  function paint(ctx,t,project){
    if(!t?.polygons)return;
    ctx.save();ctx.strokeStyle='#d5cf9b99';ctx.fillStyle='#8e997314';ctx.lineWidth=1;
    for(const p of t.polygons){
      ctx.beginPath();for(const ring of [p.outer,...p.holes||[]])ring.forEach((v,i)=>{const q=project(v.x,v.y);i?ctx.lineTo(q.x,q.y):ctx.moveTo(q.x,q.y);if(i===ring.length-1)ctx.closePath();});
      ctx.fill('evenodd');ctx.stroke();
    }ctx.restore();
  }
  root.HarpoonNavigation={paint,draw(ctx,obs,project){
    if(!obs)return;
    if(key!==obs.scenario_id){key=obs.scenario_id;terrain=null;loading=false;}
    if(terrain===null&&!loading){loading=true;const requested=key;
      fetch('/api/terrain').then(r=>{if(!r.ok)throw Error(r.status);return r.json();}).then(v=>{if(requested===key&&v.scenario_id===key)terrain=v.terrain||{};}).catch(()=>{if(requested===key)terrain={};}).finally(()=>{if(requested===key)loading=false;});
    }
    paint(ctx,terrain,project);
  }};
})(typeof window!=='undefined'?window:globalThis);
