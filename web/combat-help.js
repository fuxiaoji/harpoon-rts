/* Player explanations of the current RTS implementation, never adjudication. */
(function (root) {
  'use strict';
  const pair = (zh,en) => ({zh,en});
  const rules = {
    air_burst: pair('近程拦截破片波及','Fragments after a close intercept'),
    tdl_share: pair('己方战术数据链共享','Friendly tactical data sharing'),
    cross_fix: pair('多个方位交叉定位','Cross-fix from passive bearings'),
    bol_launch: pair('低质量接触应急发射','Emergency launch on a poor track'),
    friendly_fire: pair('误伤友方','Friendly fire'),
    fire_out: pair('火灾已扑灭','Fire extinguished'),
    flooding_contained: pair('进水已控制','Flooding contained'),
    system_repair: pair('受损系统恢复','Damaged systems repaired'),
    towed_array_deployed: pair('拖曳声纳已展开','Towed sonar deployed'),
    towed_array_retracted: pair('拖曳声纳已收回','Towed sonar retracted'),
    battery_low: pair('潜艇电量不足','Submarine battery low'),
    sonobuoy_laid: pair('声纳浮标已布放','Sonobuoy deployed'),
    sonobuoy_expired: pair('声纳浮标已失效','Sonobuoy expired'),
    dogfight_merge: pair('航空器进入近距缠斗','Aircraft enter a dogfight'),
    dogfight_pass: pair('缠斗交错攻击判定','Dogfight firing pass'),
    dogfight_hit: pair('缠斗攻击命中','Dogfight hit'),
    dogfight_end: pair('航空器脱离缠斗','Aircraft disengages'),
    runway_degraded: pair('跑道受损影响航空作业','Runway damage slows operations'),
    wake_detected: pair('尾流自导鱼雷捕获尾迹','Wake-homing torpedo acquires a wake'),
    critical_ignored: pair('重创条目不适用，已忽略','Critical row not applicable'),
    location_light_crit: pair('舰艏或舰艉附加轻微重创','Additional bow or stern light critical'),
  };
  const meanings = {
    air_burst: pair('来袭武器已在近处被拦截，但爆炸破片仍波及被保护单位；不等于整枚导弹直接命中。','The incoming weapon was intercepted nearby, but fragments affected the defended unit. This is not a direct missile impact.'),
    tdl_share: pair('己方平台正在交换接触情报。这是情报更新，不是发射或命中；共享情报也可能降质或延迟。','Friendly platforms are exchanging tracks. This is an intelligence update, not a launch or hit; shared tracks may be delayed or degraded.'),
    cross_fix: pair('多处被动方位被组合用于定位，原本只有方向的接触获得了位置估计。','Several passive bearings were combined to estimate a position for a bearing-only track.'),
    bol_launch: pair('对低质量接触执行了本RTS实现的应急发射；该分支采用降低命中率的即时结算改编。','An emergency launch used the RTS adaptation for a poor-quality track, with reduced probability and immediate adjudication.'),
    friendly_fire: pair('本次交战产生友方附带损伤，请检查友军与交战区的位置。','This engagement caused friendly collateral damage. Check friendly positions around the engagement.'),
    fire_out: pair('损管行动扑灭了一处火灾，不代表已经恢复此前损失的全部舰体或系统。','Damage control extinguished a fire; previous hull and system losses are not necessarily repaired.'),
    flooding_contained: pair('损管行动控制了进水，不代表舰体损失已全部恢复。','Damage control contained flooding; previous hull damage is not necessarily repaired.'),
    system_repair: pair('记录中列出的受损系统得到修复；系统恢复与舰体耐久是不同指标。','The listed damaged systems recovered. System condition and hull durability are separate measures.'),
    towed_array_deployed: pair('满足当前引擎的低速与部署时间条件后，拖曳阵列自动投入工作。','The towed array deployed after the engine’s low-speed and deployment-time conditions were met.'),
    towed_array_retracted: pair('航速超过当前引擎的收回条件，拖曳阵列被收回。','The towed array retracted after speed exceeded the engine’s operating condition.'),
    battery_low: pair('常规潜艇电池进入低电量状态，当前最大航速受到限制。','A conventional submarine has low battery and a reduced maximum speed.'),
    sonobuoy_laid: pair('己方航空器布放了有工作寿命的声纳浮标，以维持水下探测。','A friendly aircraft placed a finite-life sonobuoy to support underwater detection.'),
    sonobuoy_expired: pair('这枚声纳浮标已超过工作寿命，不能再继续提供探测。','This sonobuoy has reached the end of its working life and no longer provides detection.'),
    dogfight_merge: pair('航空器进入近距交战状态，后续交错攻击由现有缠斗机制推进。','Aircraft entered close combat; subsequent firing passes are advanced by the dogfight subsystem.'),
    dogfight_pass: pair('这是一次交错攻击尝试；命中与否以本条数值和后续命中记录为准。','This is one firing-pass attempt; its result is given by this record and any following hit report.'),
    dogfight_hit: pair('一次近距空战攻击实际造成了损伤。','A close-air-combat attack caused actual damage.'),
    dogfight_end: pair('航空器结束当前缠斗，不代表已经返航或回收。','The aircraft left its current dogfight; it has not necessarily returned or landed.'),
    runway_degraded: pair('机场受损降低了跑道可用性，航空作业周转会变慢。','Airfield damage reduced runway operability and slowed aircraft turnaround.'),
    wake_detected: pair('尾流自导鱼雷识别到舰船尾迹，开始沿尾流搜索或跟踪。','A wake-homing torpedo acquired a ship wake for its search or tracking process.'),
    critical_ignored: pair('掷出的重创涉及本单位没有安装的系统，因此未对该系统施加损伤。','The rolled critical refers to a system this unit does not carry, so that system damage was ignored.'),
    location_light_crit: pair('本RTS按命中部位执行了一次附加轻微重创；这是已声明的估算改编。','The RTS applied an extra light critical for hit location, using its declared estimated adaptation.'),
  };
  const pick=(value,lang)=>value?.[lang==='en'?'en':'zh'] || '';
  function firingState(unit,lang='zh-CN') {
    if (!unit) return pick(pair('先选择己方单位，查看它的开火方式。','Select a friendly unit to inspect its firing mode.'),lang);
    const assigned=unit.assigned!=null;
    return pick(assigned
      ? pair(`指定 C${unit.assigned} 持续自动交战${unit.fire_on?'；失去指派后仍可自动选敌':''}`,`Continuously engaging assigned C${unit.assigned}${unit.fire_on?'; free-target fallback remains on':''}`)
      : unit.fire_on ? pair('自动选敌开火：会自行寻找接触交战','Automatic target selection and fire enabled')
      : pair('人工攻击：当前没有指定目标，也没有自动选敌','Manual attack: no assigned target or automatic selection'),lang);
  }
  function effectZh(effect) {
    const replacements=[
      [/^weapon mount rounds (\d+) -> (\d+) \(half\)$/,'一座武器座可用弹药从 $1 减至 $2（减半）'],
      [/^sensor \((radar|sonar)\) -50% -> ([\d.]+)$/,(_,s,n)=>`${s==='radar'?'雷达':'声纳'}能力降低50个百分点，剩余比例 ${n}`],
      [/^(engineering: mobility|CIC: fire control|bridge: fire control|rudder: mobility) -([\d.]+) -> ([\d.]+)$/,(_,s,n,left)=>`${({'engineering: mobility':'轮机损伤：机动能力','CIC: fire control':'作战中心损伤：火控能力','bridge: fire control':'舰桥损伤：火控能力','rudder: mobility':'舵机损伤：机动能力'})[s]}降低 ${n}，剩余比例 ${left}`],
      [/^fire \+(\d+)s -> ([\d.]+)s$/,'火灾持续时间增加 $1 秒，累计 $2 秒'],
      [/^flooding \+(\d+)s -> ([\d.]+)s$/,'进水持续时间增加 $1 秒，累计 $2 秒'],
      [/^cargo\/superstructure; no modeled effect$/,'命中货舱或上层建筑；当前模型未施加额外效果'],
      [/^weapon mount \(no armed mount\); no effect$/,'没有可受影响的有弹武器座，未产生额外效果'],
    ];
    for(const [pattern,value] of replacements) if(pattern.test(effect)) return effect.replace(pattern,value);
    return null;
  }
  function traceZh(raw) {
    const s=String(raw||'').replace(/\s+/g,' ').trim(); let match;
    if((match=s.match(/^PD-band intercept at ([\d.]+) nmi; fragment strike chance ([\d.]+)%; airburst critical d20=(\d+): (.+)$/))) {
      const effect=effectZh(match[4]);
      return effect ? `拦截点距被保护单位 ${match[1]} 海里；破片波及概率 ${match[2]}%；D20（20面骰）结果 ${match[3]}。${effect}。` : null;
    }
    const patterns=[
      [/^TDL net (\d+) unit\(s\): (\d+) track\(s\) relayed \(q×0.9\), (\d+) delayed to non-TDL units \(q×0.5\), (\d+) track\(s\) re-shared$/,'数据链网络有 $1 个单位；转发 $2 条接触（质量乘0.9），向无数据链单位延迟转发 $3 条（质量乘0.5），重新共享 $4 条。这里的“质量”不是命中率。'],
      [/^passive bearing cross-fix \((\d+)\+ units\); quality ([\d.]+)->([\d.]+), bearing_only cleared$/,'至少 $1 个单位参与方位交叉定位；情报质量从 $2 提升至 $3，接触已不再仅有方位。'],
      [/^damage control extinguished the fire on (.+) \(dc_progress ([\d.]+)\)$/,'$1 的火灾已扑灭；损管进度累计 $2。'],
      [/^damage control contained flooding on (.+) \(dc_progress ([\d.]+)\)$/,'$1 的进水已控制；损管进度累计 $2。'],
      [/^damage control restored (.+) on (.+); systems now mobility ([\d.]+), radar ([\d.]+), sonar ([\d.]+), fire_control ([\d.]+)$/,(_,systems,name,m,r,s,f)=>`${name} 修复了 ${systems.replace(/mobility/g,'机动').replace(/fire_control/g,'火控').replace(/radar/g,'雷达').replace(/sonar/g,'声纳')}；当前能力比例：机动 ${m}、雷达 ${r}、声纳 ${s}、火控 ${f}。1.00表示完整能力，不是舰体耐久。`],
      [/^battery ([\d.]+)% below ([\d.]+)%; max speed halved to ([\d.]+) kt$/,'电池剩余 $1%，低于 $2% 阈值；最大航速减半至 $3 节。'],
      [/^towed array streamed after ([\d.]+) s below ([\d.]+) kt \(speed ([\d.]+) kt\)$/,'持续 $1 秒低于 $2 节后，拖曳声纳展开；当时航速 $3 节。'],
      [/^towed array recovered: speed ([\d.]+) kt above ([\d.]+) kt$/,'航速 $1 节超过 $2 节的收回条件，拖曳声纳收回。'],
      [/^(.+) merges with #(\d+) at ([\d.]+) nmi$/,'$1 在 $3 海里距离与航空器 #$2 进入近距缠斗。'],
      [/^pass (\d+): (.+) hits (.+) for ([\d.]+) damage; target hp ([\d.]+)\/([\d.]+)$/,'第 $1 次交错：$2 命中 $3，造成 $4 损伤；目标剩余耐久 $5 / $6。'],
      [/^(.+) disengages after (\d+) passes \((.+)\)$/,'$1 在 $2 次交错后脱离缠斗；具体退出条件见原始记录。'],
      [/^sonobuoy #(\d+) laid at \(([-\d.]+), ([-\d.]+)\) on contact (.+) quality ([\d.]+); expires t=(\d+)$/,'布放声纳浮标 #$1，坐标（$2，$3），关联接触 $4、质量 $5；在模拟第 $6 秒失效。'],
      [/^sonobuoy #(\d+) expired after (\d+) s at \(([-\d.]+), ([-\d.]+)\)$/,'声纳浮标 #$1 在工作 $2 秒后失效；位置（$3，$4）。'],
      [/^(.+) runway below ([\d.]+)% operable \(hull ([\d.]+)%\); deck cycle ([\d.]+) s = nominal x([\d.]+)$/,'$1 的跑道可用性低于 $2%（耐久剩余 $3%）；航空作业周期变为 $4 秒，即正常值的 $5 倍。'],
      [/^(.+) detected the wake of (.+) \(([\d.]+) nm off the wake centerline\)$/,'$1 发现 $2 的尾迹；距尾流中心线 $3 海里。'],
      [/^BOL launch: track q=([\d.]+) < 0.30, (\d+) round\(s\), Pk ×0.25, immediate unguided adjudication \(adapted §8.4.2\)$/,'接触质量 $1 低于0.30，发射 $2 发；本RTS应急分支将命中概率乘0.25并立即结算（8.4.2改编）。'],
      [/^SAM\/AAM engagement by #(\d+) within 2.0 nm of friendly aircraft #(\d+); 5% misidentification, half damage$/,'单位 #$1 的防空交战发生在友机 #$2 周围2.0海里内；当前改编分支的5%误判已触发，友机承受半额损伤。'],
      [/^(.+) column row `(.+)` \(roll (\d+)\) ignored: system not fitted$/,'重创表列 $1、条目「$2」、掷骰 $3；该系统未安装，因此忽略此条。'],
      [/^(.+) hit by (.+): (.+) light critical \(10% bow\/stern location effect\)$/,'命中部位 $1，武器 $2；触发10%舰艏/舰艉附加效果，造成 $3 轻微重创（RTS估算改编）。'],
      [/^pass (\d+): (.+) \(maneuver ([\d.]+), ([\d.]+) kt, ([\d.]+) m\) vs (.+) \(maneuver ([\d.]+), ([\d.]+) kt, ([\d.]+) m\); gun pk ([\d.]+); hit=(true|false); target hp ([\d.]+)\/([\d.]+)$/,(_,pass,a,am,as,ah,b,bm,bs,bh,pk,hit,hp,max)=>`第 ${pass} 次交错：${a}（机动值 ${am}，${as} 节，${ah} 米）攻击 ${b}（机动值 ${bm}，${bs} 节，${bh} 米）。机炮命中概率 ${(Number(pk)*100).toFixed(1)}%；${hit==='true'?'本次命中':'本次未命中'}；目标耐久 ${hp} / ${max}。`],
    ];
    for(const [pattern,replacement] of patterns) if(pattern.test(s)) return s.replace(pattern,replacement);
    return null;
  }
  function ruleReport(event,lang='zh-CN') {
    const known=!!rules[event.rule];
    return {title:pick(rules[event.rule]||pair('规则处理记录','Rules processing record'),lang),
      meaning:pick(meanings[event.rule]||pair('当前引擎记录了一次规则处理；这条记录暂未提供专门的中文含义。','The engine recorded a rules operation; a dedicated explanation is not yet available.'),lang),
      detail:lang==='en'?String(event.detail||''):traceZh(event.detail),known};
  }
  const notes={
    detection:pair('这是一次己方发现记录；位置是估计值，不代表已经识别型号或获得可靠射击条件。','A friendly detection, with an estimated position; this does not prove class identification or a reliable firing solution.'),
    contact_identified:pair('接触已有型号判断，但位置与质量仍需要持续更新。','The contact has a class assessment; its position and quality still need updates.'),
    contact_lost:pair('最近缺少有效更新，旧接触可能已经偏离原位置。','No recent useful update; the old contact may have moved away from its estimate.'),
    weapons_release:pair('武器已经实际发射，仍需等待在途、拦截及最终命中结果。','A weapon actually launched; flight, interception and impact results are still pending.'),
    gun_action:pair('这是一次已经执行的炮击，命中数在本条列出。若处于自动交战，冷却完成后还可能继续射击。','An executed gun salvo; this entry gives its hit count. Automatic engagement may fire again after cooldown.'),
    incoming:pair('这是来袭警报；预计抵达时间会随位置变化，不代表必定命中。','An incoming warning; estimated time changes with geometry and does not guarantee a hit.'),
    impact:pair('这是实际命中造成的损伤；剩余耐久越低，单位越接近损失。','Actual impact damage; lower remaining durability means a unit is closer to destruction.'),
    chaff_deployed:pair('箔条已释放，但需要后续诱偏或交战记录才能判断效果。','Chaff deployed; later diversion or combat reports determine its effect.'),
  };
  function eventNote(event,lang) {
    if(event.event==='intercept') return pick(event.intercepted
      ? pair('本次拦截成功；其他来袭弹仍可能继续，近处爆炸还可能产生破片波及。','This interception succeeded; other threats may remain and close explosions may still produce fragments.')
      : pair('本次拦截未成功；来袭弹可能继续飞行，后续仍需留意拦截与命中记录。','This intercept failed; the incoming weapon may continue. Watch for later interception or impact reports.'),lang);
    return pick(notes[event.event],lang);
  }
  if(typeof module!=='undefined'&&module.exports) module.exports={firingState,ruleReport,traceZh,eventNote};
  if(!root.document) return;
  root.HarpoonCombat={firingState,ruleReport,traceZh,eventNote};
  const I=root.HarpoonI18n, b=(zh,en)=>pick(pair(zh,en),I.language);
  let overlay, returnFocus;
  const closeHelp=()=>{overlay.hidden=true;returnFocus?.focus();};
  function renderHelp(){
    if(!overlay)return;
    const languageSelect=overlay.querySelector('[data-help-language]');
    const languageFocused=languageSelect===document.activeElement;
    overlay.innerHTML=`<section class="operator-dialog" role="dialog" aria-modal="true" aria-labelledby="operator-title"><header><h1 id="operator-title">${b('先弄清：你在控制什么','Understand what you control')}</h1><select data-help-language aria-label="语言 / Language"><option value="zh-CN">简体中文</option><option value="en">English</option></select><button data-help-close aria-label="${b('关闭玩法说明','Close help')}">×</button></header><p class="operator-pause">${b('已暂停，方便阅读。关闭说明后仍保持暂停，由你选择继续倍速。','Paused for reading. Closing help keeps time paused until you resume.')}</p>
      <h2>${b('开火：四种操作不是同一件事','Firing: four different actions')}</h2><table><thead><tr><th>${b('操作','Action')}</th><th>${b('实际效果','Actual effect')}</th></tr></thead><tbody>
      <tr><td>${b('点选接触','Select contact')}</td><td>${b('只查看详情，不下达攻击。','Inspect information; no attack is ordered.')}</td></tr>
      <tr><td>${b('人工射击1发','Fire 1 round manually')}</td><td>${b('一次性命令。当前按钮每次提交1发；下一发需要等冷却再下令。暂停时仅排队，运行后执行；回执不等于成功发射。','One order for one round. Wait for cooldown before ordering another. While paused it only queues; a receipt does not prove a successful shot.')}</td></tr>
      <tr><td>${b('指定目标自动交战','Assign target for automatic engagement')}</td><td>${b('持续命令。会自行选用可用武器，冷却完成且条件允许时继续打。当前实现中，即使关闭自动选敌，指派目标仍会被攻击。','A standing order. Available weapons engage again when conditions and cooldown permit. An assigned target is engaged even when free-target selection is off.')}</td></tr>
      <tr><td>${b('自动选敌开火','Automatic target selection and fire')}</td><td>${b('原“自由开火”：没有有效指派时，引擎自动寻找接触交战；不是完整作战参谋。','Previously Weapons free: when no assigned target resolves, the engine selects a contact. This is not a full command staff.')}</td></tr></tbody></table>
      <p>${b('要停止主动自动交战：先取消指派，再关闭自动选敌。已发射武器不会因此撤回；点防御等自卫机制仍可能自主反应。别靠连点射击维持火力。','To stop active automatic engagement, clear the assigned target and turn off automatic selection. Existing projectiles are not recalled; point defence may still react independently. Repeated clicking is not a continuous-fire mode.')}</p>
      <h2>${b('探测：先发现，再判断能不能打','Detection: discovery precedes a firing decision')}</h2><ol><li>${b('时间运行时传感器自动扫描；当前RTS基础扫描周期为4个模拟秒。暂停时不会刷新探测。','Sensors scan as time advances; the current RTS base interval is four simulated seconds. Pausing stops detection updates.')}</li><li>${b('雷达/目视受目标高度、视距、环境与目标特征影响。数据库最大距离不等于任何目标都能在那个距离被发现。','Radar and lookouts depend on height, horizon, conditions and target characteristics. Database maximum range is not guaranteed detection range.')}</li><li>${b('电子支援听雷达等辐射，通常先有方位；被动声纳听噪声。主动声纳会发声探测，也可能暴露自己，需在机动面板单独开启。','ESM listens for emissions and often starts with a bearing; passive sonar listens for noise. Active sonar transmits and may reveal you; enable it separately in MANOEUVRE.')}</li><li>${b('C编号是己方接触，位置有误差。质量表示情报可靠程度，不是命中概率；“已识别型号”也不代表位置无误差。陈旧接触要重新获取。','C labels are friendly contact records with uncertain positions. Quality is intelligence reliability, not hit probability. An identified class does not imply exact position; stale tracks need reacquisition.')}</li></ol>
      <h2>${b('太平洋风暴为什么双方会自己动','Why both sides act in Pacific Storm')}</h2><p>${b('该想定启动时就有航速、航线、护航跟随关系和自由开火姿态。预设命令会持续执行，武器会自动选敌；任务执行、自卫与缠斗也有底层自动机制。它是机制验证大想定，直接开局容易信息过载。','The scenario starts with speed, routes, escort stations and weapons-free posture. Standing orders continue and weapons select contacts automatically. Mission execution, defence and dogfights also have automatic subsystems. This large integration scenario is demanding for a first game.')}</p><p>${b('仓库有启发式脚本参谋与可配置大模型接口，但普通实时对局没有持续调度它们。脚本参谋用于自对弈/单步AI接口；这轮教学仅有受控的防御课对手。看到双方自主移动，不代表大模型在连续指挥。','The repository includes heuristic staff and a configurable LLM interface, but normal real-time games do not continuously schedule them. Staff are used by self-play and the one-step AI endpoint; training adds a controlled defence opponent. Autonomous movement is not evidence of continuous LLM command.')}</p>
      <h2>${b('战报怎么看','Reading the reports')}</h2><p>${b('先看：发现了什么 → 是否已发射 → 是否被拦截 → 是否命中 → 造成多少损伤。规则事件会给出中文含义和关键数值。D20是20面骰；概率是判定条件，不是保证结果；能力比例1.00表示完整系统能力。数据链共享不是攻击记录。','Read: detection → launch → interception → impact → damage. Rule records provide an explanation and key values. D20 means a 20-sided die. Probability is not a guaranteed outcome; system ratio 1.00 means full capability. Data-sharing records are not attacks.')}</p><p class="operator-source">${b('说明基于当前RTS代码与已声明改编；探测概念来源：Harpoon V 2026英文版5.1–5.4（5-1页等）。详细桌面规则尚未全部实现。','This describes the current RTS code and declared adaptations. Detection concepts: Harpoon V 2026 English rules §§5.1–5.4 (including p. 5-1). Full tabletop procedures are not all implemented.')}</p><footer><button data-help-close>${b('明白了，返回暂停的战局','Return to the paused game')}</button></footer></section>`;
    if(languageSelect)overlay.querySelector('[data-help-language]').replaceWith(languageSelect);
    overlay.querySelector('[data-help-language]').value=I.language;
    if(languageFocused)overlay.querySelector('[data-help-language]').focus();
  }
  async function openHelp(){
    if(!root.HarpoonUI)return;
    await root.HarpoonUI.setSpeed(0);
    if(root.HarpoonUI.snapshot().speed!==0)return;
    returnFocus=document.activeElement;
    if(!overlay){overlay=document.createElement('div');overlay.className='operator-overlay';overlay.dataset.i18nNative='';document.body.append(overlay);overlay.addEventListener('click',e=>{if(e.target.closest('[data-help-close]'))closeHelp();});overlay.addEventListener('change',e=>{if(e.target.matches('[data-help-language]'))I.setLanguage(e.target.value);});overlay.addEventListener('keydown',e=>{if(e.key==='Escape'){e.preventDefault();closeHelp();}if(e.key==='Tab'){const items=[...overlay.querySelectorAll('button,select')],first=items[0],last=items.at(-1);if(e.shiftKey&&document.activeElement===first){e.preventDefault();last.focus();}else if(!e.shiftKey&&document.activeElement===last){e.preventDefault();first.focus();}}});}
    overlay.hidden=false;renderHelp();overlay.querySelector('[data-help-close]').focus();
  }
  document.getElementById('btn-operator-help').addEventListener('click',()=>void openHelp());
  root.addEventListener('harpoon:language',renderHelp);
})(typeof window==='undefined'?globalThis:window);
