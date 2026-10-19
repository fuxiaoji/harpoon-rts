/* Geographic imagery and mission presentation. No enemy truth or combat constants. */
(function(root) {
  'use strict';
  const zh = () => root.HarpoonI18n?.language !== 'en';
  const text = (cn, en) => zh() ? cn : en;
  const longitudeScale = t => 60 * Math.cos(t.reference_lat * Math.PI / 180);
  const bounds = t => ({width:(t.east-t.west)*longitudeScale(t),height:(t.north-t.south)*60});
  const geographic = (p,t) => ({lon:t.west+p.x/longitudeScale(t),lat:t.south+p.y/60});
  const local = (lon,lat,t) => ({x:(lon-t.west)*longitudeScale(t),y:(lat-t.south)*60});
  const reasons = {
    mission_mutual_carrier_loss:['双方航母均失去演习规定的作战能力','Both carrier forces disabled under exercise conditions'],
    mission_defending_carriers_disabled:['中方两艘航母均失能，美方获胜','Both Chinese carriers disabled; US mission victory'],
    mission_intruding_carriers_disabled:['美方两艘航母均失能，中方获胜','Both US carriers disabled; Chinese mission victory'],
    mission_zone_held:['美方航母连续占据任务区满3分钟','A US carrier held the mission area for 3 continuous minutes'],
    mission_area_denied:['到期拒止成功，中方两艘航母保持作战能力','Area denied until the deadline; both Chinese carriers remain functional'],
    mission_denied_with_carrier_loss:['区域拒止，但中方航母受损超标：平局','Area denied with excessive Chinese carrier damage: draw'],
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
    if (obs && !obs.theater && obs.units) obs.theater = synthTheater(obs);
    const active=!!obs?.theater;
    document.body.classList.toggle('geographic-theater',active);
    document.getElementById('map-theater').hidden=!obs?.mission;
    document.getElementById('map-basemap').hidden=!active;
    document.getElementById('mission-open').hidden=!obs?.mission;
    credit.hidden=!active;
    panel.hidden=!obs?.mission;
    if (!active) return;
    for(const [id,cn,en] of [['nav-controls','编队列表','Fleet'],['orders-panel','交战命令','Engagement'],['deck-panel','飞行甲板','Flight deck']]) {
      document.querySelector(`[data-jump="${id}"]`).textContent=text(cn,en);
    }
    document.getElementById('map-theater').textContent=text('战区','Theater');
    const toggle=document.getElementById('map-basemap');
    toggle.textContent=satellite?text('卫星','Satellite'):text('网格','Grid');
    toggle.setAttribute('aria-pressed',String(satellite));
    credit.textContent=obs.theater.attribution
      ? `${obs.theater.attribution} · ${obs.theater.imagery_date} · ${text('地理参考底图','Geographic reference')}`
      : text('NASA Blue Marble 2004 · 地理参考底图','NASA Blue Marble 2004 · Geographic reference');
    credit.title=text('2004年卫星合成影像；地形不参与海岸碰撞、水深或遮蔽裁决。','2004 satellite composite; terrain does not adjudicate coastline collision, water depth or masking.');
    document.getElementById('map-imagery-status').textContent=imageState==='error'?text('影像加载失败；请切换网格','Imagery unavailable; select Grid'):imageState==='ready'?text('离线影像已载入','Offline imagery loaded'):text('正在载入离线影像','Loading offline imagery');
    if(obs.mission) {
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
    }
  }
  function drawBasemap(ctx,obs,toScreen,w,h) {
    const t=obs?.theater;
    if(!t) return;
    if(imagePath!==t.image) {
      imagePath=t.image; imageState='loading';
      const pending=image=new Image();
      pending.onload=()=>{if(image===pending){imageState='ready'; renderHUD(latest);}};
      pending.onerror=()=>{if(image===pending){imageState='error'; renderHUD(latest);}};
      pending.src=t.image;
    }
    if(!satellite || imageState!=='ready') return;
    const b=bounds(t), tl=toScreen(0,b.height), br=toScreen(b.width,0);
    ctx.save();
    ctx.drawImage(image,tl.x,tl.y,br.x-tl.x,br.y-tl.y);
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
    if(!m) return;
    const hours=(m.remaining_s+latest.tick)/3600;
    const manual=!latest.automated_side;
    root.HarpoonUI?.setSpeed(0);
    const content=document.getElementById('mission-briefing-content');
    content.innerHTML=zh()?`
      <h2>双盾行动：双航母区域拒止</h2>
      <p>${manual?'双边手操教学：顶部蓝/红按钮切换指挥席，双方自动参谋均关闭；四艘航母起始停航，等待你下令。':'你指挥中国蓝方。'}辽宁、山东各有055、两艘052D、054A、093A与舰载航空力量；美国红方由福特、尼米兹两支编队组成。本想定初始34个在海、在空单位，另有甲板待命飞机。</p>
      <h3>怎样才算赢</h3><p>守住金色圆区${hours}小时，同时让辽宁、山东各自舰体HP保持在50%以上，即中方获胜。若两艘美航母都降至50%或以下，中方提前获胜。任一可用美航母在区内连续停留180秒，或中方两航母均失能，美方获胜。拒止成功但中方仅剩一艘可用航母判平局；双方均失能也判平局。</p>
      <h3>从这里开始</h3><ol><li>保持暂停，点击右侧“CV-16 辽宁号”，再点上方“飞行甲板”。在歼-15一行点“战斗空中巡逻”放飞2架；山东也可同样操作。直-18F用于反潜、直-18J用于侦察。</li><li>点“编队列表”选一艘055或052D，再点“交战命令”。开“自动选敌”后会自行交战；选中接触再“指定目标自动交战”会持续打指定目标。“人工射击1发”只发一次。</li><li>按30倍推进搜索，交战时暂停或1倍。放飞后要经过模拟时间才会起飞，排队回执不等于战果。</li><li>${manual?'切换蓝/红席后，先选本方单位再下令。双方只通过各自探测获得接触；自动参谋关闭，航空任务和持续交战仍执行。':'美方有预设航路和规则参谋，自动放飞CAP、反潜和接触驱动的突击波；并非大模型指挥。'}</li></ol>
      ${manual?'<p>双方初始关闭自动选敌。既有巡逻、编队跟随、自卫和已指定目标仍按引擎执行；切换视角不重置战局，也不等于发现敌情。适合一人双边教学，不模拟独立AI对抗。</p>':''}
      <p class="mission-note">虚构演习，阵位、编组、挂载和50%任务阈值均为设计假设。当前RTS数据库含已声明估算；HP阈值是任务条件。卫星影像为NASA 2004年合成图，用于地理定位，不参与海岸碰撞、水深或遮蔽裁决。</p>`:`
      <h2>Twin Shields: dual-carrier area denial</h2>
      <p>${manual?'Hotseat lesson: use the Blue / Red buttons to command both sides. Both automated staffs are off. Carriers start holding position.':'Command China / Blue.'} Liaoning and Shandong each have a Type 055, two Type 052Ds, a Type 054A, a Type 093A and aviation. US / Red fields Ford and Nimitz groups. There are 34 initial deployed units plus ready deck aircraft.</p>
      <h3>Mission conditions</h3><p>China wins by denying the gold circle for ${hours} hours with both carriers above 50% hull HP, or by disabling both US carriers. The US wins if one functional carrier holds the circle for 180 consecutive seconds or both Chinese carriers are disabled. Denial with only one Chinese carrier functional is a draw. Mutual carrier loss is a draw.</p>
      <h3>First orders</h3><ol><li>Stay paused. Select CV-16 LIAONING in the unit list. In Flight Deck choose J-15 and CAP to launch two aircraft; repeat on Shandong.</li><li>Select a Type 055 or 052D. Automatic target selection permits automatic engagements. Assigning a contact creates a standing engagement order. Fire 1 Round issues one shot only.</li><li>Use 30× for search, pause or 1× for combat. Orders need simulation time to execute; a queued receipt is not a combat result.</li><li>${manual?'Switch Blue / Red seats, then select your own unit. Both staffs are off; patrol and standing engagement orders still execute.':'The US follows authored routes and observation-only rule-based CAP, ASW and strike scheduling. No LLM is controlling this game.'}</li></ol>
      ${manual?'<p>Automatic target selection initially off on both sides. Existing patrol, formation, self-defense and assigned-target orders still execute. Switching seats neither resets the battle nor creates a detection. This is a two-seat lesson, not independent AI competition.</p>':''}
      <p class="mission-note">Fictional exercise with authored positions, groups, loadouts and the 50% mission threshold. The current RTS database includes declared estimates. NASA\'s 2004 satellite composite provides geographic reference only; coastline collision, water depth and terrain masking are not adjudicated.</p>`;
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
