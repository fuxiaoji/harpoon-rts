/* Read-only archive projection. No game API, orders or simulation calls. */
(function(root) {
  'use strict';
  const SIDES = ['Blue', 'Red'];
  const finite = n => typeof n === 'number' && Number.isFinite(n);
  const validTick = n => finite(n) && n >= 0 && n <= 100000000 && Number.isInteger(n);
  const point = p => p && finite(p.x) && finite(p.y);
  function fail() { throw new Error('INVALID_REPLAY'); }
  function normalize(input) {
    if (!input || input.format !== 'harpoon-observation-replay' || input.version !== 1) fail();
    if (typeof input.title !== 'string' && !(input.title && typeof input.title.zh === 'string' && typeof input.title.en === 'string')) fail();
    if (input.theater && (!['west','east','south','north','reference_lat'].every(k=>finite(input.theater[k])) || input.theater.east<=input.theater.west || input.theater.north<=input.theater.south || Math.abs(input.theater.reference_lat)>=90)) fail();
    if (input.bookmarks && (!Array.isArray(input.bookmarks) || input.bookmarks.some(b=>!validTick(b.tick) || !Array.isArray(b.views) || b.views.some(v=>!['Blue','Red','God'].includes(v))))) fail();
    const out = {...input, frames:{}, events:[]};
    const ownership = new Map();
    for (const side of SIDES) {
      const source = input.frames?.[side];
      if (!Array.isArray(source) || !source.length || source.length > 100000) fail();
      const unique = new Map();
      for (const frame of source) {
        if (!validTick(frame.tick) || !Array.isArray(frame.units) || !Array.isArray(frame.contacts) || frame.units.length > 20000 || frame.contacts.length > 20000) fail();
        if (frame.units.some(u => u.side !== side || !point(u.pos) || !finite(u.id))) fail();
        if (frame.contacts.some(c => !point(c.estimate) || !finite(c.id) || 'about' in c || 'offset' in c)) fail();
        if (frame.units.some(u=>typeof u.name !== 'string' || typeof u.kind !== 'string' || u.weapons != null && !Array.isArray(u.weapons))) fail();
        if (frame.mission?.config && (!point(frame.mission.config.center) || !finite(frame.mission.config.radius_nm) || frame.mission.config.radius_nm<0)) fail();
        if (new Set(frame.units.map(u=>u.id)).size !== frame.units.length || new Set(frame.contacts.map(c=>c.id)).size !== frame.contacts.length) fail();
        for(const u of frame.units) { if(ownership.has(u.id) && ownership.get(u.id)!==side) fail(); ownership.set(u.id,side); }
        unique.set(frame.tick, frame);
      }
      out.frames[side] = [...unique.values()].sort((a,b)=>a.tick-b.tick);
    }
    if (!Array.isArray(input.events) || input.events.length > 1000000) fail();
    const events = new Map();
    for (const e of input.events) {
      if (!validTick(e.tick) || !Number.isSafeInteger(e.seq) || e.seq < 1 || typeof e.event !== 'string') fail();
      if (!(e.vis === 'All' || e.vis === 'Referee' || SIDES.includes(e.vis?.Side))) fail();
      if (events.has(e.seq) && JSON.stringify(events.get(e.seq)) !== JSON.stringify(e)) fail();
      events.set(e.seq, e);
    }
    out.events = [...events.values()].sort((a,b)=>a.tick-b.tick || a.seq-b.seq);
    out.end = Math.max(...SIDES.map(s=>out.frames[s].at(-1).tick));
    if (out.events.some(e=>e.tick>out.end)) fail();
    out.ticks = [...new Set(SIDES.flatMap(s=>out.frames[s].map(f=>f.tick)))].sort((a,b)=>a-b);
    return out;
  }
  function floorFrame(frames, tick) {
    let lo=0, hi=frames.length;
    while(lo<hi) { const m=(lo+hi)>>>1; if(frames[m].tick<=tick) lo=m+1; else hi=m; }
    return lo ? frames[lo-1] : null;
  }
  function allowed(event, view) { return view==='God' || event.vis==='All' || event.vis?.Side===view; }
  function project(replay, tick, view) {
    if (!['Blue','Red','God'].includes(view)) fail();
    const at = Math.max(0, Math.min(replay.end, finite(tick)?tick:0));
    const blue=floorFrame(replay.frames.Blue,at), red=floorFrame(replay.frames.Red,at);
    const selected=view==='God'?[blue,red].filter(Boolean):[view==='Blue'?blue:red].filter(Boolean);
    return {tick:at, view, snapshots:{Blue:blue?.tick??null,Red:red?.tick??null},
      units:selected.flatMap(f=>f.units), contacts:view==='God'?[]:(selected[0]?.contacts||[]),
      events:replay.events.filter(e=>e.tick<=at && allowed(e,view)),
      mission:selected[0]?.mission||null,
      result:selected.find(f=>f.over)?.result||null};
  }
  function neighbor(replay,tick,direction) {
    return direction>0 ? replay.ticks.find(t=>t>tick)??replay.end : [...replay.ticks].reverse().find(t=>t<tick)??0;
  }
  function trail(replay,tick,view,id,isContact=false) {
    if(isContact && view==='God') return [];
    const sides=view==='God'?SIDES:[view], points=[];
    for(const side of sides) for(const frame of replay.frames[side]) {
      if(frame.tick>tick) break;
      const unit=(isContact?frame.contacts:frame.units).find(u=>u.id===id);
      if(unit) points.push({tick:frame.tick,...(isContact?unit.estimate:unit.pos)});
    }
    return points.slice(-300);
  }
  function serializable(replay) {
    const {end,ticks,...archive}=replay; return archive;
  }
  const api={normalize,floorFrame,allowed,project,neighbor,trail,serializable};
  if(typeof module!=='undefined') module.exports=api;
  root.HarpoonReplayModel=api;
})(typeof window!=='undefined'?window:globalThis);
