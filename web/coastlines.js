// The public chart is generated from the exact scenario collision geometry.
// It contains no units, intelligence or referee state. Old scenarios stay frozen.
(() => {
  let status='idle',chart=null,scenarioId=null;
  let archivedTerrain=null,archivedChart=null;
  function draw(ctx,obs,toScreen,width,height,terrain=null) {
    if(!['taiwan_island_exercise','taiwan_island_opening','island_joint_exercise'].includes(obs?.scenario_id))return;
    if(!terrain&&scenarioId!==obs.scenario_id){scenarioId=obs.scenario_id;chart=null;status='idle';}
    if(terrain && terrain!==archivedTerrain) {
      archivedTerrain=terrain;
      archivedChart={polygons:terrain.polygons.map(p=>({bounds:p.bounds||[
        Math.min(...p.outer.map(q=>q.x)),Math.min(...p.outer.map(q=>q.y)),Math.max(...p.outer.map(q=>q.x)),Math.max(...p.outer.map(q=>q.y))],
        rings:[p.outer,...(p.holes||[])].map(r=>r.map(q=>[q.x,q.y]))}))};
    }
    const active=terrain?archivedChart:chart;
    if(!terrain && status==='idle') {
      status='loading';
      const id=scenarioId;
      fetch(id==='taiwan_island_exercise'?'maps/taiwan-island-land.json':'maps/taiwan-island-opening-land.json').then(r=>{if(!r.ok)throw Error(r.status);return r.json();})
        .then(data=>{if(id===scenarioId){chart=data;status='ready';}}).catch(()=>{if(id===scenarioId)status='error';});
    }
    if(!active)return;
    ctx.save();ctx.strokeStyle='#d1c88799';ctx.fillStyle='#7da68b13';ctx.lineWidth=1;
    for(const p of active.polygons) {
      const a=toScreen(p.bounds[0],p.bounds[1]),b=toScreen(p.bounds[2],p.bounds[3]);
      if(Math.max(a.x,b.x)<0||Math.min(a.x,b.x)>width||Math.max(a.y,b.y)<0||Math.min(a.y,b.y)>height)continue;
      ctx.beginPath();
      for(const ring of p.rings){ring.forEach(([x,y],i)=>{const q=toScreen(x,y);if(i)ctx.lineTo(q.x,q.y);else ctx.moveTo(q.x,q.y);});ctx.closePath();}
      ctx.fill('evenodd');ctx.stroke();
    }
    ctx.restore();
  }
  window.HarpoonCoastlines={draw,get status(){return archivedChart?'archive-ready':status;}};
})();
