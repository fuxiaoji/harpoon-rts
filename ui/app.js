const app = {
  side: "nato", state: null, actions: [], selectedUnit: null, selectedAction: null,
  phaseNames: { plotting:"计划", movement:"移动", planned_fire:"计划射击", detection:"探测", reaction_fire:"反应射击", resolution:"结算" },
  actionNames: { move:"机动", fire:"武器发射", wait:"保持当前命令", air_mission:"航空任务", air_attack:"空中攻击", submarine_attack:"潜艇攻击", land_strike:"对陆打击", electronic_warfare:"电子战", mine_warfare:"水雷战" },
  tutorial: { active:false, step:0 }, aiOpponent:false
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
  { kicker:"命中裁决边界", title:"教学裁判处理 P-15M", copy:"注意：权威命中表尚未逐表核验，所以引擎不会伪造命中概率。本教学明确采用“裁判预设命中”，随后只自动执行已核验的损伤公式。", button:"裁判预设命中并查看判伤" },
  { kicker:"完成第二回合", title:"继续到下一计划阶段", copy:"计划射击阶段会发射四枚 Harpoon；探测阶段报告 Good 火控解的来源；结算阶段更新战备和胜负。", button:"推进并解释当前阶段" },
  { kicker:"导弹飞行", title:"锁定等待命令，进入第三回合", copy:"这一回合无需新机动。双方保持命令，重点观察四枚 Harpoon 如何在移动阶段抵达。", button:"双方保持并锁定" },
  { kicker:"导弹飞行", title:"推进到 Harpoon 齐射抵达", copy:"查看海图上的在途实体和发射事件中的射程、速度、ETA 公式。", button:"推进并解释当前阶段" },
  { kicker:"损伤查表", title:"逐枚裁决四枚 Harpoon", copy:"教学裁判预设四枚命中。每枚伤害 = 40 + floor(D6÷2) DP；事件将逐项显示骰值、累计 DP、50% 重创线和沉没线。", button:"裁判预设命中并逐枚判伤" },
  { kicker:"胜负判定", title:"推进到结算阶段", copy:"Smyshlennyy 达到最大 DP 后沉没；结算阶段依据苏舰状态与 NATO 重创舰数量给出胜利等级。", button:"推进并解释当前阶段" },
  { kicker:"完整战役完成", title:"NATO 取得决定性胜利", copy:"你已经走完移动、探测/火控、ROE、武器发射、导弹飞行、人工命中边界、自动 DP 判伤和胜负判定。可展开任一事件复盘全部计算。", button:"结束教学并查看战报" }
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
  renderMap(); renderUnits(); renderActions(); renderEvents(); renderTutorial();
}

function coordinates(position) {
  const scale = 28;
  return { left:`calc(50% + ${position.x_nm * scale}px)`, top:`calc(50% - ${position.y_nm * scale}px)` };
}

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
  $("#event-log").innerHTML=events.length?events.map(event=>`<article class="event-card ${event.random_record?"random":""}"><span class="event-meta">T${event.turn} · ${escapeHtml(app.phaseNames[event.phase]||event.phase)}</span><h3>${escapeHtml(eventName(event.event_type))}</h3><p>${escapeHtml(event.actors.join(" → ")||"系统事件")}</p>${event.random_record?`<p>D6: ${escapeHtml(event.random_record.rolls.join(", "))}</p>`:""}<details><summary>查看结算明细</summary><div class="trace-grid">${flattenTrace(event.payload).map(([key,value])=>`<div><b>${escapeHtml(key)}</b><span>${escapeHtml(value)}</span></div>`).join("")||"<div><span>此事件没有额外数值输入</span></div>"}</div><div class="event-rules">规则：${escapeHtml(event.rule_ids.join(" · ")||"想定胜负条件")}</div></details></article>`).join(""):`<div class="empty-state">尚无事件。提交命令并推进阶段开始推演。</div>`;
}

function eventName(type){return ({orders_locked:"双方命令锁定",unit_moved:"单位移动结算",weapon_launched:"武器发射",weapon_arrival_requires_adjudication:"武器抵达待裁决",weapon_adjudicated:"命中与损伤裁决",contact_solution_report:"接触与火控解报告",readiness_transition_started:"战备转换查表",attack_word_check:"攻击命令检定",victory:"胜负判定",launch_rejected_at_resolution:"发射被拒绝"})[type]||type;}
function flattenTrace(value,prefix="") { const rows=[]; Object.entries(value||{}).forEach(([key,item])=>{const label=prefix?`${prefix}.${key}`:key;if(item&&typeof item==="object"&&!Array.isArray(item))rows.push(...flattenTrace(item,label));else rows.push([label,Array.isArray(item)?item.join(", "):String(item)]);});return rows; }

function selectUnit(unitId) { app.selectedUnit=unitId; renderMap(); renderUnits(); renderActions(); activateTab("units",false); if(app.tutorial.active&&app.tutorial.step===1&&unitId==="poolster-a835")setTutorialStep(2);if(app.tutorial.active&&app.tutorial.step===8&&unitId==="uss-copeland-ffg25")setTutorialStep(9); }
function activateTab(name,teach=true) { $$(".tab").forEach(t=>t.classList.toggle("active",t.dataset.tab===name)); $$(".tab-content").forEach(v=>v.classList.toggle("active",v.id===`${name.slice(0,-1)}-view`)); if(teach&&app.tutorial.active&&app.tutorial.step===2&&name==="actions")setTutorialStep(3);if(teach&&app.tutorial.active&&app.tutorial.step===9&&name==="actions")setTutorialStep(10); }

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

function phaseLesson(phase){return ({plotting:{title:"计划阶段 · 命令封存",copy:"裁决双方已经锁定的计划，只记录命令数量，不泄露秘密内容；随后进入移动。"},movement:{title:"移动阶段 · 坐标与在途武器",copy:"舰艇按 speed×3÷60 计算三分钟航程；在途导弹的 ETA 减 1，抵达后停在待裁决状态。"},planned_fire:{title:"计划射击 · 射程门与 ETA",copy:"检查 Good 火控解、弹药和最小/最大射程；通过后扣弹药，并用 ceil(距离÷每回合航程) 计算 ETA。"},detection:{title:"探测阶段 · 接触质量",copy:"当前严格想定直接给定 Good 火控解。完整雷达/声呐探测表尚未核验，因此这里只报告来源，不进行虚构骰。"},reaction_fire:{title:"反应射击 · 当前能力边界",copy:"此阶段通常处理 SAM、近防炮、诱饵和干扰；这些表尚未 verified，当前严格子集不自动裁决，并在覆盖台账中保持阻塞。"},resolution:{title:"结算阶段 · 战备、损伤与胜负",copy:"更新 Condition III→I 的剩余回合，并依据 DP 阈值和想定胜利条件检查胜负，然后时间推进三分钟。"}})[phase]||{title:"阶段执行",copy:"推进当前阶段并查看下方事件明细。"};}

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
  if(app.tutorial.step===19){await advanceTutorialPhase();if(app.state.winner)setTutorialStep(20);return;}
  if(app.tutorial.step===20){app.tutorial.active=false;app.tutorial.step=-1;renderTutorial();toast("完整战役教学完成");}
}
function exitTutorial(){app.tutorial.active=false;app.tutorial.step=-1;renderTutorial();toast("已退出教学模式");}

$$(".side-button").forEach(button=>button.onclick=()=>{if(app.tutorial.active||app.aiOpponent)return;app.side=button.dataset.side;app.selectedUnit=null;refresh(false);});
$$(".tab").forEach(button=>button.onclick=()=>activateTab(button.dataset.tab));
$("#lock-button").onclick=()=>app.aiOpponent?lockTutorialPlayer():mutate("/api/lock",{side_id:app.side},"本方命令已锁定");
$("#resolve-button").onclick=()=>mutate("/api/resolve",{},"阶段已推进");
$("#reset-button").onclick=()=>{if(confirm("确定重置 Tattletail 想定？当前对局将被清空。"))mutate("/api/reset",{},"想定已重置");};
$("#dialog-close").onclick=$("#dialog-cancel").onclick=()=>$("#action-dialog").close();
$("#action-form").addEventListener("submit",submitAction);
$("#tutorial-next").onclick=tutorialNext;
$("#tutorial-exit").onclick=exitTutorial;
$("#tutorial-toggle").onclick=()=>{app.tutorial.step=0;app.tutorial.active=false;renderTutorial();};
refresh(false);
