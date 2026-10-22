/* Independent archive viewer: reads completed archives only; never sends game commands or opens a live stream. */
(() => {
  'use strict';
  const MAX_FILE_BYTES=320*1024*1024,MAX_EXPANDED_BYTES=320*1024*1024;
  const M=window.HarpoonReplayModel, $=id=>document.getElementById(id);
  let language='zh-CN'; try{language=localStorage.getItem('harpoon.language')||language;}catch{}
  const en=()=>language==='en', t=(zh,eng)=>en()?eng:zh;
  const localName=value=>window.HarpoonNames?.text(value,language)||value;
  const localWeapon=w=>window.HarpoonNames?.name('weapon',w.weapon,w.name||w.weapon,language)||w.name||w.weapon;
  const label=value=>typeof value==='string'?value:value?.[en()?'en':'zh']||'';
  const clock=seconds=>{const n=Math.floor(seconds);return [Math.floor(n/3600),Math.floor(n/60)%60,n%60].map(v=>String(v).padStart(2,'0')).join(':');};
  const number=n=>Number.isFinite(n)?n.toFixed(1):'—';
  const kind=k=>({Ship:t('水面舰','Ship'),Aircraft:t('飞机','Aircraft'),Sub:t('潜艇','Submarine'),Missile:t('导弹','Missile'),Torpedo:t('鱼雷','Torpedo'),Base:t('基地','Base')})[k]||k;
  const mission=m=>({cap:t('战斗空巡','CAP'),asw:t('反潜搜索','ASW'),recon:t('侦察','Recon'),strike:t('突击','Strike'),rtb:t('返航','RTB')})[m]||m||t('无','None');
  const side=s=>s==='Blue'?t('蓝方','Blue'):s==='Red'?t('红方','Red'):t('上帝视角','God view');
  const winner=s=>s?side(s):t('平局','Draw');
  let replay=null, picture=null, cursor=0, view='Blue', playing=false, selected='', selectedEvent=null, catalog=[], loadGeneration=0;
  const map={x:390,y:320,scale:1,width:0,height:0,satellite:true,image:null,imageStatus:'idle',hits:[]};
  const canvas=$('replay-map'), ctx=canvas.getContext('2d');
  const keyEvents=new Set(['scenario_started','scenario_ended','impact','sunk','air_shot_down','air_launch','air_recovered','critical_hit','ditched','base_destroyed']);
  function resultReason(code){
    if(code==='exercise_objectives_achieved')return t('航母与设施目标均达标，蓝方目标分领先','Carrier/facility objectives achieved with a Blue objective lead');
    if(code==='exercise_forces_preserved')return t('红方保存与反击成功；蓝方未同时达标并领先','Red preservation/counterstrike succeeds; Blue did not meet both thresholds and lead');
    if(code==='time limit: kill count')return picture?.result?.scored?t('时限到，按加权分裁决','Time limit: weighted score'):t('时限到，按击杀数裁决','Time limit: kill count');
    if(code==='mission_zone_held'){
      const hold=picture?.mission?.config?.hold_s;
      return hold==null?t('航母完成连续占区任务','Carrier completed the continuous hold'):t(`航母连续占区${hold}秒，任务完成`,`Carrier held for ${hold} seconds; mission complete`);
    }
    return ({mission_area_denied:t('守住任务区直至时限','Area denied until deadline'),mission_intruding_carriers_disabled:t('入侵方航母失能','Intruding carriers disabled'),mission_defending_carriers_disabled:t('防守方航母失能','Defending carriers disabled'),mission_mutual_carrier_loss:t('双方航母均失能','Both carrier forces disabled'),mission_denied_with_carrier_loss:t('拒止成功但航母损失超标','Area denied with excessive carrier loss')})[code]||code;
  }
  function status(message,error=false){$('status').hidden=!message;$('status').textContent=message;$('status').dataset.error=String(error);}
  function enable(yes){for(const id of ['play','previous-frame','next-frame','previous-event','next-event','timeline','goto','export'])$(id).disabled=!yes;}
  function pause(){playing=false;$('play').textContent=t('播放','Play');$('play').setAttribute('aria-pressed','false');}
  function seek(tick){if(!replay)return;cursor=Math.max(0,Math.min(replay.end,tick));pause();selectedEvent=null;render();}
  function localize(){
    document.documentElement.lang=language;$('replay-language').value=language;
    $('map-readout').textContent='';
    document.title=t('鱼叉 · 战局复盘','Harpoon · Battle replay');
    for(const el of document.querySelectorAll('[data-zh]'))el.textContent=el.dataset[en()?'en':'zh'];
    $('event-search').placeholder=t('搜索战报','Search events');
    for(const option of $('recording').options)if(option.value)option.textContent=label(catalog.find(c=>c.id===option.value)?.title)||option.textContent;
    if(replay){$('battle-title').textContent=label(replay.title);render();}
    $('play').textContent=playing?t('暂停','Pause'):t('播放','Play');
  }
  async function readLimited(response,limit){
    const reader=response.body.getReader(), parts=[];let length=0;
    while(true){const {done,value}=await reader.read();if(done)break;length+=value.byteLength;if(length>limit){await reader.cancel();throw Error('FILE_TOO_LARGE');}parts.push(value);}
    const bytes=new Uint8Array(length);let offset=0;for(const part of parts){bytes.set(part,offset);offset+=part.byteLength;}return bytes;
  }
  async function decode(bytes){
    if(bytes.byteLength>MAX_FILE_BYTES)throw Error('FILE_TOO_LARGE');
    let raw=bytes;
    if(bytes[0]===31&&bytes[1]===139){
      if(!window.DecompressionStream)throw Error('GZIP_UNSUPPORTED');
      raw=await readLimited(new Response(new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip'))),MAX_EXPANDED_BYTES);
    }
    return M.normalize(JSON.parse(new TextDecoder().decode(raw)));
  }
  function loadError(error){
    const message=error?.message==='FILE_TOO_LARGE'?t('文件超过大小限制（原文件和解压后均最多320MiB）。','File exceeds the size limit (320MiB file and expanded data).')
      :error?.message==='GZIP_UNSUPPORTED'?t('浏览器不支持解压，请导入解压后的JSON。','This browser cannot decompress gzip; import the extracted JSON.')
      :t('无法导入：请选择有效的鱼叉复盘JSON或JSON.GZ文件。当前已打开的复盘仍保留。','Cannot import: choose a valid Harpoon replay JSON or JSON.GZ. The current replay is preserved.');
    status(message,true);
  }
  function adopt(archive){
    replay=archive;cursor=0;selected='';selectedEvent=null;pause();enable(true);$('timeline').max=replay.end;$('timeline').value=0;
    $('battle-title').textContent=label(replay.title);$('source-json').textContent=JSON.stringify(replay.source||{},null,2);
    status('');setImage();fit(false);render();
  }
  async function loadRecording(id){
    const item=catalog.find(c=>c.id===id);if(!item)return;
    const generation=++loadGeneration;pause();status(t('正在读取复盘…','Loading recording…'));
    try{
      if(!item.save_id&&!/^[a-zA-Z0-9_.-]+$/.test(item.file))throw Error('INVALID_REPLAY');
      const response=await fetch(item.save_id?'/api/saves/'+encodeURIComponent(item.save_id)+'/replay':'replays/'+item.file);if(!response.ok)throw Error('HTTP');
      const bytes=await readLimited(response,MAX_FILE_BYTES);
      if(item.sha256){const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),b=>b.toString(16).padStart(2,'0')).join('');if(hash!==item.sha256)throw Error('HASH_MISMATCH');}
      const archive=await decode(bytes);if(generation===loadGeneration)adopt(archive);
    }catch(error){if(generation===loadGeneration)loadError(error);}
  }
  async function initCatalog(){
    const params=new URLSearchParams(location.search),saved=params.get('save');
    if(params.has('current')||saved){
      try{
        const response=await fetch(saved?'/api/saves/'+encodeURIComponent(saved)+'/replay':'/api/replay/current');
        if(!response.ok)throw Error(await response.text());
        adopt(await decode(await readLimited(response,MAX_FILE_BYTES)));
        $('recording').replaceChildren(new Option(label(replay.title),'server'));
      }catch(e){status(t('无法读取终局复盘：','Cannot open completed replay: ')+e.message,true);}
      return;
    }

    const generation=loadGeneration;
    try{const results=await Promise.allSettled([fetch('replays/catalog.json').then(r=>r.ok?r.json():{replays:[]}),fetch('/api/saves',{cache:'no-store'}).then(r=>r.ok?r.json():{saves:[]})]);
      if(generation!==loadGeneration)return;
      catalog=results[0].status==='fulfilled'?results[0].value.replays||[]:[];
      if(results[1].status==='fulfilled')catalog.push(...(results[1].value.saves||[]).filter(s=>s.over&&!s.invalid).map(s=>({id:'save_'+s.id,save_id:s.id,title:s.label||s.scenario})));
      $('recording').replaceChildren();for(const item of catalog){const option=new Option(label(item.title),item.id);$('recording').add(option);}
      if(!catalog.length)throw Error('NO_CATALOG');
      const wanted=new URLSearchParams(location.search).get('replay');const first=catalog.find(c=>c.id===wanted)||catalog[0];$('recording').value=first.id;await loadRecording(first.id);
    }catch{if(replay)return;$('recording').replaceChildren(new Option(t('暂无本地档案，请导入','No recordings; import a file'),''));status(t('还没有部署复盘档案。点击“导入文件”，选择已导出的复盘JSON或JSON.GZ。','No recording is deployed. Choose Import file and select a replay JSON or JSON.GZ.'));}
  }
  function setImage(){
    $('map-credit').textContent=replay?.theater?.attribution||'NASA Blue Marble · 2004';
    $('map-credit').title=replay?.theater?.source_url||'';
    map.image=null;map.imageStatus='idle';const path=window.HarpoonBasemaps?window.HarpoonBasemaps.path(replay?.theater):replay?.theater?.image;if(!path)return;
    if(!/^maps\/[a-zA-Z0-9_.-]+\.(jpg|jpeg|png|webp)$/.test(path))return;
    const image=new Image();map.image=image;map.imageStatus='loading';
    image.onload=()=>{if(map.image===image){map.imageStatus='ready';draw();}};
    image.onerror=()=>{if(map.image===image){map.imageStatus='error';draw();}};image.src=path;
  }
  function fit(theater){
    if(!replay)return;
    const p=M.project(replay,cursor,view), positions=p.units.filter(u=>!['Missile','Torpedo'].includes(u.kind)).map(u=>u.pos).concat(p.contacts.map(c=>c.estimate));
    if(theater&&replay.theater){const b=replay.theater,w=(b.east-b.west)*60*Math.cos(b.reference_lat*Math.PI/180),h=(b.north-b.south)*60;map.x=w/2;map.y=h/2;map.scale=Math.min(map.width/(w*1.05),map.height/(h*1.05));}
    else if(positions.length){const xs=positions.map(p=>p.x),ys=positions.map(p=>p.y),minX=Math.min(...xs),maxX=Math.max(...xs),minY=Math.min(...ys),maxY=Math.max(...ys);map.x=(minX+maxX)/2;map.y=(minY+maxY)/2;map.scale=Math.min(map.width/Math.max(140,(maxX-minX)*1.35),map.height/Math.max(130,(maxY-minY)*1.6));}
    else{map.x=390;map.y=320;map.scale=1;}
    map.scale=Math.max(.08,Math.min(30,map.scale));draw();
  }
  const screen=p=>({x:map.width/2+(p.x-map.x)*map.scale,y:map.height/2-(p.y-map.y)*map.scale});
  const world=p=>({x:map.x+(p.x-map.width/2)/map.scale,y:map.y-(p.y-map.height/2)/map.scale});
  function zoom(factor,anchor={x:map.width/2,y:map.height/2}){const before=world(anchor);map.scale=Math.max(.08,Math.min(replay?.theater?.tile_layer?256:30,map.scale*factor));const after=world(anchor);map.x+=before.x-after.x;map.y+=before.y-after.y;draw();}
  function mark(point,kindName,color,estimated,course,active){
    ctx.save();ctx.translate(point.x,point.y);ctx.rotate((Number(course)||0)*Math.PI/180);ctx.strokeStyle=color;ctx.fillStyle=estimated?'#091e2c':color;ctx.lineWidth=active?2.5:1.5;
    const size=['Missile','Torpedo'].includes(kindName)?3:6;ctx.beginPath();
    if(kindName==='Aircraft'){ctx.moveTo(0,-size-2);ctx.lineTo(size,size);ctx.lineTo(0,size/2);ctx.lineTo(-size,size);ctx.closePath();}
    else if(estimated||kindName==='Sub'){ctx.moveTo(0,-size);ctx.lineTo(size,0);ctx.lineTo(0,size);ctx.lineTo(-size,0);ctx.closePath();}
    else ctx.rect(-size*.65,-size,size*1.3,size*2);
    ctx.fill();ctx.stroke();ctx.restore();
    if(active){ctx.beginPath();ctx.strokeStyle='#fff';ctx.lineWidth=1;ctx.arc(point.x,point.y,13,0,Math.PI*2);ctx.stroke();}
  }
  function draw(){
    ctx.clearRect(0,0,map.width,map.height);ctx.fillStyle='#071a27';ctx.fillRect(0,0,map.width,map.height);map.hits=[];
    if(!replay||!picture)return;
    const theater=replay.theater;
    if(theater&&map.satellite&&map.imageStatus==='ready'){
      const w=(theater.east-theater.west)*60*Math.cos(theater.reference_lat*Math.PI/180),h=(theater.north-theater.south)*60;
      const a=screen({x:0,y:h}),b=screen({x:w,y:0});ctx.drawImage(map.image,a.x,a.y,b.x-a.x,b.y-a.y);ctx.fillStyle='#05192340';ctx.fillRect(0,0,map.width,map.height);
    }
    if(theater&&map.satellite){
      const tiles=window.HarpoonSatelliteTiles?.draw(ctx,theater,(x,y)=>screen({x,y}),map.width,map.height,draw);
      if(tiles){
        const credit=tiles.credit+' · CC BY 4.0 · NASA offline fallback';
        $('map-credit').textContent=credit;$('map-credit').title='https://cloudless.eox.at/license-non-commercial';
        let overlay=$('map-imagery-credit');
        if(!overlay){overlay=document.createElement('a');overlay.id='map-imagery-credit';overlay.href='https://cloudless.eox.at/license-non-commercial';overlay.target='_blank';overlay.rel='noopener';canvas.parentElement.append(overlay);}
        overlay.textContent=credit;overlay.hidden=false;
      }
    }
    if($('map-imagery-credit')&&(!theater?.tile_layer||!map.satellite))$('map-imagery-credit').hidden=true;
    window.HarpoonNavigation?.paint(ctx,replay.terrain,(x,y)=>screen({x,y}));
    window.HarpoonCoastlines?.draw(ctx,replay.frames.Blue[0],(x,y)=>screen({x,y}),map.width,map.height,replay.terrain);
    const spacing=map.scale>3?10:map.scale>1?25:100, low=world({x:0,y:map.height}),high=world({x:map.width,y:0});
    ctx.strokeStyle='#b1d0e51c';ctx.lineWidth=1;ctx.beginPath();
    for(let x=Math.floor(low.x/spacing)*spacing;x<high.x;x+=spacing){const a=screen({x,y:0});ctx.moveTo(a.x,0);ctx.lineTo(a.x,map.height);}
    for(let y=Math.floor(low.y/spacing)*spacing;y<high.y;y+=spacing){const a=screen({x:0,y});ctx.moveTo(0,a.y);ctx.lineTo(map.width,a.y);}ctx.stroke();
    const m=picture.mission?.config;
    if(m){const p=screen(m.center);ctx.beginPath();ctx.arc(p.x,p.y,m.radius_nm*map.scale,0,Math.PI*2);ctx.fillStyle='#f2b66014';ctx.strokeStyle='#f2b660';ctx.lineWidth=1.5;ctx.setLineDash([8,6]);ctx.fill();ctx.stroke();ctx.setLineDash([]);ctx.fillStyle='#ffe0a7';ctx.font='12px "Microsoft YaHei",sans-serif';ctx.textAlign='center';ctx.fillText(t('拒止任务区','Mission area'),p.x,p.y-m.radius_nm*map.scale-12);}
    if(selected&&$('trails').checked){const isContact=selected.startsWith('c:'),id=Number(selected.split(':')[1]),points=M.trail(replay,cursor,view,id,isContact);ctx.beginPath();ctx.strokeStyle=isContact?'#f2b66099':'#dcecf399';ctx.lineWidth=1.5;points.forEach((p,i)=>{const q=screen(p);if(i)ctx.lineTo(q.x,q.y);else ctx.moveTo(q.x,q.y);});ctx.stroke();}
    for(const unit of picture.units){const p=screen(unit.pos),key='u:'+unit.id,color=unit.side==='Blue'?'#4bcaff':'#ff7162';mark(p,unit.kind,color,false,unit.course_deg,key===selected);map.hits.push({...p,key});
      if(key===selected||/^CV[N-]/.test(unit.name)){ctx.font='600 11px "Microsoft YaHei",sans-serif';ctx.textAlign='left';ctx.lineWidth=4;ctx.strokeStyle='#05131d';ctx.strokeText(localName(unit.name),p.x+12,p.y-6);ctx.fillStyle=color;ctx.fillText(localName(unit.name),p.x+12,p.y-6);}}
    for(const contact of picture.contacts){const p=screen(contact.estimate),key='c:'+contact.id;ctx.globalAlpha=contact.stale ? .45 : 1;mark(p,contact.category,'#f2b660',true,contact.est_course_deg,key===selected);map.hits.push({...p,key});if(key===selected){ctx.fillStyle='#f2b660';ctx.textAlign='left';ctx.font='12px sans-serif';ctx.fillText('C'+contact.id,p.x+14,p.y-5);}ctx.globalAlpha=1;}
    let nm=50;while(nm*map.scale>130)nm/=2;while(nm*map.scale<55)nm*=2;ctx.strokeStyle='#dcecf3';ctx.lineWidth=2;ctx.beginPath();ctx.moveTo(map.width-25-nm*map.scale,map.height-27);ctx.lineTo(map.width-25,map.height-27);ctx.stroke();ctx.fillStyle='#dcecf3';ctx.font='11px sans-serif';ctx.textAlign='right';ctx.fillText(`${nm} ${t('海里','nm')}`,map.width-25,map.height-35);
    if(map.imageStatus==='error'){$('map-readout').textContent=t('影像不可用，已保留网格海图','Imagery unavailable; grid map retained');}
    canvas.dataset.view=view;canvas.dataset.units=String(picture.units.length);canvas.dataset.contacts=String(picture.contacts.length);
  }
  function optionList(){
    const entries=[new Option(t('点击海图或选择单位','Click map or choose unit'),'')];
    for(const u of picture.units)entries.push(new Option(`${side(u.side)} · ${localName(u.name)}`,'u:'+u.id));
    for(const c of picture.contacts)entries.push(new Option(`C${c.id} · ${localName(c.class_name)||kind(c.category)}${c.stale?t('（陈旧）',' (stale)'):''}`,'c:'+c.id));
    if(!entries.some(o=>o.value===selected))selected='';
    $('unit-select').replaceChildren(...entries);$('unit-select').value=selected;
  }
  function addText(parent,tag,text,className){const el=document.createElement(tag);el.textContent=text;if(className)el.className=className;parent.append(el);return el;}
  function details(){
    const box=$('unit-detail');box.replaceChildren();
    const id=Number(selected.split(':')[1]),isContact=selected.startsWith('c:');
    const u=(isContact?picture.contacts:picture.units).find(u=>u.id===id);
    if(!u){addText(box,'p',t('选择单位查看当时的航速、舰体、弹药与任务。','Select a unit for its recorded speed, hull, ammunition and mission.'));return;}
    addText(box,'strong',isContact?`C${u.id} · ${localName(u.class_name)||kind(u.category)}`:localName(u.name));
    const dl=document.createElement('dl');box.append(dl);
    const row=(name,value)=>{addText(dl,'dt',name);addText(dl,'dd',value);};
    if(isContact){row(t('接触性质','Track'),t('敌方位置估计，非真态','Enemy estimate, not truth'));row(t('质量','Quality'),number(u.quality*100)+'%');row(t('上次更新距记录时刻','Age at sample'),number(u.last_seen_s)+t('秒','s'));row(t('航速估计','Estimated speed'),number(u.est_speed_kts)+t('节',' kt'));row(t('状态','Status'),u.stale?t('陈旧 / 已失联','Stale / lost'):t('跟踪中','Tracked'));}
    else{row(t('阵营 / 类别','Side / type'),`${side(u.side)} / ${kind(u.kind)}`);row(t('记录时间','Recorded at'),clock(picture.snapshots[u.side]));row(t('舰体 / 耐久','Hull / durability'),`${number(u.hp)} / ${number(u.hp_max)}`);row(t('实际 / 命令航速','Actual / ordered speed'),`${number(u.speed_kts)} / ${number(u.ordered_speed_kts)} ${t('节','kt')}`);row(t('航向','Course'),number(u.course_deg)+'°');row(t('任务','Mission'),mission(u.mission));row(t('自动选敌','Auto target selection'),u.fire_on?t('已授权','Enabled'):t('关闭','Off'));row(t('持续目标','Standing target'),u.assigned==null?t('无','None'):'C'+u.assigned);
      if(u.weapons?.length){addText(box,'p',t('当时弹药','Recorded ammunition'));const list=document.createElement('ul');box.append(list);for(const w of u.weapons)addText(list,'li',`${localWeapon(w)} × ${w.ammo}`);}}
  }
  function eventText(e){
    const who=localName(e.unit_name||e.shooter_name)||(e.unit!=null?`#${e.unit}`:''), contact=e.contact==null?'':`C${e.contact}`, air=mission(e.mission);
    const names={scenario_started:t('对局开始','Battle started'),scenario_ended:t('任务终局','Mission ended'),impact:t('命中','Impact'),weapons_release:t('武器已发射','Weapon launched'),sam_shot:t('防空导弹发射','SAM launched'),incoming:t('来袭警报','Incoming'),intercept:e.intercepted?t('拦截成功','Intercept succeeded'):t('拦截未成','Intercept failed'),detection:t('发现接触','Contact detected'),contact_identified:t('接触识别','Contact identified'),contact_lost:t('接触失联','Contact lost'),contact_sank:t('接触确认沉没','Contact sank'),chaff_deployed:t('释放箔条','Chaff deployed'),decoy_fooled:t('诱偏成功','Decoy succeeded'),counter_detected:t('己方辐射被发现','Counter-detected'),gun_action:t('炮击','Gun action'),critical_hit:t('关键系统损伤','Critical damage'),sunk:t('单位损失','Unit lost'),ditched:t('飞机迫降损失','Aircraft ditched'),air_shot_down:t('飞机被击落','Aircraft shot down'),air_launch:t('放飞飞机','Aircraft launched'),air_recovered:t('飞机回收','Aircraft recovered'),rules_event:t('规则处理','Rules processing')};
    let title=names[e.event]||e.event,detail=[who,contact,localWeapon({weapon:e.weapon})].filter(Boolean).join(' · ');
    if(e.event==='impact')detail=`${who} · ${localWeapon({weapon:e.weapon})} · ${t('损伤','damage')} ${number(e.damage)} · ${t('剩余','remaining')} ${number(e.hp_left)}`;
    if(e.event==='air_launch')detail=`#${e.carrier} · ${e.class} × ${e.count} · ${air}`;
    if(e.event==='air_recovered')detail=`#${e.carrier} · ${e.class} × ${e.count}`;
    if(e.event==='contact_identified')detail=`${contact} · ${localName(e.class_name)}`;
    if(e.event==='rules_event'){const report=window.HarpoonCombat.ruleReport(e,language);title=report.title;detail=[report.meaning,report.detail].filter(Boolean).join(' · ');}
    if(e.event==='scenario_ended')detail=`${winner(e.winner)} · ${resultReason(e.reason)}`;
    return {title,detail};
  }
  function filteredEvents(events){const f=$('event-filter').value,search=$('event-search').value.trim().toLowerCase();return events.filter(e=>f==='all'||f==='key'&&(keyEvents.has(e.event)||e.event==='rules_event'&&['tactical_ai','llm_ai'].includes(e.domain))||f==='impact'&&['impact','sunk','air_shot_down','critical_hit','ditched'].includes(e.event)||f==='ai'&&e.event==='rules_event'&&['tactical_ai','llm_ai'].includes(e.domain)||f==='air'&&e.event.startsWith('air_')||f==='detection'&&['detection','contact_identified','contact_lost','contact_sank'].includes(e.event)).filter(e=>{const s=eventText(e);return !search||`${s.title} ${s.detail} ${e.seq}`.toLowerCase().includes(search);});}
  function events(){
    if($('event-filter').value==='orders'){
      const box=$('events'),search=$('event-search').value.trim().toLowerCase();box.replaceChildren();
      const names={set_course:'航向',set_speed:'航速',set_depth:'潜深',set_altitude:'高度',goto:'航路',stop:'停车',follow:'跟随',clear_station:'解除跟随',set_sonar:'声纳',set_weapons_free:'自动选敌',set_auto_chaff:'自动箔条',assign_target:'持续指派',clear_target:'解除目标',fire:'射击',deploy_chaff:'释放箔条',launch_aircraft:'起飞',recover_aircraft:'整批召回',return_to_base:'单机返航'};
      const orders=picture.orders.filter(o=>!search||JSON.stringify(o).toLowerCase().includes(search));
      $('event-count').textContent=t(`${orders.length}条本方/裁判命令`,`${orders.length} visible orders`);
      for(const o of orders.slice(-300).reverse()){
        const d=document.createElement('details');box.append(d);
        addText(d,'summary',`${clock(o.tick)} · ${side(o.side)} · #${o.command.unit} · ${en()?o.command.cmd:names[o.command.cmd]||o.command.cmd}`);
        addText(d,'p',t('这是下达记录；执行效果请核对发射、回收和判伤事件。','This is a submitted order; check launch, recovery and damage events for execution.'));
        addText(d,'pre',JSON.stringify(o.command,null,2));
      }
      return;
    }
    const box=$('events'),matches=filteredEvents(picture.events);box.replaceChildren();$('event-count').textContent=t(`${picture.events.length}条可见`,`${picture.events.length} visible`);
    if(selectedEvent&&picture.events.some(e=>e.seq===selectedEvent.seq)){
      const info=eventText(selectedEvent),panel=document.createElement('div');panel.id='event-detail';box.append(panel);addText(panel,'strong',`${clock(selectedEvent.tick)} · ${info.title}`);addText(panel,'p',info.detail);const note=window.HarpoonCombat.eventNote(selectedEvent,language);if(note)addText(panel,'p',note);
      const details=document.createElement('details');panel.append(details);addText(details,'summary',t('原始裁决记录','Original adjudication record'));addText(details,'pre',JSON.stringify(selectedEvent,null,2));
    }
    if(!matches.length){addText(box,'p',t('这个时刻还没有符合筛选条件的可见战报。可拖动时间轴或点击“下一事件”。','No matching visible events yet. Move the timeline or choose Next event.'));return;}
    if(matches.length>120)addText(box,'p',t(`共${matches.length}条匹配，显示最近120条；可缩小筛选范围。`,`${matches.length} matches; latest 120 shown. Narrow the filter for older events.`));
    for(const e of matches.slice(-120).reverse()){
      const button=document.createElement('button'),info=eventText(e);button.type='button';button.dataset.seq=e.seq;button.dataset.tick=e.tick;
      addText(button,'time',clock(e.tick));addText(button,'span',' '+info.title);addText(button,'small',info.detail);
      button.addEventListener('click',()=>{seek(e.tick);selectedEvent=e;events();});box.append(button);
    }
  }
  function render(){
    if(!replay)return;picture=M.project(replay,cursor,view);$('map-empty').hidden=true;
    $('clock').textContent=`${clock(cursor)} / ${clock(replay.end)}`;$('timeline').value=Math.floor(cursor);
    $('archive-summary').textContent=t(`真实观察回放 · ${replay.events.length}条事件 · 地图记录最大间隔${replay.source?.max_sample_gap_s??'—'}秒`, `Recorded observations · ${replay.events.length} events · maximum map sample gap ${replay.source?.max_sample_gap_s??'—'}s`);
    $('view-note').textContent=view==='God'?t('双方己方真态；仅供复盘','Both sides’ recorded truth; replay only'):t('己方真态 + 当时接触估计','Own truth + contemporary contact estimates');
    for(const b of $('perspectives').querySelectorAll('button'))b.setAttribute('aria-pressed',String(b.dataset.view===view));
    const samples=(view==='God'?['Blue','Red']:[view]).map(s=>`${side(s)} ${picture.snapshots[s]===null?t('暂无记录','no sample'):clock(picture.snapshots[s])+t(`（距游标${Math.floor(cursor-picture.snapshots[s])}秒）`,` (${Math.floor(cursor-picture.snapshots[s])}s behind)`)} `);
    $('sampling-note').textContent=t('地图记录：','Map samples: ')+samples.join(' · ')+t('；保持最近样本，不补算运动。','; last sample held, no movement interpolation.');
    $('picture-count').textContent=t(`${picture.units.length}单位 / ${picture.contacts.length}接触`,`${picture.units.length} units / ${picture.contacts.length} contacts`);
    optionList();details();events();$('bookmarks').replaceChildren();
    for(const b of replay.bookmarks||[]){if(!b.views?.includes(view))continue;const button=document.createElement('button');addText(button,'time',clock(b.tick));addText(button,'span',label(b.label));button.dataset.bookmark=b.tick;button.onclick=()=>seek(b.tick);$('bookmarks').append(button);}
    $('event-markers').replaceChildren();for(const e of replay.events.filter(e=>['impact','scenario_ended'].includes(e.event)&&M.allowed(e,view))){const marker=document.createElement('i');marker.style.left=(e.tick/Math.max(1,replay.end)*100)+'%';$('event-markers').append(marker);}
    const result=picture.result;
    const score=result?(result.scored?(String(result.reason||'').startsWith('exercise_')?t(`演习目标分 蓝${result.blue_points} : 红${result.red_points}`,`Objective score Blue ${result.blue_points} : Red ${result.red_points}`):t(`加权分 蓝${result.blue_points} : 红${result.red_points}`,`Weighted score Blue ${result.blue_points} : Red ${result.red_points}`)):t(`击杀 蓝${result.blue_kills??'—'} : 红${result.red_kills??'—'}`,`Kills Blue ${result.blue_kills??'—'} : Red ${result.red_kills??'—'}`)):'';
    $('outcome').textContent=result?`${winner(result.winner)}${result.winner?t('获胜',' victory'):''} · ${score} · ${resultReason(result.reason)}`:t('空格播放/暂停；方向键逐记录；海图滚轮缩放、拖动平移。','Space: play/pause · arrows: samples · wheel: zoom · drag: pan');
    draw();
  }
  $('perspectives').onclick=e=>{const button=e.target.closest('[data-view]');if(!button)return;view=button.dataset.view;selected='';selectedEvent=null;render();};
  $('recording').onchange=e=>void loadRecording(e.target.value);
  $('import').onclick=()=>$('import-file').click();
  $('import-file').onchange=async e=>{const file=e.target.files?.[0];if(!file)return;const generation=++loadGeneration;pause();try{if(file.size>MAX_FILE_BYTES)throw Error('FILE_TOO_LARGE');const archive=await decode(new Uint8Array(await file.arrayBuffer()));if(generation===loadGeneration){adopt(archive);let option=$('recording').querySelector('[value="__imported"]');if(!option){option=new Option('','__imported');$('recording').add(option);}option.dataset.zh='已导入：'+file.name;option.dataset.en='Imported: '+file.name;option.textContent=option.dataset[en()?'en':'zh'];$('recording').value='__imported';status(t('文件已导入；可随时导出并再次打开。','Imported; export to reopen it later.'));}}catch(error){loadError(error);}finally{e.target.value='';}};
  $('export').onclick=()=>{if(!replay)return;const url=URL.createObjectURL(new Blob([JSON.stringify(M.serializable(replay))],{type:'application/json'}));const a=document.createElement('a');a.href=url;a.download=(String(replay.id||'battle').replace(/[^a-zA-Z0-9_-]/g,'_'))+'.replay.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);};
  $('replay-language').onchange=e=>{language=e.target.value;try{localStorage.setItem('harpoon.language',language);}catch{}localize();};
  function playToggle(){if(!replay)return;if(playing){pause();return;}if(cursor>=replay.end)cursor=0;playing=true;$('play').textContent=t('暂停','Pause');$('play').setAttribute('aria-pressed','true');render();}
  $('play').onclick=playToggle;
  $('timeline').oninput=e=>seek(Number(e.target.value));
  $('previous-frame').onclick=()=>replay&&seek(M.neighbor(replay,cursor,-1));$('next-frame').onclick=()=>replay&&seek(M.neighbor(replay,cursor,1));
  function eventStep(direction){if(!replay)return;const list=filteredEvents(replay.events.filter(e=>M.allowed(e,view)));const event=direction>0?list.find(e=>e.tick>cursor):[...list].reverse().find(e=>e.tick<cursor);if(event){seek(event.tick);selectedEvent=event;events();}}
  $('previous-event').onclick=()=>eventStep(-1);$('next-event').onclick=()=>eventStep(1);
  $('event-filter').onchange=()=>replay&&events();$('event-search').oninput=()=>replay&&events();
  $('unit-select').onchange=e=>{selected=e.target.value;details();draw();};
  $('unit-select').addEventListener('pointerdown',pause);
  $('events').addEventListener('pointerdown',pause);
  $('goto').onclick=()=>{const value=$('goto-time').value.trim(),match=value.match(/^(\d{1,4}):([0-5]\d):([0-5]\d)$/);if(!match){status(t('请输入 hh:mm:ss，例如 00:17:58。','Enter hh:mm:ss, for example 00:17:58.'),true);return;}status('');seek(Number(match[1])*3600+Number(match[2])*60+Number(match[3]));};
  $('goto-time').onkeydown=e=>{if(e.key==='Enter')$('goto').click();};
  $('fit').onclick=()=>fit(false);$('theater').onclick=()=>fit(true);$('zoom-in').onclick=()=>zoom(1.3);$('zoom-out').onclick=()=>zoom(1/1.3);$('satellite').onclick=()=>{map.satellite=!map.satellite;$('satellite').setAttribute('aria-pressed',String(map.satellite));draw();};$('trails').onchange=draw;
  $('source-info').onclick=()=>$('source-dialog').showModal();
  let drag=null;
  const local=e=>{const r=canvas.getBoundingClientRect();return{x:e.clientX-r.left,y:e.clientY-r.top};};
  canvas.addEventListener('wheel',e=>{e.preventDefault();zoom(e.deltaY<0?1.12:1/1.12,local(e));},{passive:false});
  canvas.onpointerdown=e=>{if(e.button!==0)return;const p=local(e);drag={...p,start:p,moved:false};canvas.setPointerCapture(e.pointerId);};
  canvas.onpointermove=e=>{const p=local(e);if(drag){if(Math.hypot(p.x-drag.start.x,p.y-drag.start.y)>4)drag.moved=true;map.x-=(p.x-drag.x)/map.scale;map.y+=(p.y-drag.y)/map.scale;drag.x=p.x;drag.y=p.y;draw();}else{const w=world(p);$('map-readout').textContent=`X ${w.x.toFixed(1)} / Y ${w.y.toFixed(1)} ${t('海里','nm')}`;}};
  canvas.onpointerup=e=>{if(!drag)return;const p=local(e);if(!drag.moved){const hit=map.hits.map(h=>({...h,d:Math.hypot(h.x-p.x,h.y-p.y)})).filter(h=>h.d<16).sort((a,b)=>a.d-b.d)[0];selected=hit?.key||'';if(picture){$('unit-select').value=selected;details();draw();}}drag=null;canvas.releasePointerCapture(e.pointerId);};canvas.onpointercancel=()=>{drag=null;};
  document.addEventListener('keydown',e=>{if($('source-dialog').open||/INPUT|SELECT|TEXTAREA|BUTTON|A/.test(e.target.tagName)||!replay)return;if(e.code==='Space'){e.preventDefault();playToggle();}if(e.code==='ArrowRight'){e.preventDefault();seek(M.neighbor(replay,cursor,1));}if(e.code==='ArrowLeft'){e.preventDefault();seek(M.neighbor(replay,cursor,-1));}});
  function resize(){const rect=$('map-area').getBoundingClientRect();map.width=rect.width;map.height=rect.height;const dpr=Math.min(2,devicePixelRatio||1);canvas.width=Math.round(rect.width*dpr);canvas.height=Math.round(rect.height*dpr);ctx.setTransform(dpr,0,0,dpr,0,0);draw();}
  new ResizeObserver(resize).observe($('map-area'));resize();localize();void initCatalog();
  let last=performance.now(),lastRender=0;
  function animation(now){const delta=Math.min((now-last)/1000,.5);last=now;if(playing&&replay){cursor=Math.min(replay.end,cursor+delta*Number($('speed').value));if(now-lastRender>100||cursor>=replay.end){render();lastRender=now;}if(cursor>=replay.end)pause();}requestAnimationFrame(animation);}
  requestAnimationFrame(animation);
})();
