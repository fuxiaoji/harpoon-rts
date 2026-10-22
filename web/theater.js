/* Geographic imagery and mission presentation. No enemy truth or combat constants. */
(function(root) {
  'use strict';
  const zh = () => root.HarpoonI18n?.language !== 'en';
  const text = (cn, en) => zh() ? cn : en;
  const durationLabel = seconds => {
    const s = Math.max(1, Number(seconds) || 0);
    if (s >= 3600) {
      const h = s / 3600;
      return zh() ? `${Number.isInteger(h) ? h.toFixed(0) : h.toFixed(1)} 小时` : `${Number.isInteger(h) ? h.toFixed(0) : h.toFixed(1)} h`;
    }
    const m = Math.max(1, Math.round(s / 60));
    return zh() ? `${m} 分钟` : `${m} min`;
  };
  const longitudeScale = t => 60 * Math.cos(t.reference_lat * Math.PI / 180);
  const bounds = t => ({width:(t.east-t.west)*longitudeScale(t),height:(t.north-t.south)*60});
  const geographic = (p,t) => ({lon:t.west+p.x/longitudeScale(t),lat:t.south+p.y/60});
  const local = (lon,lat,t) => ({x:(lon-t.west)*longitudeScale(t),y:(lat-t.south)*60});
  const reasons = {
    exercise_objectives_achieved:['航母与设施目标均达标，且蓝方目标分领先','Carrier and facility objectives achieved with a Blue objective-point lead'],
    exercise_forces_preserved:['红方保存与反击成功，蓝方未同时满足目标及领先条件','Red preservation/counterstrike succeeds; Blue did not meet both objective thresholds and lead'],
    mission_mutual_carrier_loss:['双方航母均失去演习规定的作战能力','Both carrier forces disabled under exercise conditions'],
    mission_defending_carriers_disabled:['中方两艘航母均失能，美方获胜','Both Chinese carriers disabled; US mission victory'],
    mission_intruding_carriers_disabled:['美方两艘航母均失能，中方获胜','Both US carriers disabled; Chinese mission victory'],
    mission_zone_held:['美方航母连续占据任务区满3分钟','A US carrier held the mission area for 3 continuous minutes'],
    mission_area_denied:['到期拒止成功，中方两艘航母保持作战能力','Area denied until the deadline; both Chinese carriers remain functional'],
    mission_denied_with_carrier_loss:['区域拒止，但中方航母受损超标：平局','Area denied with excessive Chinese carrier damage: draw'],
    'time limit: kill count':['击杀数决胜','Kill-count decision'],
    'Blue forces destroyed':['红方全歼蓝方','Red destroyed all Blue forces'],
    'Red forces destroyed':['蓝方全歼红方','Blue destroyed all Red forces'],
    'mutual annihilation':['双方同归于尽：平局','Mutual annihilation: draw'],
    'VP margin reached':['VP差额达到提前胜利条件','VP margin reached'],
  };
  const reason = code => reasons[code]?.[zh()?0:1] || code;
  let latest, callbacks, image, imagePath='', imageState='idle', satellite=true;
  // 没有自带战区元数据的想定：按当前单位分布合成一个默认战区（经纬度
  // 字段，内部按琉球纬度 26°N 换算成海里），让卫星参考底图对全部想定可用。
  let synthTheaterCache=null, synthFor=null;
  function synthTheater(obs) {
    if (!obs) return null;
    const key = obs.scenario_id || obs.scenario_name;
    if (synthFor === key) return synthTheaterCache;
    const pts = [...(obs.units||[]), ...(obs.contacts||[])]
      .map(e=>e.pos||e.estimate).filter(Boolean);
    if (!pts.length) return null;
    let minX=Infinity,minY=Infinity,maxX=-Infinity,maxY=-Infinity;
    for (const p of pts){minX=Math.min(minX,p.x);maxX=Math.max(maxX,p.x);minY=Math.min(minY,p.y);maxY=Math.max(maxY,p.y);}
    const m = Math.max(40,(maxX-minX)*0.15);
    const x0 = Math.min(0, minX-m), y0 = Math.min(0, minY-m);
    const wNm = Math.max(320,(maxX+m)-x0), hNm = Math.max(260,(maxY+m)-y0);
    const DEG_LON_PER_NM = 1/(60*0.899), DEG_LAT_PER_NM = 1/60; // 26°N
    synthFor = key;
    synthTheaterCache = { synthetic:true, reference_lat:26,
      image:'maps/west-pacific-blue-marble.jpg',
      west: x0*DEG_LON_PER_NM, east: (x0+wNm)*DEG_LON_PER_NM,
      south: y0*DEG_LAT_PER_NM, north: (y0+hNm)*DEG_LAT_PER_NM };
    return synthTheaterCache;
  }
  function renderHUD(obs) {
    latest=obs;
    if (typeof document==='undefined') return;
    const panel=document.getElementById('mission-brief');
    const credit=document.getElementById('map-attribution');
    // A fictional local grid has no real-world geographic origin. Never
    // stretch an unrelated satellite image over it or mutate its observation.
    const active=!!obs?.theater;
    const killCount=obs?.victory?.mode==='kill_count';
    const hasBrief=!!(obs?.mission || obs?.exercise || killCount);
    document.body.classList.toggle('geographic-theater',active);
    document.getElementById('map-theater').hidden=!obs?.mission;
    document.getElementById('map-basemap').hidden=!active;
    document.getElementById('mission-open').hidden=!hasBrief;
    credit.hidden=!active;
    panel.hidden=!hasBrief;
    if (!active) return;
    for(const [id,cn,en] of [['nav-controls','编队列表','Fleet'],['orders-panel','交战命令','Engagement'],['deck-panel','飞行甲板','Flight deck']]) {
      const jump=document.querySelector(`[data-jump="${id}"]`);
      if(jump)jump.textContent=text(cn,en);
    }
    document.getElementById('map-theater').textContent=text('战区','Theater');
    const toggle=document.getElementById('map-basemap');
    toggle.textContent=satellite?text('卫星','Satellite'):text('网格','Grid');
    toggle.setAttribute('aria-pressed',String(satellite));
    credit.textContent=obs.theater.tile_layer===root.HarpoonSatelliteTiles?.layer ? root.HarpoonSatelliteTiles.credit+' · CC BY 4.0 · NASA offline fallback' : obs.theater.attribution
      ? `${obs.theater.attribution} · ${obs.theater.imagery_date} · ${text('地理参考底图','Geographic reference')}`
      : text('NASA Blue Marble 2004 · 地理参考底图','NASA Blue Marble 2004 · Geographic reference');
    credit.title=text('卫星拼接影像；独立海岸矢量负责避陆。点击查看影像来源与授权。影像不提供水深、高程或实时情报。','Satellite composite; separate coast vectors govern land avoidance. Open imagery sources and license. No depth, elevation or live intelligence.');
    credit.onclick=()=>root.open(obs.theater.tile_layer?'https://cloudless.eox.at/license-non-commercial':obs.theater.source_url,'_blank','noopener');
    document.getElementById('map-imagery-status').textContent=imageState==='error'?text('影像加载失败；请切换网格','Imagery unavailable; select Grid'):imageState==='ready'?text('离线影像已载入','Offline imagery loaded'):text('正在载入离线影像','Loading offline imagery');
    if(obs.exercise) {
      const e=obs.exercise,remaining=obs.victory?.remaining_s||0;
      document.getElementById('mission-clock').textContent=`${text('任务剩余','Remaining')} ${Math.floor(remaining/3600)}:${String(Math.floor(remaining/60)%60).padStart(2,'0')}:${String(remaining%60).padStart(2,'0')}`;
      document.getElementById('mission-own').textContent=e.locked?text('主动指挥锁定 · 日常巡逻继续','Active orders locked · routine patrol continues'):text('主动指挥可用','Active orders available');
      document.getElementById('mission-objective').textContent=text(`蓝方须取得航母损伤分≥${e.required_carrier_points}、设施损伤分≥${e.required_infrastructure_points}且总分领先；红方保存力量并反击。`,`Blue needs carrier damage ≥${e.required_carrier_points}, facility damage ≥${e.required_infrastructure_points} and a point lead; Red preserves forces and counterstrikes.`);
      document.getElementById('mission-ai').textContent=text('未知敌方损伤不补算；战前资料为一次快照。设施/部署为虚构，底图与岸线为地理参考。','Unknown enemy damage stays unknown; briefing is one snapshot. Facilities/deployment are fictional; imagery/coast are geographic reference.');
      document.getElementById('mission-open').textContent=text('演习简报与计分','Exercise briefing and scoring');
    }else if(obs.mission) {
      const m=obs.mission, remaining=m.remaining_s;
      const clock=`${String(Math.floor(remaining/3600)).padStart(2,'0')}:${String(Math.floor(remaining/60)%60).padStart(2,'0')}:${String(remaining%60).padStart(2,'0')}`;
      document.getElementById('mission-clock').textContent=`${text('任务剩余','Remaining')} ${clock}`;
      document.getElementById('mission-own').textContent=`${text('本方可用航母','Own functional carriers')} ${m.own_functional}/${m.own_total}`;
      document.getElementById('mission-objective').textContent=obs.your_side===m.config.defender
        ?text(`守住金色圆区；保护辽宁、山东。美舰持续入圈${m.config.hold_s}秒即失败。`,`Deny the gold circle; protect Liaoning and Shandong. A US carrier holding for ${m.config.hold_s}s defeats denial.`)
        :text(`任意一艘航母连续入圈${m.config.hold_s}秒。当前本方持续 ${m.own_hold_s||0} 秒。`,`Hold with one carrier for ${m.config.hold_s} seconds. Own continuous hold: ${m.own_hold_s||0}s.`);
      document.getElementById('mission-ai').textContent=obs.automated_side
        ?text('美方规则参谋已启用；暂停时不行动','US rule-based staff enabled; inactive while paused')
        :text('双边手操；自动参谋关闭；已下达的任务继续执行','Hotseat command; automated staff off; standing tasks still execute');
      document.getElementById('mission-open').textContent=text('任务简报与操作顺序','Briefing and first orders');
    } else if(killCount) {
      const v=obs.victory, remaining=v.remaining_s;
      const clock=`${String(Math.floor(remaining/3600)).padStart(2,'0')}:${String(Math.floor(remaining/60)%60).padStart(2,'0')}:${String(remaining%60).padStart(2,'0')}`;
      document.getElementById('mission-clock').textContent=`${text('任务剩余','Remaining')} ${clock}`;
      document.getElementById('mission-own').textContent=text(`确认击杀 ${v.own_confirmed_kills} · 损失 ${v.own_losses}`,`Confirmed kills ${v.own_confirmed_kills} · losses ${v.own_losses}`);
      const span=durationLabel(v.duration_s);
      const w=v.kill_weights;
      const weightText=w
        ? text(`分值：舰船 ${w.ship} · 潜艇 ${w.sub} · 飞机 ${w.aircraft} · 基地 ${w.base}。`,`Kill values: ship ${w.ship}, sub ${w.sub}, aircraft ${w.aircraft}, base ${w.base}. `)
        : '';
      document.getElementById('mission-objective').textContent=text(`到 ${span} 时击毁敌方平台更多的一方获胜；相等为平局。导弹/鱼雷不计，燃油迫降不计为敌方击杀。`,`At ${span}, the side with more destroyed enemy platforms wins; equal counts draw. Missiles/torpedoes do not count, and fuel ditching is not an enemy kill.`)+weightText;
      document.getElementById('mission-ai').textContent=text('右侧 AI 参谋可启用并对话；开启“允许下令”后可由 AI 下达单位命令。','Enable the AI staff on the right; with “Allow orders” it can issue unit commands.');
      document.getElementById('mission-open').textContent=text('任务简报与操作顺序','Briefing and first orders');
    }
  }
  function drawBasemap(ctx,obs,toScreen,w,h) {
    const t=obs?.theater;
    if(!t) return;
    const path=root.HarpoonBasemaps?root.HarpoonBasemaps.path(t):t.image;
    if(!path)return;
    if(imagePath!==path) {
      imagePath=path; imageState='loading';
      const pending=image=new Image();
      pending.onload=()=>{if(image===pending){imageState='ready'; renderHUD(latest);}};
      pending.onerror=()=>{if(image===pending){imageState='error'; renderHUD(latest);}};
      pending.src=path;
    }
    if(!satellite) return;
    const b=bounds(t), tl=toScreen(0,b.height), br=toScreen(b.width,0);
    ctx.save();
    if(imageState==='ready')ctx.drawImage(image,tl.x,tl.y,br.x-tl.x,br.y-tl.y);
    const tiles=root.HarpoonSatelliteTiles?.draw(ctx,t,toScreen,w,h,()=>renderHUD(latest));
    if(tiles)document.getElementById('map-imagery-status').textContent=tiles.ready===tiles.total&&tiles.total
      ? text(`全球卫星瓦片已载入 · L${tiles.zoom}`,`Global satellite tiles ready · L${tiles.zoom}`)
      : tiles.errors?text('卫星瓦片不可用，显示离线底图','Satellite tiles unavailable; offline fallback')
      :text(`卫星瓦片 ${tiles.ready}/${tiles.total} · 离线底图兜底`,`Satellite tiles ${tiles.ready}/${tiles.total} · offline fallback`);
    ctx.fillStyle='rgba(6,20,30,0.08)';ctx.fillRect(0,0,w,h);
    ctx.restore();
  }
  function drawOverlay(ctx,obs,toScreen,scale) {
    const m=obs?.mission;
    if(!m) return;
    const p=toScreen(m.config.center.x,m.config.center.y), r=m.config.radius_nm*scale;
    ctx.save();ctx.fillStyle='rgba(244,193,91,0.09)';ctx.strokeStyle='#f4c15b';ctx.lineWidth=2;
    ctx.setLineDash([9,6]);ctx.beginPath();ctx.arc(p.x,p.y,r,0,2*Math.PI);ctx.fill();ctx.stroke();ctx.setLineDash([]);
    ctx.fillStyle='#f4c15b';ctx.font='600 13px "Microsoft YaHei",sans-serif';ctx.textAlign='center';
    ctx.fillText(text(`拒止任务区 · ${m.config.radius_nm}海里`,`Denial area · ${m.config.radius_nm} nm`),p.x,p.y-r-14);
    if(obs.theater) {
      ctx.font='12px "Microsoft YaHei",sans-serif';ctx.fillStyle='#c7dbd4';
      for(const [lon,lat,cn,en] of [[120.8,23.7,'台湾岛','Taiwan'],[127.65,26.55,'冲绳岛','Okinawa'],[121.1,18.45,'吕宋岛','Luzon']]) {
        const pos=local(lon,lat,obs.theater), q=toScreen(pos.x,pos.y);
        ctx.strokeStyle='#06141f';ctx.lineWidth=4;ctx.strokeText(text(cn,en),q.x,q.y);ctx.fillText(text(cn,en),q.x,q.y);
      }
    }
    ctx.restore();
  }
  function showBriefing() {
    const m=latest?.mission;
    const v=latest?.victory;
    const e=latest?.exercise;
    const w=v?.kill_weights;
    const killCount=!m && v?.mode==='kill_count';
    if(!m && !killCount && !e) return;
    root.HarpoonUI?.setSpeed(0);
    const content=document.getElementById('mission-briefing-content');
    if(e) {
      content.innerHTML=text(
        '<h2>东岛警报 · 虚构设施演习</h2><p>真实台海及琉球地域底图；机场、港口、部署、兵力与预警条件均为游戏假设。</p><h3>任务与计分</h3><p>六小时。蓝方航母损伤目标共60分，机场与港口共40分。须取得至少30分航母损伤、20分设施损伤，并超过红方保存力量及反击得分。红方保存价值为100减蓝方造成的目标损伤，另可获最多20分反击分。击落飞机不直接增加目标分。</p><h3>警戒与情报</h3><p>红方首次由本方传感器发现敌方导弹，或已遭敌对命中时，开放主动命令。此前日常巡逻、传感器及系统自卫继续运行。另设虚构360秒航空战备转换：各红方飞行甲板留2架即用防空机，其余库存待转换结束；这不是现实战备数据。蓝方开局获得港内全部舰艇及设施的一次可瞄准定位快照，后续不会获得隐藏真态；北斗仅是导航授时背景，不是成像侦察。</p><h3>兵力与边界</h3><p>蓝方两支航母编队及两个独立水面分队，共26艘水面舰、8艘核潜艇；6个虚构航空站含浙江/上海方向支援，共144架岸基飞机（含30架H-6J代理）；6个巡航弹单位共192弹。红方有两支起始停泊的航母编队、6艘核潜艇、3个虚构机场、2个港口与日常执勤飞机。潜艇起始距敌水面舰至少80海里。机场/港口沿用明确标注的RTS通用代理；没有推算现实基地结构、跑道程序、港口水深或弹道导弹。</p><h3>本席已知目标状态</h3><div data-exercise-targets></div>',
        '<h2>Island Alert · fictional exercise</h2><p>Real regional geography with fictional facilities, deployments, force sizes and alert conditions.</p><h3>Objectives</h3><p>Six hours. Blue can earn60 carrier-damage and40 facility-damage points. It needs at least30 carrier and20 facility points and a lead over Red. Red scores preserved value (100 minus Blue target damage) plus up to20 counterstrike points. Aircraft kills do not directly score.</p><h3>Alert and information</h3><p>Red active orders unlock on its own observed hostile missile or a hostile hit. Routine flight, sensors and system self-defence continue. An authored360-second preparation period leaves two ready AAM aircraft per Red air host; remaining inventory prepares. This is a game assumption. Blue receives one initial targeting-quality snapshot of moored ships and facilities, never live hidden truth. BeiDou is navigation/timing background, not imaging reconnaissance.</p><h3>Forces and limits</h3><p>Blue: two carrier groups and two independent surface groups (26 surface ships), eight nuclear submarines, six fictional air stations including Zhejiang/Shanghai sectors with144 shore aircraft (30 H-6J proxies), and192 cruise rounds in six batteries. Red: two initially moored carrier groups, six nuclear submarines, three fictional air stations, two ports and routine aircraft. Submarines start at least80nm from hostile ships. Installations use labelled generic RTS proxies; no real facility layouts, detailed runway procedures, harbor depth or ballistic missiles are modeled.</p><h3>Known objective status</h3><div data-exercise-targets></div>');
      const targets=content.querySelector('[data-exercise-targets]');
      if(['taiwan_island_opening','island_joint_exercise'].includes(latest.scenario_id)) {
        const opening=document.createElement('p');opening.className='mission-note';
        opening.textContent=text('空中开场：蓝方64架飞机（48突击、16护航）与64枚导弹已在途。它们从上述联队和弹库扣除，并消耗此前飞行时间/油量；这些是虚构演习初态，非开局额外增援。','Hot opening:64 Blue aircraft (48 strike,16 escort) and64 missiles are already in flight, deducted from the above wings and magazines with elapsed fuel/time charged. This is an authored initial state.');
        content.insertBefore(opening,content.children[2]);
      }
      for(const g of e.naval_groups||[]) {
        const row=document.createElement('p');
        row.textContent=text(`本方编组 ${g.name} · 旗舰 #${g.leader} · ${g.members.length}艘；独立编组不自动并入其他航母。`,`Own group ${g.name} · leader #${g.leader} · ${g.members.length} ships; independent groups retain their own leader.`);
        targets.appendChild(row);
      }
      for(const g of e.joint_groups||[]) {
        const row=document.createElement('p');row.textContent=text(`本席联合任务组 ${g.name} · ${g.members.length}个初始宿主/平台；后续出动、弹耗与复核见任务队列。`,`Own service group ${g.name} · ${g.members.length} initial hosts/platforms; task queue shows later sorties, expenditure and reviews.`);targets.append(row);
      }
      for(const o of e.objectives||[]) {
        const row=document.createElement('p');
        const hp=o.hp_fraction===null||o.hp_fraction===undefined?text('未知/无新估损','unknown/no new BDA'):`${Math.round(o.hp_fraction*100)}% HP`;
        row.textContent=`${o.name}${o.contact_id===undefined?'':` · C${o.contact_id}`} · ${o.points} ${text('分','points')} · ${hp}`;
        targets.appendChild(row);
      }
      document.getElementById('mission-close').textContent=text('开始指挥','Take command');
      document.getElementById('mission-dialog').showModal();
      return;
    }else if(killCount) {
      const span=durationLabel(v.duration_s);
      const ownKills=v.own_confirmed_kills, ownLosses=v.own_losses;
      const title=root.HarpoonI18n?.scenario(latest?.scenario_id)?.title || latest?.scenario_name || text('击杀数对决','Kill duel');
      content.innerHTML=zh()?`
        <h2>${title}</h2>
        <p>辽宁、山东对福特、尼米兹，${span}虚构演习。区域拒止圆区已取消，双方航母都从停航开始。胜负只看击毁敌方平台数量。</p>
        <h3>怎样才算赢</h3><p>到 ${span} 时，加权击杀分更高的一方获胜；分数相同为平局。${w?`分值：舰船 ${w.ship}、潜艇 ${w.sub}、飞机 ${w.aircraft}、基地 ${w.base}；`:'舰艇、潜艇、飞机、基地各计 1；'}导弹和鱼雷不计；飞机燃油迫降不计为敌方击杀。当前本方确认击杀 ${ownKills}${w&&v.own_kill_points?`（加权 ${v.own_kill_points} 分）`:''}，己方损失 ${ownLosses}。敌方真实损失在终局前不公开。</p>
        <h3>AI 参谋</h3><p>右侧 “AI 参谋” 面板可以启用、提问、要求立即汇报；开启“允许下令”后，参谋会把合法命令直接送入权威引擎。每 5 分钟模拟时间自动汇报一次，记忆会跨汇报保留。想要观战两名 AI 完整对局，可用服务端 <code>/api/agent/duel</code>，双方使用各自的双盲观察。</p>
        <h3>从这里开始</h3><ol><li>保持暂停，先选“AI 参谋”，点“启用”并勾选“允许下令”；也可以先提问“你的首轮计划是什么？”</li><li>参谋会建议航母东进、AEW 前出、战斗机巡逻和护航舰自动选敌。命令排队后需要推进模拟时间才生效。</li><li>用 30× 或更低倍速推进；每 300 模拟秒（5 分钟）参谋主动汇报。暂停时不会自动汇报，可点“立即汇报”。</li><li>想自己接管时，取消“允许下令”，在编队列表、交战命令和飞行甲板正常操作。AI 记忆和最近简报仍可查看。</li></ol>
        <p class="mission-note">虚构演习，阵位、编组和挂载为设计假设；当前 RTS 数据库含已声明估算。卫星影像仅用于地理参考，海岸规则使用独立矢量数据；水深和航空地形遮蔽尚未实现。</p>`:`
        <h2>${title}</h2>
        <p>Liaoning and Shandong against Ford and Nimitz in a ${span} fictional exercise. The area-denial circle is removed and both carrier groups start stopped. Only destroyed enemy platforms decide the result.</p>
        <h3>Victory</h3><p>At ${span}, the higher weighted kill score wins; equal scores draw. ${w?`Kill values: ship ${w.ship}, submarine ${w.sub}, aircraft ${w.aircraft}, base ${w.base}; `:'Ships, submarines, aircraft and bases each count 1; '}missiles and torpedoes do not count; aircraft lost to fuel are not enemy kills. Own confirmed kills ${ownKills}${w&&v.own_kill_points?` (weighted ${v.own_kill_points} points)`:''}, own losses ${ownLosses}. The enemy's true losses stay hidden until the end.</p>
        <h3>AI staff</h3><p>The AI STAFF panel on the right can be enabled, queried, or asked for an immediate report. With “Allow orders” enabled, valid commands go straight into the authoritative engine. It reports every five simulated minutes and keeps memory across reports. Two staff officers can play a full match through the server's <code>/api/agent/duel</code>, each using only its own double-blind observation.</p>
        <h3>First orders</h3><ol><li>Stay paused. In AI STAFF click Enable and check Allow orders; or ask “What is your first plan?”</li><li>The staff will recommend moving the carriers east, pushing AEW forward, setting CAP, and letting escorts auto-engage. Queued orders need simulated time to execute.</li><li>Use 30× or slower. It reports every 300 simulated seconds (5 minutes). It does not report while paused; use Report now.</li><li>To take over, uncheck Allow orders and use the normal unit, engagement and flight-deck controls. Memory and recent reports remain visible.</li></ol>
        <p class="mission-note">Fictional exercise with authored positions, groups and loadouts. The current RTS database includes declared estimates. Satellite imagery is geographic reference only; charted coast rules use separate vector data; bathymetry and airborne terrain masking remain unmodelled.</p>`;
      document.getElementById('mission-close').textContent=text('接管指挥，保持暂停','Take command, remain paused');
      document.getElementById('mission-dialog').showModal();
      return;
    }
    const hours=(m.remaining_s+latest.tick)/3600;
    const manual=!latest.automated_side;
    content.innerHTML=zh()?`
      <h2>双盾行动：双航母区域拒止</h2>
      <p>${manual?'双边手操教学：顶部蓝/红按钮切换指挥席，双方自动参谋均关闭；四艘航母起始停航，等待你下令。':'你指挥中国蓝方。'}辽宁、山东各有055、两艘052D、054A、093A与舰载航空力量；美国红方由福特、尼米兹两支编队组成。本想定初始34个在海、在空单位，另有甲板待命飞机。</p>
      <h3>怎样才算赢</h3><p>守住金色圆区${hours}小时，同时让辽宁、山东各自舰体HP保持在50%以上，即中方获胜。若两艘美航母都降至50%或以下，中方提前获胜。任一可用美航母在区内连续停留180秒，或中方两航母均失能，美方获胜。拒止成功但中方仅剩一艘可用航母判平局；双方均失能也判平局。</p>
      <h3>从这里开始</h3><ol><li>保持暂停，点击右侧“CV-16 辽宁号”，再点上方“飞行甲板”。在歼-15一行点“战斗空中巡逻”放飞2架；山东也可同样操作。直-18F用于反潜、直-18J用于侦察。</li><li>点“编队列表”选一艘055或052D，再点“交战命令”。开“自动选敌”后会自行交战；选中接触再“指定目标自动交战”会持续打指定目标。“人工射击1发”只发一次。</li><li>按30倍推进搜索，交战时暂停或1倍。放飞后要经过模拟时间才会起飞，排队回执不等于战果。</li><li>${manual?'切换蓝/红席后，先选本方单位再下令。双方只通过各自探测获得接触；自动参谋关闭，航空任务和持续交战仍执行。':'美方有预设航路和规则参谋，自动放飞CAP、反潜和接触驱动的突击波；并非大模型指挥。'}</li></ol>
      ${manual?'<p>双方初始关闭自动选敌。既有巡逻、编队跟随、自卫和已指定目标仍按引擎执行；切换视角不重置战局，也不等于发现敌情。适合一人双边教学，不模拟独立AI对抗。</p>':''}
      <p class="mission-note">虚构演习，阵位、编组、挂载和50%任务阈值均为设计假设。当前RTS数据库含已声明估算；HP阈值是任务条件。卫星影像为NASA 2004年合成图，用于地理定位，海岸规则使用独立矢量数据；水深和航空地形遮蔽尚未实现。</p>`:`
      <h2>Twin Shields: dual-carrier area denial</h2>
      <p>${manual?'Hotseat lesson: use the Blue / Red buttons to command both sides. Both automated staffs are off. Carriers start holding position.':'Command China / Blue.'} Liaoning and Shandong each have a Type 055, two Type 052Ds, a Type 054A, a Type 093A and aviation. US / Red fields Ford and Nimitz groups. There are 34 initial deployed units plus ready deck aircraft.</p>
      <h3>Mission conditions</h3><p>China wins by denying the gold circle for ${hours} hours with both carriers above 50% hull HP, or by disabling both US carriers. The US wins if one functional carrier holds the circle for 180 consecutive seconds or both Chinese carriers are disabled. Denial with only one Chinese carrier functional is a draw. Mutual carrier loss is a draw.</p>
      <h3>First orders</h3><ol><li>Stay paused. Select CV-16 LIAONING in the unit list. In Flight Deck choose J-15 and CAP to launch two aircraft; repeat on Shandong.</li><li>Select a Type 055 or 052D. Automatic target selection permits automatic engagements. Assigning a contact creates a standing engagement order. Fire 1 Round issues one shot only.</li><li>Use 30× for search, pause or 1× for combat. Orders need simulation time to execute; a queued receipt is not a combat result.</li><li>${manual?'Switch Blue / Red seats, then select your own unit. Both staffs are off; patrol and standing engagement orders still execute.':'The US follows authored routes and observation-only rule-based CAP, ASW and strike scheduling. No LLM is controlling this game.'}</li></ol>
      ${manual?'<p>Automatic target selection initially off on both sides. Existing patrol, formation, self-defense and assigned-target orders still execute. Switching seats neither resets the battle nor creates a detection. This is a two-seat lesson, not independent AI competition.</p>':''}
      <p class="mission-note">Fictional exercise with authored positions, groups, loadouts and the 50% mission threshold. The current RTS database includes declared estimates. NASA\'s 2004 satellite composite provides geographic reference only; charted coast rules use separate vector data; bathymetry and airborne terrain masking remain unmodelled.</p>`;
    document.getElementById('mission-close').textContent=text('接管指挥，保持暂停','Take command, remain paused');
    document.getElementById('mission-dialog').showModal();
  }
  function attach(handlers) {
    callbacks=handlers;
    document.getElementById('map-theater').onclick=()=>callbacks.fitTheater();
    document.getElementById('map-basemap').onclick=()=>{satellite=!satellite;renderHUD(latest);};
    document.getElementById('mission-open').onclick=showBriefing;
    document.getElementById('mission-close').onclick=()=>document.getElementById('mission-dialog').close();
  }
  const api={bounds,geographic,local,reason,renderHUD,drawBasemap,drawOverlay,attach};
  if(typeof module!=='undefined') module.exports=api;
  root.HarpoonTheater=api;
})(typeof window!=='undefined'?window:globalThis);
