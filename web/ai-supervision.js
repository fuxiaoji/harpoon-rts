/* Friendly task health only. This display never sends orders. */
(function(root){
  'use strict';
  const names={shore_missile:['岸基巡航弹','Shore cruise units'],shore_aviation:['岸基航空','Shore aviation'],naval:['海军编队','Naval forces'],patrol:['日常执勤','Routine patrol'],infrastructure:['设施','Installations']};
  function project(obs){
    if(!obs)return null;
    const units=(obs.units||[]).filter(u=>u.side===obs.your_side),byId=new Map(units.map(u=>[Number(u.id),u]));
    const p=obs.ai_supervision||{},origins={...(p.known_air_origins||{})};
    for(const f of obs.exercise?.opening_flights||[])origins[f.unit]=f.home;
    const groups=(obs.exercise?.joint_groups||[]).filter(g=>g.side===obs.your_side).map(g=>{
      const ids=new Set(g.members.map(Number));
      const air=units.filter(u=>u.kind==='Aircraft'&&(ids.has(Number(u.id))||ids.has(Number(origins[u.id]))));
      const rounds=Object.entries(p.actual_offensive_rounds_by_unit||{}).filter(([id])=>ids.has(Number(id))||ids.has(Number(origins[id]))).reduce((n,[,v])=>n+Number(v),0);
      return {role:g.role,name:g.name,platforms:g.members.filter(id=>byId.has(Number(id))).length,linked_aircraft:air.length,actual_rounds:rounds};
    });
    const tasks=Object.values(p.tasks||{}),attacks=Object.values(p.attack_account||{});
    return {groups,tasks:tasks.length,expired:tasks.filter(t=>t.expires_at<=obs.tick).length,
      held_attacks:attacks.filter(a=>a.hold_until>obs.tick).length,ew:Object.keys(p.support_bindings||{}).length,asw:Object.keys(p.asw_assignments||{}).length,
      queued:(p.scheduled_releases||[]).filter(r=>byId.has(Number(r.unit))),limits:p.limits||null,airborne:units.filter(u=>u.kind==='Aircraft').length,observed_at:p.observed_at??null};
  }
  function render(obs){
    const box=root.document?.getElementById('ai-task-health');if(!box)return;
    const data=project(obs);box.hidden=!data;if(!data)return;
    const en=root.HarpoonI18n?.language==='en',t=(zh,eng)=>en?eng:zh;
    const summary=box.querySelector('summary'),body=box.querySelector('[data-ai-health-body]');
    summary.textContent=t(`AI任务复核 · ${data.tasks}项`, `AI task review · ${data.tasks}`);body.replaceChildren();
    const row=text=>{const el=root.document.createElement('p');el.textContent=text;body.append(el);};
    row(t(`在空 ${data.airborne} · 攻击待复核 ${data.held_attacks} · 定时发射 ${data.queued.length} · 干扰支援 ${data.ew} · 反潜分区 ${data.asw}`,`Airborne ${data.airborne} · attack holds ${data.held_attacks} · queued releases ${data.queued.length} · EW bindings ${data.ew} · ASW sectors ${data.asw}`));
    for(const r of data.queued.slice(0,4))row(t(`#${r.unit} → C${r.contact}：最早T${r.fire_at}、截止T${r.deadline}复核；尚未确认实际发射。`,`#${r.unit} → C${r.contact}: revalidate from T${r.fire_at} until T${r.deadline}; no actual release confirmed yet.`));
    if(data.observed_at!==null)row(t(`复核数据T${data.observed_at}；当前T${obs.tick}。`,`Review data T${data.observed_at}; current T${obs.tick}.`));
    if(data.limits)row(t(`CAP预算 ${data.limits.cap_global}架；后续突击在空预算 ${data.limits.active_strike_aircraft}架；持续任务 ${data.limits.task_review_s}秒复核。`,`CAP budget ${data.limits.cap_global}; subsequent active-strike budget ${data.limits.active_strike_aircraft}; task review ${data.limits.task_review_s}s.`));
    for(const g of data.groups){const label=(names[g.role]||[g.name,g.name])[en?1:0];row(t(`${label}：现存宿主/平台 ${g.platforms}，已关联在空 ${g.linked_aircraft}，记录进攻弹药实耗 ${g.actual_rounds}。`,`${label}: ${g.platforms} hosts/platforms, ${g.linked_aircraft} linked airborne, ${g.actual_rounds} recorded offensive rounds spent.`));}
    row(t('只统计本方已知信息；开场预置在途弹与后续AI实际发射分别记录。无确认估损不等于没有命中。','Friendly observations only. Authored opening ordnance is separate from later AI releases. No confirmed BDA does not mean no hit.'));
  }
  root.HarpoonAiHealth={project,render};if(typeof module!=='undefined')module.exports={project};
})(typeof window!=='undefined'?window:globalThis);
