/* Task-level player controls. All orders still pass through the authority API. */
(function(root){
  'use strict';
  function selected(obs,ids){const wanted=new Set(ids.map(Number));return(obs?.units||[]).filter(u=>u.side===obs.your_side&&wanted.has(Number(u.id))&&!['Missile','Torpedo'].includes(u.kind));}
  function members(obs,ids){return selected(obs,ids).filter(u=>u.kind!=='Base');}
  function speedOrders(obs,ids,speedKts){
    if(!Number.isFinite(speedKts)||speedKts<0||speedKts>2000)return[];
    const units=members(obs,ids),selectedIds=new Set(units.map(u=>Number(u.id))),orders=[];
    for(const u of units){const station=u.navigation?.station;
      if(station&&selectedIds.has(Number(station.leader)))continue;
      if(station)orders.push({cmd:'clear_station',unit:u.id});
      if(u.kind==='Aircraft')orders.push({cmd:'retask_aircraft',unit:u.id,mission:{mission:'recon',point:u.pos},manual:true});
      orders.push({cmd:'set_speed',unit:u.id,speed_kts:speedKts});
    }return orders;
  }
  function routeOrders(obs,ids,point,formation=true,speedKts=null){
    if(!point||!Number.isFinite(point.x)||!Number.isFinite(point.y))return[];
    const units=members(obs,ids);if(!units.length)return[];
    const centre={x:units.reduce((s,u)=>s+u.pos.x,0)/units.length,y:units.reduce((s,u)=>s+u.pos.y,0)/units.length};
    const leader=units[0],following=formation&&units.length>1&&units.every(u=>u.kind==='Ship');
    const orders=[];
    for(const u of units){
      orders.push({cmd:'clear_station',unit:u.id});
      if(following&&u.id!==leader.id){
        const dx=u.pos.x-leader.pos.x,dy=u.pos.y-leader.pos.y;
        orders.push({cmd:'follow',unit:u.id,leader:leader.id,bearing_rel_deg:(Math.atan2(dx,dy)*180/Math.PI-leader.course_deg+720)%360,distance_nm:Math.hypot(dx,dy)});
      }else{
        const goal=following?point:{x:point.x+u.pos.x-centre.x,y:point.y+u.pos.y-centre.y};
        if(u.kind!=='Aircraft'&&Number.isFinite(speedKts)&&speedKts>0&&speedKts<=2000)orders.push({cmd:'set_speed',unit:u.id,speed_kts:speedKts});
        if(u.kind==='Aircraft')orders.push({cmd:'retask_aircraft',unit:u.id,mission:{mission:'recon',point:goal},manual:true});
        orders.push({cmd:'goto',unit:u.id,waypoints:[goal]});
      }
    }
    return orders;
  }
  const model={selected,members,routeOrders,speedOrders};
  if(typeof module!=='undefined')module.exports=model;
  if(!root.document)return;
  const $=id=>document.getElementById(id),ui=()=>root.HarpoonUI;
  const en=()=>root.HarpoonI18n?.language==='en',t=(zh,eng)=>en()?eng:zh;
  const panel=document.createElement('section');panel.id='fleet-panel';panel.className='panel fleet-panel';panel.dataset.i18nNative='';
  panel.innerHTML=`<div class="fleet-heading"><h2 id="fleet-title"></h2><span id="fleet-count"></span></div>
    <div class="fleet-modes" role="group" aria-label="海图操作 / Chart mode"><button data-fleet-mode="browse"></button><button data-fleet-mode="route"></button><button data-fleet-mode="box"></button><button id="fleet-multi" aria-pressed="false"></button></div>
    <div class="fleet-filter"><input id="fleet-search" type="search"><select id="fleet-kind"><option value=""></option><option value="Ship"></option><option value="Sub"></option><option value="Aircraft"></option></select></div>
    <div id="fleet-roster"></div>
    <div class="fleet-selection"><strong id="fleet-selection"></strong><button id="fleet-select-visible"></button><button id="fleet-clear"></button></div>
    <div class="fleet-actions"><button id="fleet-take"></button><button id="fleet-resume"></button><button id="fleet-stop"></button></div>
    <div class="fleet-orders"><label id="fleet-formation-label"><input id="fleet-formation" type="checkbox" checked><span></span></label><button id="fleet-plot"></button></div>
    <div class="fleet-speed"><label for="fleet-speed" id="fleet-speed-label"></label><input id="fleet-speed" type="number" min="0" max="2000" step="1"><button id="fleet-set-speed"></button></div>
    <p id="fleet-control-note"></p>
    <div id="fleet-draft" hidden><p id="fleet-draft-text"></p><div class="fleet-actions"><button id="fleet-confirm"></button><button id="fleet-cancel"></button></div></div>
    <details class="fleet-groups"><summary id="fleet-groups-title"></summary><div id="fleet-suggestions"></div><div class="fleet-slot"><select id="fleet-slot"></select><input id="fleet-group-name" maxlength="30"><button id="fleet-save-group"></button></div><div id="fleet-groups"></div></details>
    <p id="fleet-feedback" role="status"></p>`;
  const scroll=document.querySelector('.sidebar-scroll');scroll.prepend(panel);
  $('fleet-roster').append($('friendly-list'));
  function fold(id,zh,eng){const node=$(id);if(!node)return;const d=document.createElement('details');d.className='fleet-fold';const s=document.createElement('summary');s.dataset.zh=zh;s.dataset.en=eng;d.append(s);node.before(d);d.append(node);return d;}
  const navFold=fold('nav-controls','单舰详细操纵','Individual controls');
  fold('tactical-panel','状态机辅助设置','FSM assistance');fold('agent-panel','LLM参谋设置','LLM staff settings');
  const aiFolds=[...scroll.querySelectorAll('.fleet-fold')].filter(d=>d!==navFold);for(const d of aiFolds)scroll.append(d);
  const quick=document.querySelector('.sidebar-quick');if(quick){quick.querySelector('[data-jump="nav-controls"]').dataset.jump='fleet-panel';}
  const modeBar=document.createElement('div');modeBar.id='fleet-map-mode';modeBar.dataset.i18nNative='';modeBar.setAttribute('role','status');$('map-shell').append(modeBar);
  const choices=document.createElement('div');choices.id='fleet-unit-choices';choices.dataset.i18nNative='';choices.hidden=true;choices.setAttribute('role','group');$('map-shell').append(choices);
  const choiceIdentity=s=>JSON.stringify([s.observation?.session_revision,s.observation?.your_side,s.units,s.unit,s.contact]);let choiceKey='';
  root.addEventListener('harpoon:unit-choices',e=>{const s=snap(),d=e.detail;choiceKey=choiceIdentity(s);choices.replaceChildren();const title=document.createElement('strong');title.textContent=t('选择重叠单位','Choose overlapping unit');choices.append(title);for(const id of d.ids){const u=s.observation?.units.find(u=>Number(u.id)===id);if(!u)continue;const button=document.createElement('button');button.textContent=`${name(u)} · #${id}`;button.onclick=()=>{choices.hidden=true;ui().focusUnit(id,d.additive);};choices.append(button);}const close=document.createElement('button');close.textContent=t('取消','Cancel');close.onclick=()=>{choices.hidden=true;};choices.append(close);choices.hidden=false;const shell=$('map-shell');choices.style.left=Math.max(5,Math.min(d.x,shell.clientWidth-255))+'px';choices.style.top=Math.max(5,Math.min(d.y,shell.clientHeight-choices.offsetHeight-10))+'px';});
  root.addEventListener('harpoon:selection',()=>{if(choiceIdentity(snap())!==choiceKey)choices.hidden=true;});
  let draft=null,busy=false,groups={},groupKey='',lastRoster='',lastLanguage='',failures=[],lastSubmission=null;
  const feedback=$('fleet-feedback');
  function snap(){return ui()?.snapshot()||{units:[]};}
  function savedMember(u){return{id:Number(u.id),class:u.class,name:u.name};}
  function currentGroup(g,obs){return(g?.members||[]).filter(m=>(obs?.units||[]).some(u=>u.id===m.id&&u.class===m.class&&u.name===m.name)).map(m=>m.id);}
  function store(){try{localStorage.setItem(groupKey,JSON.stringify(groups));}catch{}}
  function setFeedback(text){feedback.textContent=text;}
  function name(u){return root.HarpoonNames?.unit(u)||u.name;}
  function visibleUnits(obs){const query=$('fleet-search').value.trim().toLowerCase(),kind=$('fleet-kind').value;return(obs?.units||[]).filter(u=>!['Missile','Torpedo'].includes(u.kind)&&(!kind||u.kind===kind)&&(!query||`${name(u)} ${u.name} ${u.class} ${u.id}`.toLowerCase().includes(query)));}
  function render(){
    const s=snap(),obs=s.observation,units=selected(obs,s.units||[]),mobile=members(obs,s.units||[]),has=units.length>0,locked=units.filter(u=>u.manual_control).length,authority=Number(obs?.control_version)>=3;
    if(lastSubmission&&lastSubmission.revision===obs?.session_revision&&obs.tick>lastSubmission.tick&&!lastSubmission.notified&&!busy){lastSubmission.notified=true;setFeedback(t(`模拟已推进至 T${obs.tick}；本批${lastSubmission.queued}条已提交，执行、拒绝或延迟以作战记录为准。`,`Simulation advanced to T${obs.tick}; ${lastSubmission.queued} submitted. Consult the log for execution, rejection or delay.`));}
    const key=`harpoon.groups:${obs?.scenario_id||''}:${obs?.your_side||''}`;
    if(key!==groupKey){groupKey=key;try{groups=JSON.parse(localStorage.getItem(key)||'{}');if(!groups||Array.isArray(groups)||typeof groups!=='object')groups={};}catch{groups={};}draft=null;}
    if(draft&&(draft.revision!==obs?.session_revision||draft.side!==obs?.your_side||JSON.stringify(draft.ids)!==JSON.stringify(s.units))){draft=null;}
    $('fleet-title').textContent=t('舰队指挥','Fleet command');$('fleet-count').textContent=t(`${(obs?.units||[]).filter(u=>!['Missile','Torpedo'].includes(u.kind)).length}个单位`,`${(obs?.units||[]).filter(u=>!['Missile','Torpedo'].includes(u.kind)).length} units`);
    const modes={browse:t('浏览','Browse'),route:t('指定航路','Plot route'),box:t('框选','Box select')};
    for(const b of panel.querySelectorAll('[data-fleet-mode]')){b.textContent=modes[b.dataset.fleetMode];b.setAttribute('aria-pressed',String(s.mode===b.dataset.fleetMode));b.disabled=busy||b.dataset.fleetMode==='route'&&(!mobile.length||!authority);}
    $('fleet-multi').textContent=t('多选','Multi-select');$('fleet-multi').setAttribute('aria-pressed',String(!!s.multi));
    $('fleet-search').placeholder=t('搜索舰名、型号或编号','Find name, class or ID');$('fleet-search').ariaLabel=t('筛选本方单位','Filter own units');
    [...$('fleet-kind').options].forEach((o,i)=>o.textContent=[t('全部','All'),t('水面舰','Ships'),t('潜艇','Submarines'),t('航空器','Aircraft')][i]);
    const visible=new Set(visibleUnits(obs).map(u=>Number(u.id)));for(const b of $('friendly-list').querySelectorAll('[data-select-unit]'))b.hidden=!visible.has(Number(b.dataset.selectUnit));
    $('fleet-selection').textContent=t(`已选${units.length} · 人工${locked}`,`${units.length} selected · ${locked} manual`);
    for(const [id,zh,eng]of [['fleet-select-visible','选中筛选结果','Select filtered'],['fleet-clear','清空','Clear'],['fleet-take','人工接管','Take control'],['fleet-resume','允许AI辅助','Allow AI'],['fleet-stop','停止','Stop'],['fleet-plot','在海图指定目的地','Choose destination'],['fleet-confirm','确认下令','Confirm order'],['fleet-cancel','取消','Cancel'],['fleet-save-group','存入编组','Save group']])$(id).textContent=t(zh,eng);
    for(const id of ['fleet-take','fleet-resume','fleet-stop','fleet-plot'])$(id).disabled=busy||!has||!!obs?.over||!authority;
    for(const id of ['fleet-stop','fleet-plot','fleet-set-speed'])$(id).disabled=busy||!mobile.length||!!obs?.over||!authority;
    $('fleet-speed-label').textContent=t('编组航速（节）','Group speed (kt)');$('fleet-set-speed').textContent=t('设置航速','Set speed');
    $('fleet-control-note').textContent=mobile.some(u=>u.kind==='Aircraft')?t('航空器改速转手动航行；低油返航继续。','Aircraft speed changes enable manual flight; fuel return remains active.'):t('单舰改速会离队；选中领舰可整队调速。','A follower alone detaches; select its leader to adjust formation speed.');
    const selectionKey=JSON.stringify(s.units);if($('fleet-speed').dataset.selection!==selectionKey){$('fleet-speed').dataset.selection=selectionKey;const values=new Set(mobile.map(u=>u.ordered_speed_kts??u.speed_kts));$('fleet-speed').value=values.size===1?[...values][0]:'';$('fleet-speed').placeholder=t('各不相同','Mixed');}
    $('fleet-confirm').disabled=busy||!authority||!!draft?.previewing||!!draft?.error;
    $('fleet-stop').textContent=units.some(u=>u.kind==='Aircraft')?t('停止 / 返航','Stop / RTB'):t('停止航行','Stop moving');
    $('fleet-formation-label').querySelector('span').textContent=t('水面舰保持队形','Keep ship formation');
    $('fleet-groups-title').textContent=t('编组与快捷键','Groups and shortcuts');
    if(!$('fleet-slot').options.length)for(let i=1;i<=9;i++)$('fleet-slot').add(new Option(String(i),String(i)));
    $('fleet-group-name').placeholder=t('编组名称','Group name');
    for(const summary of scroll.querySelectorAll('.fleet-fold>summary'))summary.textContent=en()?summary.dataset.en:summary.dataset.zh;
    const mode=s.mode||'browse';modeBar.textContent=mode==='browse'?t('浏览模式：点空白不会改令','Browse: empty chart clicks issue no orders'):mode==='box'?t('拖出矩形选择本方单位；Esc取消','Drag a box to select own units; Esc cancels'):t('点击海图选择目的地，预览后确认；Esc取消','Choose a destination, then confirm; Esc cancels');modeBar.textContent=t(`已选${units.length} · 人工${locked} ｜ `,`${units.length} selected · ${locked} manual | `)+modeBar.textContent;if(draft)modeBar.textContent=draft.error?t('航路受阻 · 尚未下令','Route blocked · no orders issued'):t(`航路草稿 · ${members(obs,draft.ids).length}个单位 · 尚未下达`, `Route draft · ${members(obs,draft.ids).length} units · not issued`);modeBar.dataset.mode=draft?'route':mode;
    $('fleet-draft').hidden=!draft;
    if(draft){const moving=members(obs,draft.ids),formation=$('fleet-formation').checked&&moving.length>1&&moving.every(u=>u.kind==='Ship');$('fleet-draft-text').textContent=t(`向 X ${draft.point.x.toFixed(1)} / Y ${draft.point.y.toFixed(1)} 下达${moving.length}个单位的机动令${units.length>moving.length?`，跳过${units.length-moving.length}个固定设施`:''}。${formation?`领舰：${name(moving[0])}，其余维持相对队位。`:'各自保持相对位置前往。'}${draft.speed?`航速${draft.speed}节（可在航速栏调整后重选目的地）。`:''}将解除旧跟随；航空器切换手动航行。`,`Move ${moving.length} units to X ${draft.point.x.toFixed(1)} / Y ${draft.point.y.toFixed(1)}. ${formation?'Leader: '+name(moving[0])+'.':'Preserve relative offsets.'} ${draft.speed?`Speed ${draft.speed} kt; adjust the speed field and plot again to change.`:``} Existing stations clear; aircraft switch to manual navigation.`);}
    const roster=JSON.stringify([groups,obs?.units?.map(u=>[u.id,u.class,u.name]),en()]);
    if(roster!==lastRoster){lastRoster=roster;$('fleet-groups').replaceChildren();for(const [slot,g]of Object.entries(groups)){const ids=currentGroup(g,obs);const b=document.createElement('button');b.textContent=`${slot} · ${g.name} (${ids.length})`;b.disabled=!ids.length;b.onclick=()=>ui().selectUnits(ids);const row=document.createElement('div');row.className='fleet-saved-row';const remove=document.createElement('button');remove.textContent=t('删除','Delete');remove.setAttribute('aria-label',t('删除编组 ','Delete group ')+g.name);remove.onclick=()=>{delete groups[slot];store();lastRoster='';render();};row.append(b,remove);$('fleet-groups').append(row);}
      const host=(obs?.units||[]).filter(u=>u.kind==='Ship'&&u.deck);$('fleet-suggestions').replaceChildren();
      for(const h of host){const ids=(obs?.units||[]).filter(u=>u.kind==='Ship').filter(u=>host.reduce((a,b)=>Math.hypot(u.pos.x-b.pos.x,u.pos.y-b.pos.y)<Math.hypot(u.pos.x-a.pos.x,u.pos.y-a.pos.y)?b:a,host[0]).id===h.id).map(u=>u.id);const b=document.createElement('button');b.textContent=t(`${name(h)}附近（${ids.length}艘）`,`Near ${name(h)} (${ids.length})`);b.title=t('仅按本方位置建议编组，可重新选择成员','Suggested from friendly positions; membership can be adjusted');b.onclick=()=>ui().selectUnits(ids);$('fleet-suggestions').append(b);}}
    if(lastLanguage!==String(en())+authority){lastLanguage=String(en())+authority;setFeedback(!authority?t('当前是旧版服务，仅可浏览编组；新版指挥请打开8822。','This older server supports group browsing only; open 8822 for the new command controls.'):t('浏览不改令；Shift点选可增减。手动命令持续接管该单位，保留现任务与基本自卫。允许AI仅解除锁定，需已开启全席托管。','Browse issues no orders; Shift-click adds/removes. Human orders retain control, standing tasks and basic self-defence. Allow AI unlocks units; a seat controller must also be enabled.'));}
  }
  async function send(orders){
    const s=snap();if(busy||!orders.length||!s.observation||s.observation.over)return;
    busy=true;failures=[];lastSubmission=null;render();let queued=0;
    try{for(const order of orders.filter(c=>c.cmd==='goto'&&c.waypoints.length))for(const point of order.waypoints)await ui().previewNavigation(order.unit,point);
      for(const order of orders){const current=snap();if(current.observation?.session_revision!==s.observation.session_revision||current.observation?.your_side!==s.observation.your_side)throw Error(t('战局或阵营已切换，停止下令','Game or side changed; remaining orders cancelled'));if(await ui().sendCommand(order,true))queued++;}
      lastSubmission={revision:s.observation.session_revision,tick:s.observation.tick,queued,notified:false};setFeedback(t(`${queued}/${orders.length}条已排队${s.speed===0?'；模拟暂停，尚未执行':''}。查看作战记录核对执行。`,`${queued}/${orders.length} queued${s.speed===0?'; paused, not executed':''}. Check the event log for actual execution.`)+(failures.length?' '+failures.slice(0,3).join(' · '):''));
    }catch(e){setFeedback(String(e.message||e));}finally{busy=false;render();}
  }
  function cancel(){draft=null;ui().setCommandMode('browse');render();}
  root.addEventListener('harpoon:command-result',e=>{if(busy&&!e.detail.ok)failures.push(`#${e.detail.command.unit}: ${e.detail.message}`);});
  panel.addEventListener('click',e=>{const b=e.target.closest('[data-fleet-mode]');if(b){draft=null;ui().setCommandMode(b.dataset.fleetMode);render();}});
  $('fleet-multi').onclick=()=>ui().setMultiSelect(!snap().multi);
  $('fleet-select-visible').onclick=()=>ui().selectUnits(visibleUnits(snap().observation).map(u=>u.id));
  $('fleet-clear').onclick=()=>ui().selectUnits([]);
  $('fleet-search').oninput=render;$('fleet-kind').onchange=render;
  $('fleet-plot').onclick=()=>{draft=null;ui().setCommandMode('route');};
  $('fleet-take').onclick=()=>send(selected(snap().observation,snap().units).map(u=>({cmd:'set_unit_automation',unit:u.id,enabled:false})));
  $('fleet-resume').onclick=()=>send(selected(snap().observation,snap().units).map(u=>({cmd:'set_unit_automation',unit:u.id,enabled:true})));
  $('fleet-set-speed').onclick=()=>{const raw=$('fleet-speed').value.trim(),value=Number(raw);if(!raw||!Number.isFinite(value)||value<0||value>2000)return setFeedback(t('请填写0～2000节的航速；空白不下令。','Enter a speed between 0 and 2000 kt; blank issues no order.'));send(speedOrders(snap().observation,snap().units,value));};
  $('fleet-stop').onclick=()=>{const s=snap(),orders=[];for(const u of members(s.observation,s.units)){if(u.kind==='Aircraft'){orders.push({cmd:'return_to_base',unit:u.id});continue;}orders.push({cmd:'clear_station',unit:u.id},{cmd:'stop',unit:u.id});}send(orders);};
  $('fleet-confirm').onclick=()=>{const s=snap();if(!draft||draft.previewing||draft.error||draft.revision!==s.observation?.session_revision||draft.side!==s.observation?.your_side)return cancel();const orders=routeOrders(s.observation,draft.ids,draft.point,$('fleet-formation').checked,draft.speed);draft=null;send(orders);};
  $('fleet-cancel').onclick=cancel;
  function saveGroup(slot){const s=snap(),units=selected(s.observation,s.units);if(!units.length)return;groups[slot]={name:$('fleet-group-name').value.trim()||t(`编组${slot}`,`Group ${slot}`),members:units.map(savedMember)};store();lastRoster='';render();}
  async function previewDraft(){const d=draft,s=snap();if(!d)return;const generation=(d.previewSeq||0)+1;d.previewSeq=generation;d.previewing=true;d.error='';d.paths=[];render();try{const orders=routeOrders(s.observation,d.ids,d.point,$('fleet-formation').checked,d.speed).filter(c=>c.cmd==='goto');for(const order of orders){const result=await ui().previewNavigation(order.unit,order.waypoints[0]);if(draft!==d||d.previewSeq!==generation)return;d.paths.push(result);}}catch(error){if(draft!==d||d.previewSeq!==generation)return;d.error=String(error.message||error);lastSubmission=null;setFeedback(t('未下令：','No orders issued: ')+d.error);}finally{if(draft===d&&d.previewSeq===generation){d.previewing=false;render();}}}
  $('fleet-formation').onchange=()=>{if(draft)previewDraft();else render();};
  $('fleet-save-group').onclick=()=>saveGroup($('fleet-slot').value);
  root.addEventListener('harpoon:map-order-point',e=>{const s=snap(),moving=members(s.observation,s.units);if(!moving.length)return;const raw=Number($('fleet-speed').value),stopped=moving.some(u=>u.kind!=='Aircraft'&&u.ordered_speed_kts===0);draft={point:e.detail,ids:[...s.units],revision:s.observation.session_revision,side:s.observation.your_side,speed:raw>0?raw:stopped?16:null};previewDraft();$('fleet-draft').scrollIntoView({block:'nearest'});});
  root.addEventListener('harpoon:cancel-map-order',cancel);
  root.addEventListener('harpoon:selection',render);root.addEventListener('harpoon:language',()=>{lastRoster='';render();});
  scroll.addEventListener('click',e=>{const jump=e.target.closest('[data-jump]');if(jump){const node=$(jump.dataset.jump);const folded=node?.closest('details');if(folded)folded.open=true;node?.scrollIntoView({block:'start'});}});
  document.addEventListener('keydown',e=>{if(e.isComposing||e.target.closest('input,select,textarea,[contenteditable="true"]')||document.querySelector('dialog[open],.operator-overlay:not([hidden])'))return;if(e.key==='Enter'&&draft&&!busy&&!e.target.closest('button,a,summary,[role="button"]')){e.preventDefault();$('fleet-confirm').click();return;}if(!(/^[1-9]$/.test(e.key)))return;if(e.ctrlKey){e.preventDefault();saveGroup(e.key);}else if(groups[e.key]){e.preventDefault();ui().selectUnits(currentGroup(groups[e.key],snap().observation));}});
  root.HarpoonFleet={...model,active:true,draw(ctx,project){if(!draft)return;const p=project(draft.point.x,draft.point.y);ctx.save();ctx.strokeStyle='#f2b660aa';ctx.lineWidth=1.4;ctx.setLineDash([5,5]);for(const path of draft.paths||[]){const unit=snap().observation?.units.find(u=>u.id===path.unit);if(!unit)continue;const start=project(unit.pos.x,unit.pos.y);ctx.beginPath();ctx.moveTo(start.x,start.y);for(const point of path.waypoints){const q=project(point.x,point.y);ctx.lineTo(q.x,q.y);}ctx.stroke();}ctx.setLineDash([]);ctx.strokeStyle='#f2b660';ctx.fillStyle='#f2b660';ctx.lineWidth=2;ctx.beginPath();ctx.arc(p.x,p.y,10,0,Math.PI*2);ctx.stroke();ctx.font='13px "Microsoft YaHei",sans-serif';ctx.fillText(draft.error?t('航路受阻，未下令','Route blocked; no orders issued'):draft.previewing?t('正在校验航路','Checking route'):t('待确认航路','Unconfirmed route'),p.x+15,p.y-8);ctx.restore();}};
  render();
})(typeof window!=='undefined'?window:globalThis);
