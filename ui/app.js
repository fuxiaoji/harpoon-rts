const app = {
  side: "nato", state: null, actions: [], selectedUnit: null, selectedAction: null,
  phaseNames: { plotting:"计划", movement:"移动", planned_fire:"计划射击", detection:"探测", reaction_fire:"反应射击", resolution:"结算" },
  actionNames: { move:"机动", fire:"武器发射", wait:"保持当前命令", air_mission:"航空任务", air_attack:"空中攻击", submarine_attack:"潜艇攻击", land_strike:"对陆打击", electronic_warfare:"电子战", mine_warfare:"水雷战" },
  tutorial: { active:false, step:0 }, aiOpponent:false,
  scenarios:[], controlMode:"human", staffPlan:null, staffApprovals:[], staffBriefing:null,
  mapView: { zoom:1, x:0, y:0, dragging:false, pointerId:null, startX:0, startY:0, originX:0, originY:0 },
  lastFeedbackEventId:null, feedbackTimer:null
};

const tutorialSteps = [
  { kicker:"完整战役教学", title:"从航行计划打到胜负判定", copy:"你固定指挥 NATO，苏方由确定性 AI 操控。教程将覆盖移动、探测、火控解、交战规则、齐射、导弹飞行、命中裁决、DP 判伤与胜利条件。", button:"开始完整战役 · 约 12 分钟" },
  { kicker:"航行 1/5 · 认识编队", title:"选择补给舰 Poolster", copy:"Poolster 是苏方主要目标。移动不是为了“走一步”，而是改变武器射程、编队间距和后续传感器几何关系。先查看它。", focus:'[data-unit="poolster-a835"]' },
  { kicker:"航行 2/5 · 查看选择", title:"打开“行动”页", copy:"这里是人类与 AI 共用的合法行动接口。每项动作都带规则依据，红色动作会说明为什么不能做。", focus:'[data-tab="actions"]' },
  { kicker:"航行 3/5 · 规划机动", title:"选择 Poolster 的“机动”", copy:"双方先秘密规划、锁定后同时结算，所以提交顺序不会带来优势。", focus:'[data-action="move:poolster-a835"]' },
  { kicker:"航行 4/5 · 距离公式", title:"设定 20°、15 节", copy:"三分钟移动距离 = 15×3÷60 = 0.75 海里。提交后事件明细会列出公式、输入和新坐标。", focus:"#action-dialog" },
  { kicker:"航行 5/5 · 同时命令", title:"锁定 NATO 计划", copy:"锁定意味着不再改令。苏方 AI 随后通过相同 observation/action 接口规划，但无法读取你的隐藏命令。", focus:"#lock-button" },
  { kicker:"敌方计划", title:"苏方已秘密锁定", copy:"接下来逐阶段执行。计划→移动→计划射击→探测→反应射击→结算，每一步下方战报都会显示查表与计算细节。", button:"执行计划阶段" },
  { kicker:"第一战术回合", title:"执行并理解六个阶段", copy:"逐步推进；教学文字会随阶段解释当前发生什么。", button:"推进并解释当前阶段" },
  { kicker:"探测与火控", title:"选择 USS Copeland", copy:"想定初始接触已明确给出 Good 火控解，因此无需编造未核验的探测骰。Good 解允许导弹射击；苏方刚才开火后，NATO 的和平时期禁火限制也解除。", focus:'[data-unit="uss-copeland-ffg25"]' },
  { kicker:"反击 1/4 · 合法行动", title:"打开 Copeland 的“行动”页", copy:"现在“武器发射”从 BLOCKED 变为 AVAILABLE。比较前后状态，就能看到 ROE 如何影响合法动作。", focus:'[data-tab="actions"]' },
  { kicker:"反击 2/4 · 选择武器", title:"选择 RGM-84D 武器发射", copy:"引擎会检查 Good 火控解、弹药、最小/最大射程；到计划射击阶段才真正扣除弹药并生成在途武器。", focus:'[data-action="fire:uss-copeland-ffg25:rgm-84d"]' },
  { kicker:"反击 3/4 · 选择目标", title:"确认目标 Smyshlennyy", copy:"本教学用一次四枚 Harpoon 齐射缩短流程。每枚仍作为独立命令、独立在途实体和独立损伤骰处理。", focus:"#action-dialog" },
  { kicker:"反击 4/4 · 锁定齐射", title:"锁定 NATO 齐射计划", copy:"四枚命令已进入计划。苏方 AI 本回合保持航向，双方再次同时锁定。", focus:"#lock-button" },
  { kicker:"来袭导弹", title:"推进到 P-15M 抵达", copy:"先执行计划，再执行移动。在途导弹每个移动阶段减少一个到达回合。", button:"推进并解释当前阶段" },
  { kicker:"软杀伤查表", title:"自动处理 P-15M", copy:"按 Poolster 的 Medium 特征、二代 ECM、诱饵和 P-15M 二代导引头定位 8-15 表的 56% 单元，再掷 D100。只有导引成功才继续判伤。", button:"查 8-15 表、掷 D100 并查看结果" },
  { kicker:"完成第二回合", title:"继续到下一计划阶段", copy:"计划射击阶段会发射四枚 Harpoon；探测阶段报告 Good 火控解的来源；结算阶段更新战备和胜负。", button:"推进并解释当前阶段" },
  { kicker:"导弹飞行", title:"锁定等待命令，进入第三回合", copy:"这一回合无需新机动。双方保持命令，重点观察四枚 Harpoon 如何在移动阶段抵达。", button:"双方保持并锁定" },
  { kicker:"导弹飞行", title:"推进到 Harpoon 齐射抵达", copy:"查看海图上的在途实体和发射事件中的射程、速度、ETA 公式。", button:"推进并解释当前阶段" },
  { kicker:"完整损伤链", title:"逐枚裁决四枚 Harpoon", copy:"每枚先按 Small 特征、二代 J&D、三代导引头查得 80%，独立掷 D100；命中后再结算特殊伤害修正、伤害比率、D6 致命数量、D20 类型和自动火灾。", button:"逐枚查表并判伤" },
  { kicker:"胜负判定", title:"推进到结算阶段", copy:"真实骰运可能造成沉没、重创或未决结果。结算阶段严格依据当前舰艇状态和想定胜利条件报告结果，不保证教学方必胜。", button:"推进并解释当前阶段" },
  { kicker:"完整战役完成", title:"本次交战已完成", copy:"你已经走完移动、探测/火控、ROE、武器发射、导弹飞行、软杀伤查表、DP、致命损伤与胜负检查。展开事件可复盘每个行列、骰值和中间结果。", button:"结束教学并查看战报" }
];

const $ = (selector) => document.querySelector(selector);
const $$ = (selector) => [...document.querySelectorAll(selector)];
const escapeHtml = (value) => String(value ?? "").replace(/[&<>'"]/g, c => ({"&":"&amp;","<":"&lt;",">":"&gt;","'":"&#39;",'"':"&quot;"}[c]));

async function api(path, options = {}) {
  const response = await fetch(path, { headers:{"Content-Type":"application/json"}, ...options });
  const data = await response.json();
  if (!response.ok || data.error) throw new Error(data.error || `HTTP ${response.status}`);
  return data;
}

async function refresh(preserveSelection = true) {
  try {
    app.state = await api(`/api/state?side=${app.side}`);
    app.actions = app.side === "referee" ? [] : await api(`/api/actions?side=${app.side}`);
    if (!preserveSelection || !app.state.units.some(u => u.unit_id === app.selectedUnit)) app.selectedUnit = app.state.units[0]?.unit_id || null;
    render();
  } catch (error) { toast(error.message, true); }
}

function render() {
  const state = app.state;
  $("#scenario-name").textContent = (app.scenarios.find(item=>item.scenario_id===state.scenario_id)?.title || state.scenario_id).toUpperCase();
  $("#game-time").textContent = state.timestamp.slice(0,16).replace("T"," ");
  $("#turn-label").textContent = `TURN ${String(state.turn).padStart(2,"0")}`;
  $("#hash-label").textContent = `HASH ${state.state_hash.slice(0,10).toUpperCase()}`;
  $("#phase-chip").textContent = app.phaseNames[state.phase];
  $$("#phase-list li").forEach(li => li.classList.toggle("active", li.dataset.phase === state.phase));
  $$(".side-button").forEach(button => button.classList.toggle("active", button.dataset.side === app.side));
  $("#lock-status").textContent = state.locked_sides.length ? `已锁定：${state.locked_sides.join(" / ").toUpperCase()}` : "等待双方提交计划";
  const isPlayer = app.side !== "referee";
  $("#lock-button").disabled = !isPlayer || state.phase !== "plotting" || state.locked_sides.includes(app.side);
  $("#resolve-button").disabled = app.side !== "referee" && !(state.phase !== "plotting" || state.locked_sides.length === 2);
  $("#objectives").innerHTML = state.objectives.length ? state.objectives.map(item => `<li>${escapeHtml(item)}</li>`).join("") : `<li>${app.side === "referee" ? "监督双方态势与未决武器" : "未载入任务目标"}</li>`;
  renderMap(); renderUnits(); renderActions(); renderEvents(); renderTutorial(); renderStaff();
}

function renderSideButtons(){
  if(!app.state)return;
  const rail=document.querySelector(".side-switch");
  const labels={china:"中国",usa:"美国",nato:"NATO",soviet:"苏方"};
  rail.innerHTML=[...app.state.available_sides,"referee"].map(side=>`<button class="side-button ${side==="referee"?"referee":""} ${side===app.side?"active":""}" data-side="${escapeHtml(side)}">${escapeHtml(labels[side]||side.toUpperCase())}</button>`).join("");
  rail.querySelectorAll(".side-button").forEach(button=>button.onclick=()=>{if(app.tutorial.active||app.aiOpponent)return;app.side=button.dataset.side;app.selectedUnit=null;app.staffPlan=null;app.staffApprovals=[];app.staffBriefing=null;refresh(false).then(renderSideButtons);});
}

function renderStaff(){
  const enabled=app.controlMode==="staff"&&app.side!=="referee";
  $("#staff-intent-form").querySelectorAll("textarea,button").forEach(node=>node.disabled=!enabled);
  const plan=app.staffPlan;
  $("#staff-plan").classList.toggle("empty-state",!plan);
  $("#staff-plan").innerHTML=plan?`<h3>${escapeHtml(plan.summary)}</h3><p>状态：${escapeHtml(plan.status)} · 有效至 T${plan.valid_until_turn}</p><ol>${plan.steps.map(step=>`<li>${escapeHtml(step.objective)}${step.requires_confirmation?" · 需确认":""}</li>`).join("")}</ol>${plan.status==="draft"?`<button id="staff-approve-plan" class="primary-button">批准计划</button>`:`<button id="staff-execute-plan" class="primary-button">执行下一决策点</button>`}`:"尚未生成计划";
  const approvals=app.staffApprovals||[];
  $("#staff-approvals").innerHTML=approvals.length?`<h3>待指挥官确认</h3>${approvals.map(item=>`<div><p>${escapeHtml(item.reason)}<br>${escapeHtml(item.proposal.action_id)}</p><button class="primary-button staff-approve-action" data-approval="${escapeHtml(item.approval_id)}">批准该行动</button></div>`).join("")}`:"";
  const brief=app.staffBriefing;
  $("#staff-briefing").classList.toggle("empty-state",!brief);
  $("#staff-briefing").innerHTML=brief?`<h3>指挥所简报</h3><p>${escapeHtml(brief.mission_progress)}</p><strong>已确认情报</strong><ul>${brief.confirmed_intelligence.map(item=>`<li>${escapeHtml(item.statement)} · ${Math.round(item.confidence*100)}%</li>`).join("")||"<li>无</li>"}</ul><strong>建议</strong><ul>${brief.recommendations.map(item=>`<li>${escapeHtml(item)}</li>`).join("")}</ul>`:"批准计划后生成指挥所简报";
  const approvePlan=$("#staff-approve-plan"); if(approvePlan)approvePlan.onclick=approveStaffPlan;
  const execute=$("#staff-execute-plan"); if(execute)execute.onclick=executeStaffPlan;
  $$(".staff-approve-action").forEach(button=>button.onclick=()=>approveStaffAction(button.dataset.approval));
}

async function submitStaffIntent(event){
  event.preventDefault();
  try{
    app.staffPlan=await api("/api/staff/intent",{method:"POST",body:JSON.stringify({intent_id:`intent-${Date.now()}`,side_id:app.side,objective:$("#staff-intent").value})});
    app.staffApprovals=[];app.staffBriefing=null;renderStaff();toast("参谋计划已生成，等待批准");
  }catch(error){toast(error.message,true);}
}
async function approveStaffPlan(){app.staffPlan=await api("/api/staff/plan/approve",{method:"POST",body:JSON.stringify({side_id:app.side,plan_id:app.staffPlan.plan_id})});app.staffBriefing=await api(`/api/staff/briefing?side=${app.side}`);renderStaff();toast("计划已批准");}
async function executeStaffPlan(){const result=await api("/api/staff/execute",{method:"POST",body:JSON.stringify({side_id:app.side})});app.staffApprovals=result.approvals;app.staffBriefing=await api(`/api/staff/briefing?side=${app.side}`);await refresh();toast(result.approvals.length?"重大行动等待确认":"参谋已提交合法动作");}
async function approveStaffAction(approvalId){await api("/api/staff/action/approve",{method:"POST",body:JSON.stringify({side_id:app.side,approval_id:approvalId})});app.staffApprovals=app.staffApprovals.filter(item=>item.approval_id!==approvalId);app.staffBriefing=await api(`/api/staff/briefing?side=${app.side}`);await refresh();toast("重大行动已批准并提交");}

async function initializeScenarios(){
  app.scenarios=await api("/api/scenarios");
  $("#scenario-select").innerHTML=app.scenarios.map(item=>`<option value="${escapeHtml(item.scenario_id)}">${escapeHtml(item.title)} · ${escapeHtml(item.status)}</option>`).join("");
  await refresh(false);$("#scenario-select").value=app.state.scenario_id;renderSideButtons();
}

function coordinates(position) {
  const scale = 28;
  return { left:`calc(50% + ${position.x_nm * scale}px)`, top:`calc(50% - ${position.y_nm * scale}px)` };
}

function applyMapTransform() {
  const view=app.mapView;
  $("#map-world").style.transform=`translate(${view.x}px,${view.y}px) scale(${view.zoom})`;
  $("#map-zoom-label").textContent=`${Math.round(view.zoom*100)}%`;
  $("#scale-label").textContent=`${(5/view.zoom).toFixed(view.zoom>=1?1:0)} NM`;
}
function setMapZoom(next,anchorX=null,anchorY=null) {
  const map=$("#map"); const rect=map.getBoundingClientRect(); const view=app.mapView;
  const zoom=Math.min(3.5,Math.max(.45,next)); if(zoom===view.zoom)return;
  const ax=anchorX??rect.left+rect.width/2; const ay=anchorY??rect.top+rect.height/2;
  const localX=ax-(rect.left+rect.width/2)-view.x; const localY=ay-(rect.top+rect.height/2)-view.y;
  const ratio=zoom/view.zoom; view.x-=localX*(ratio-1); view.y-=localY*(ratio-1); view.zoom=zoom; applyMapTransform();
}
function resetMapView(){Object.assign(app.mapView,{zoom:1,x:0,y:0});applyMapTransform();}

function renderMap() {
  const container = $("#map-entities"); container.innerHTML = "";
  app.state.units.forEach(unit => {
    const button = document.createElement("button"); const pos = coordinates(unit.position);
    button.className = `map-entity friendly ${unit.unit_id === app.selectedUnit ? "selected" : ""}`;
    button.style.left = pos.left; button.style.top = pos.top; button.dataset.unit = unit.unit_id;
    button.innerHTML = `<span class="entity-symbol"></span><span class="entity-label">${escapeHtml(unit.unit_id)}</span>`;
    button.onclick = () => selectUnit(unit.unit_id); container.appendChild(button);
  });
  app.state.contacts.forEach(contact => {
    if (!contact.last_known_position) return;
    const button = document.createElement("button"); const pos = coordinates(contact.last_known_position);
    button.className = "map-entity contact"; button.style.left = pos.left; button.style.top = pos.top;
    button.innerHTML = `<span class="entity-symbol"></span><span class="entity-label">${escapeHtml(contact.target_unit_id || contact.contact_id)} · ${escapeHtml(contact.quality)}</span>`;
    container.appendChild(button);
  });
  app.state.in_flight_weapons.forEach((weapon, index) => {
    const shooter = app.state.units.find(u => u.unit_id === weapon.shooter_id);
    const contact = app.state.contacts.find(c => c.target_unit_id === weapon.target_id);
    const target = app.state.units.find(u => u.unit_id === weapon.target_id);
    const start = shooter?.position || {x_nm:0,y_nm:0}; const end = target?.position || contact?.last_known_position || start;
    const ratio = weapon.awaiting_adjudication ? 1 : .55; const position = {x_nm:start.x_nm+(end.x_nm-start.x_nm)*ratio,y_nm:start.y_nm+(end.y_nm-start.y_nm)*ratio}; const pos=coordinates(position);
    const button=document.createElement("button"); button.className="map-entity weapon"; button.style.left=pos.left; button.style.top=pos.top;
    button.innerHTML=`<span class="entity-symbol"></span><span class="entity-label">${escapeHtml(weapon.weapon_id)} · T-${weapon.turns_to_arrival}</span>`;
    if(app.side === "referee" && weapon.awaiting_adjudication) button.onclick=()=>openAdjudication(weapon);
    container.appendChild(button);
  });
  applyMapTransform();
}

function renderUnits() {
  const list=$("#unit-list"); list.innerHTML="";
  app.state.units.forEach(unit => {
    const max = platformMaxDp(unit.platform_id); const percent=max ? Math.round(unit.damage_dp/max*100):0;
    const button=document.createElement("button"); button.className=`unit-card ${unit.unit_id===app.selectedUnit?"active":""}`; button.dataset.unit=unit.unit_id;
    button.innerHTML=`<strong>${escapeHtml(unit.unit_id)}</strong><span>${unit.speed_knots.toFixed(0)} KT / ${unit.course_deg.toFixed(0)}°</span><div class="damage-bar"><i style="width:${percent}%"></i></div>`;
    button.onclick=()=>selectUnit(unit.unit_id); list.appendChild(button);
  });
  const unit=app.state.units.find(u=>u.unit_id===app.selectedUnit);
  $("#unit-detail").innerHTML=unit ? unitDetail(unit) : "在海图或列表中选择一个单位";
}

function platformMaxDp(platformId) { const values={"kortenaer-1982":177,"bremen-1986":175,"poolster-1983":249,"leander-b3-1989":157,"oh-perry-1989":154,"mod-kashin-1975":161}; return values[platformId] || 0; }
function unitDetail(unit) { const max=platformMaxDp(unit.platform_id); return `<p class="section-label">${escapeHtml(unit.side_id)} · ${escapeHtml(unit.platform_id)}</p><h3>${escapeHtml(unit.unit_id)}</h3><div class="detail-grid"><div><span>位置 NM</span><strong>${unit.position.x_nm.toFixed(2)}, ${unit.position.y_nm.toFixed(2)}</strong></div><div><span>航行状态</span><strong>${unit.course_deg.toFixed(0)}° / ${unit.speed_knots.toFixed(0)} KT</strong></div><div><span>损伤</span><strong>${unit.damage_dp} / ${max} DP</strong></div><div><span>战备</span><strong>${escapeHtml(unit.readiness)}</strong></div><div><span>弹药</span><strong>${Object.entries(unit.ammunition).map(([k,v])=>`${k} ×${v}`).join(", ")||"—"}</strong></div><div><span>状态</span><strong>${unit.sunk?"沉没":unit.crippled?"重创":"可用"}</strong></div></div>`; }

function renderActions() {
  const selected = app.actions.filter(a => !a.unit_id || a.unit_id === app.selectedUnit);
  $("#action-summary").textContent = app.side === "referee" ? "裁判可在海图点击抵达目标的武器进行裁决" : `${app.selectedUnit || "未选择单位"} · ${selected.filter(a=>a.available).length} 项可用行动`;
  $("#action-list").innerHTML = selected.map(action => `<button class="action-card ${action.available?"":"unavailable"}" data-action="${escapeHtml(action.action_id)}" ${action.available?"":"disabled"}><header><strong>${escapeHtml(app.actionNames[action.action_type]||action.action_type)}</strong><code>${action.available?"AVAILABLE":"BLOCKED"}</code></header><p>${escapeHtml(action.available ? action.rule.explanation : action.unavailable_reason)}</p></button>`).join("") || `<div class="empty-state">当前席位没有可提交的行动</div>`;
  $$(".action-card").forEach(button => button.onclick=()=>openAction(button.dataset.action));
}

function renderEvents() {
  const events=[...app.state.events].reverse(); $("#event-count").textContent=`${events.length} EVENTS`;
  $("#event-log").innerHTML=events.length?events.map((event,index)=>renderEventCard(event,index===0)).join(""):`<div class="empty-state">尚无事件。提交命令并推进阶段开始推演。</div>`;
  const battle=events.find(event=>event.event_type==="weapon_adjudicated"); if(battle)showCombatFeedback(battle);
}

function diceText(record){return record?.rolls?.join(" + ")||"—";}
function eventOutcome(event){
  const p=event.payload||{};
  if(event.event_type==="weapon_adjudicated")return p.hit?`${p.damage_dp||0} DP · ${p.critical_hit_resolution?.results?.length||0} 项致命损伤`:"软杀伤成功 · 导弹脱靶";
  if(event.event_type==="weapon_launched")return `${p.weapon_id} · ETA ${p.eta_turns} 回合`;
  if(event.event_type==="unit_moved")return `${Number(p.distance_nm||0).toFixed(2)} NM`;
  if(event.event_type==="victory")return String(p.winner||"胜负已判定");
  return event.actors.join(" → ")||"系统事件";
}
function resolutionSteps(event){
  const p=event.payload||{}; if(event.event_type!=="weapon_adjudicated")return [];
  const hit=p.hit_resolution||{}; const lookup=hit.lookup||{}; const coords=lookup.coordinates||{}; const roll=hit.roll||{};
  const steps=[
    ["目标与对抗",`${coords.target_signature||"—"} 特征 · ECM ${coords.ecm_generation||"—"} 代 · ${coords.countermeasure||"无对抗"} · 导引头 ${coords.seeker_generation||"—"} 代`],
    ["8-15 查表",`表格结果 ${lookup.result!=null?Math.round(lookup.result*100)+"%":"裁判裁决"}`],
    ["命中骰",`${roll.kind?.toUpperCase()||"D100"} = ${diceText(roll)} · ${hit.comparison||"人工裁决"} · ${p.hit?"导引成功":"诱偏脱靶"}`]
  ];
  if(p.hit&&p.calculation){const c=p.calculation;steps.push(["战斗部",`${c.base_damage_dp} + floor(${c.die_roll}÷${c.damage_divisor}) = ${c.raw_damage_dp} DP`]);steps.push(["舰体修正",`${c.raw_damage_dp} × ${100+c.target_damage_modifier_percent}% = ${c.result_damage_dp} DP · 累计 ${c.target_new_damage_dp}/${c.sunk_threshold_dp}`]);}
  if(p.hit&&p.critical_hit_resolution){const c=p.critical_hit_resolution;steps.push(["致命损伤",`伤害比率 ${c.damage_ratio_lookup?.coordinates?.damage_ratio_row||"—"} · 附加 ${c.missile_airframe_lookup?.result||0} · 结果：${(c.results||[]).join("、")||"无"}`]);}
  return steps;
}
function renderEventCard(event,isLatest){
  const p=event.payload||{}; const tone=event.event_type==="weapon_adjudicated"?(p.hit?"hit":"miss"):(event.random_record?"random":""); const steps=resolutionSteps(event);
  return `<article class="event-card ${tone}"><span class="event-meta">T${event.turn} · ${escapeHtml(app.phaseNames[event.phase]||event.phase)} · ${escapeHtml(event.event_id)}</span><h3>${escapeHtml(eventName(event.event_type))}</h3><p>${escapeHtml(event.actors.join(" → ")||"系统事件")}</p><div class="event-outcome"><strong>${escapeHtml(eventOutcome(event))}</strong>${event.random_record?`<code>${escapeHtml(event.random_record.kind.toUpperCase())} ${escapeHtml(diceText(event.random_record))}</code>`:""}</div>${steps.length?`<div class="resolution-steps">${steps.map(([title,text],i)=>`<div class="resolution-step"><i>${i+1}</i><div><strong>${escapeHtml(title)}</strong>${escapeHtml(text)}</div></div>`).join("")}</div>`:""}<details ${isLatest&&event.event_type==="weapon_adjudicated"?"open":""}><summary>${steps.length?"展开全部原始字段":"查看结算明细"}</summary><div class="trace-grid">${flattenTrace(p).map(([key,value])=>`<div><b>${escapeHtml(key)}</b><span>${escapeHtml(value)}</span></div>`).join("")||"<div><span>此事件没有额外数值输入</span></div>"}</div><div class="event-rules">规则：${escapeHtml(event.rule_ids.join(" · ")||"想定胜负条件")}</div></details></article>`;
}
function showCombatFeedback(event){
  if(event.event_id===app.lastFeedbackEventId)return; app.lastFeedbackEventId=event.event_id;
  const p=event.payload||{}; const node=$("#combat-feedback"); node.className=`combat-feedback ${p.hit?"hit":"miss"}`;
  node.innerHTML=`<small>WEAPON RESOLUTION · ${escapeHtml(event.actors.join(" → "))}</small><strong>${p.hit?"命中！":"诱饵奏效 · 导弹脱靶"}</strong><p>${escapeHtml(eventOutcome(event))}${p.hit?`；累计损伤 ${p.calculation?.target_new_damage_dp||0}/${p.calculation?.sunk_threshold_dp||0} DP`:"；目标未受损"}</p>`;
  requestAnimationFrame(()=>node.classList.add("show")); clearTimeout(app.feedbackTimer); app.feedbackTimer=setTimeout(()=>node.classList.remove("show"),3200);
}

function eventName(type){return ({orders_locked:"双方命令锁定",unit_moved:"单位移动结算",weapon_launched:"武器发射",weapon_arrival_requires_adjudication:"武器抵达待裁决",weapon_adjudicated:"命中与损伤裁决",contact_solution_report:"接触与火控解报告",readiness_transition_started:"战备转换查表",attack_word_check:"攻击命令检定",victory:"胜负判定",launch_rejected_at_resolution:"发射被拒绝"})[type]||type;}
function flattenTrace(value,prefix="") { const rows=[]; Object.entries(value||{}).forEach(([key,item])=>{const label=prefix?`${prefix}.${key}`:key;if(item&&typeof item==="object"&&!Array.isArray(item))rows.push(...flattenTrace(item,label));else rows.push([label,Array.isArray(item)?item.join(", "):String(item)]);});return rows; }

function selectUnit(unitId) { app.selectedUnit=unitId; renderMap(); renderUnits(); renderActions(); activateTab("units",false); if(app.tutorial.active&&app.tutorial.step===1&&unitId==="poolster-a835")setTutorialStep(2);if(app.tutorial.active&&app.tutorial.step===8&&unitId==="uss-copeland-ffg25")setTutorialStep(9); }
function activateTab(name,teach=true) { const viewId=name==="staff"?"staff-view":`${name.slice(0,-1)}-view`; $$(".tab").forEach(t=>t.classList.toggle("active",t.dataset.tab===name)); $$(".tab-content").forEach(v=>v.classList.toggle("active",v.id===viewId)); if(teach&&app.tutorial.active&&app.tutorial.step===2&&name==="actions")setTutorialStep(3);if(teach&&app.tutorial.active&&app.tutorial.step===9&&name==="actions")setTutorialStep(10); }

function openAction(actionId) {
  const action=app.actions.find(a=>a.action_id===actionId); if(!action) return; app.selectedAction=action;
  $("#dialog-title").textContent=app.actionNames[action.action_type]||action.action_type; const fields=$("#action-fields"); fields.innerHTML="";
  if(action.action_type==="move") fields.innerHTML=`<div class="field"><label>新航向 · 0–359°</label><input name="course_deg" type="number" min="0" max="359.99" step="1" value="${app.state.units.find(u=>u.unit_id===action.unit_id)?.course_deg||0}" required></div><div class="field"><label>新速度 · 节</label><input name="speed_knots" type="number" min="0" step="1" value="${app.state.units.find(u=>u.unit_id===action.unit_id)?.speed_knots||0}" required></div>`;
  if(action.action_type==="fire") fields.innerHTML=`<div class="field"><label>目标</label><select name="target_id" required>${action.candidate_targets.map(t=>`<option value="${escapeHtml(t)}">${escapeHtml(t)}</option>`).join("")}</select></div>`;
  $("#dialog-rule").innerHTML=`<strong>${escapeHtml(action.rule.rule_id)} · ${escapeHtml(action.rule.title_zh)}</strong><br>${escapeHtml(action.rule.explanation)}`;
  $("#action-dialog").showModal(); showRule(action.rule);
  if(app.tutorial.active&&app.tutorial.step===3&&actionId==="move:poolster-a835") { $("[name=course_deg]").value="20"; $("[name=speed_knots]").value="15"; setTutorialStep(4); }
  if(app.tutorial.active&&app.tutorial.step===10&&actionId==="fire:uss-copeland-ffg25:rgm-84d")setTutorialStep(11);
}

function showRule(rule) { $("#rule-detail").innerHTML=`<span class="rule-id">${escapeHtml(rule.rule_id)} · ${escapeHtml(rule.verification).toUpperCase()}</span><h3>${escapeHtml(rule.title_zh)}</h3><p>${escapeHtml(rule.explanation)}</p><div class="rule-source">${escapeHtml(rule.source_document)} · § ${escapeHtml(rule.source_section)}</div>`; }

async function submitAction(event) { event.preventDefault(); const action=app.selectedAction; if(!action)return; const form=new FormData(event.currentTarget); const parameters={}; for(const [key,value] of form.entries()) parameters[key]=key==="target_id"?value:Number(value); try { if(app.tutorial.active&&app.tutorial.step===11&&action.action_type==="fire"){await api("/api/tutorial/salvo",{method:"POST",body:"{}"});$("#action-dialog").close();setTutorialStep(12);toast("四枚 Harpoon 齐射已加入计划");await refresh();return;} const result=await api("/api/submit",{method:"POST",body:JSON.stringify({command_id:`ui-${app.state.turn}-${Date.now()}`,side_id:app.side,action_id:action.action_id,observation_version:app.state.version,parameters})}); if(!result.accepted) throw new Error(result.errors.join("；")); $("#action-dialog").close(); toast("命令已加入计划"); if(app.tutorial.active&&app.tutorial.step===4)setTutorialStep(5); await refresh(); } catch(error){toast(error.message,true);} }

async function mutate(path,payload={},message="操作完成") { try { await api(path,{method:"POST",body:JSON.stringify(payload)}); toast(message); await refresh(false); } catch(error){toast(error.message,true);} }
function openAdjudication(weapon) { const hit=confirm(`${weapon.weapon_id} 已抵达 ${weapon.target_id}\n\n确定：命中并掷伤害骰\n取消：未命中`); mutate("/api/adjudicate",{entity_id:weapon.entity_id,hit},hit?"命中裁决完成":"未命中裁决完成"); }
function toast(message,error=false){const node=$("#toast");node.textContent=message;node.className=error?"show error":"show";clearTimeout(app.toastTimer);app.toastTimer=setTimeout(()=>node.className="",2600);}

function renderTutorial() {
  const panel=$("#tutorial-panel"); panel.classList.toggle("hidden",!app.tutorial.active&&app.tutorial.step!==0);
  document.body.classList.toggle("tutorial-active",app.tutorial.active);
  document.body.classList.toggle("ai-opponent",app.aiOpponent);
  $$(".tutorial-focus").forEach(node=>node.classList.remove("tutorial-focus"));
  if(app.tutorial.step<0)return;
  const step=tutorialSteps[app.tutorial.step];
  const chapter=app.tutorial.step<8?0:app.tutorial.step<11?1:app.tutorial.step<18?2:app.tutorial.step<19?3:4;
  $("#tutorial-chapters").innerHTML=["航行","探测","交战","判伤","胜负"].map((name,index)=>`<span class="${index<chapter?"done":index===chapter?"active":""}">${name}</span>`).join("");
  $("#tutorial-step-label").textContent=app.tutorial.step===0?"完整战役训练":`战役进度 ${app.tutorial.step} / 20`;
  $("#tutorial-progress-bar").style.width=`${Math.min(app.tutorial.step/20*100,100)}%`;
  const lesson=phaseLesson(app.state?.phase);
  $("#tutorial-kicker").textContent=step.kicker; $("#tutorial-title").textContent=[7,13,15,17,19].includes(app.tutorial.step)?lesson.title:step.title; $("#tutorial-copy").textContent=[7,13,15,17,19].includes(app.tutorial.step)?lesson.copy:step.copy;
  $("#tutorial-next").textContent=step.button||"请完成高亮操作"; $("#tutorial-next").hidden=!step.button;
  $("#tutorial-exit").hidden=app.tutorial.step===0;
  if(step.focus) document.querySelectorAll(step.focus).forEach(node=>node.classList.add("tutorial-focus"));
}

function phaseLesson(phase){return ({plotting:{title:"计划阶段 · 命令封存",copy:"裁决双方已经锁定的计划，只记录命令数量，不泄露秘密内容；随后进入移动。"},movement:{title:"移动阶段 · 坐标与在途武器",copy:"舰艇按 speed×3÷60 计算三分钟航程；在途导弹的 ETA 减 1，抵达后停在待裁决状态。"},planned_fire:{title:"计划射击 · 射程门与 ETA",copy:"检查 Good 火控解、弹药和最小/最大射程；通过后扣弹药，并用 ceil(距离÷每回合航程) 计算 ETA。"},detection:{title:"探测阶段 · 接触质量",copy:"当前严格想定直接给定 Good 火控解。完整雷达/声呐探测表尚未核验，因此这里只报告来源，不进行虚构骰。"},reaction_fire:{title:"反应射击 · 防御窗口",copy:"此阶段通常安排 SAM/近防射击；当前教学在武器抵达时执行已录入的 ECM/诱饵软杀伤查表，硬杀伤渠道仍按平台能力继续扩展。"},resolution:{title:"结算阶段 · 战备、损伤与胜负",copy:"更新 Condition III→I 的剩余回合，并依据 DP 阈值和想定胜利条件检查胜负，然后时间推进三分钟。"}})[phase]||{title:"阶段执行",copy:"推进当前阶段并查看下方事件明细。"};}

function setTutorialStep(step) { app.tutorial.step=step; if(step===8)activateTab("units",false); renderTutorial(); }
async function startTutorial() { await api("/api/reset",{method:"POST",body:"{}"}); app.tutorial.active=true; app.tutorial.step=1; app.aiOpponent=true; app.side="nato"; app.selectedUnit=null; await refresh(false); }
async function lockTutorialPlayer() { try { await api("/api/lock",{method:"POST",body:JSON.stringify({side_id:"nato"})}); await api("/api/tutorial/opponent",{method:"POST",body:"{}"}); if(app.tutorial.active&&app.tutorial.step===5)setTutorialStep(6);else if(app.tutorial.active&&app.tutorial.step===12)setTutorialStep(13);toast("双方计划已锁定"); await refresh(); } catch(error){toast(error.message,true);} }
async function advanceTutorialPhase(){await api("/api/resolve",{method:"POST",body:"{}"});await refresh();toast("阶段已推进，可展开战报查看计算");}
async function tutorialNext() {
  if(app.tutorial.step===0)return startTutorial();
  if(app.tutorial.step===6){setTutorialStep(7);return;}
  if(app.tutorial.step===7){await advanceTutorialPhase();if(app.state.turn>=1)setTutorialStep(8);else renderTutorial();return;}
  if(app.tutorial.step===13){await advanceTutorialPhase();if(app.state.in_flight_weapons.some(w=>w.owner_side_id==="soviet"&&w.awaiting_adjudication))setTutorialStep(14);return;}
  if(app.tutorial.step===14){await api("/api/tutorial/adjudicate",{method:"POST",body:JSON.stringify({owner_side_id:"soviet"})});await refresh();setTutorialStep(15);toast("P-15M 判伤完成，请展开最新事件");return;}
  if(app.tutorial.step===15){await advanceTutorialPhase();if(app.state.turn>=2)setTutorialStep(16);return;}
  if(app.tutorial.step===16){await lockTutorialPlayer();setTutorialStep(17);return;}
  if(app.tutorial.step===17){await advanceTutorialPhase();if(app.state.in_flight_weapons.some(w=>w.owner_side_id==="nato"&&w.awaiting_adjudication))setTutorialStep(18);return;}
  if(app.tutorial.step===18){await api("/api/tutorial/adjudicate",{method:"POST",body:JSON.stringify({owner_side_id:"nato"})});await refresh();setTutorialStep(19);toast("四枚 Harpoon 已逐枚判伤");return;}
  if(app.tutorial.step===19){await advanceTutorialPhase();setTutorialStep(20);return;}
  if(app.tutorial.step===20){app.tutorial.active=false;app.tutorial.step=-1;renderTutorial();toast("完整战役教学完成");}
}
function exitTutorial(){app.tutorial.active=false;app.tutorial.step=-1;renderTutorial();toast("已退出教学模式");}

$$(".tab").forEach(button=>button.onclick=()=>activateTab(button.dataset.tab));
$("#lock-button").onclick=()=>app.aiOpponent?lockTutorialPlayer():mutate("/api/lock",{side_id:app.side},"本方命令已锁定");
$("#resolve-button").onclick=()=>mutate("/api/resolve",{},"阶段已推进");
$("#reset-button").onclick=()=>{if(confirm("确定重置当前想定？当前对局将被清空。"))mutate("/api/reset",{},"想定已重置");};
$("#dialog-close").onclick=$("#dialog-cancel").onclick=()=>$("#action-dialog").close();
$("#action-form").addEventListener("submit",submitAction);
$("#tutorial-next").onclick=tutorialNext;
$("#tutorial-exit").onclick=exitTutorial;
$("#tutorial-toggle").onclick=()=>{app.tutorial.step=0;app.tutorial.active=false;renderTutorial();};
$("#staff-intent-form").addEventListener("submit",submitStaffIntent);
$("#control-mode").onchange=event=>{app.controlMode=event.target.value;if(app.controlMode==="staff")activateTab("staff");renderStaff();};
$("#scenario-select").onchange=async event=>{try{await api("/api/scenarios/load",{method:"POST",body:JSON.stringify({scenario_id:event.target.value})});const entry=app.scenarios.find(item=>item.scenario_id===event.target.value);app.side=entry.sides[0];app.staffPlan=null;app.staffApprovals=[];app.staffBriefing=null;resetMapView();await refresh(false);renderSideButtons();toast("想定已切换");}catch(error){toast(error.message,true);}};
const mapNode=$("#map");
mapNode.addEventListener("wheel",event=>{event.preventDefault();setMapZoom(app.mapView.zoom*Math.exp(-event.deltaY*.0012),event.clientX,event.clientY);},{passive:false});
mapNode.addEventListener("pointerdown",event=>{if(event.target.closest("button,.tutorial-panel"))return;const v=app.mapView;Object.assign(v,{dragging:true,pointerId:event.pointerId,startX:event.clientX,startY:event.clientY,originX:v.x,originY:v.y});mapNode.classList.add("dragging");mapNode.setPointerCapture(event.pointerId);});
mapNode.addEventListener("pointermove",event=>{const v=app.mapView;if(!v.dragging||v.pointerId!==event.pointerId)return;v.x=v.originX+event.clientX-v.startX;v.y=v.originY+event.clientY-v.startY;applyMapTransform();});
function endMapDrag(event){const v=app.mapView;if(v.pointerId!==event.pointerId)return;v.dragging=false;v.pointerId=null;mapNode.classList.remove("dragging");}
mapNode.addEventListener("pointerup",endMapDrag); mapNode.addEventListener("pointercancel",endMapDrag);
mapNode.addEventListener("dblclick",event=>{if(!event.target.closest("button"))resetMapView();});
$("#map-zoom-in").onclick=()=>setMapZoom(app.mapView.zoom*1.25); $("#map-zoom-out").onclick=()=>setMapZoom(app.mapView.zoom/1.25); $("#map-reset").onclick=resetMapView;
initializeScenarios().catch(error=>toast(error.message,true));
