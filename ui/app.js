const app = {
  side: "nato", state: null, actions: [], selectedUnit: null, selectedAction: null,
  phaseNames: { plotting:"计划", movement:"移动", planned_fire:"计划射击", detection:"探测", reaction_fire:"反应射击", resolution:"结算" },
  actionNames: { move:"机动", fire:"武器发射", wait:"保持当前命令", air_mission:"航空任务", air_attack:"空中攻击", submarine_attack:"潜艇攻击", land_strike:"对陆打击", electronic_warfare:"电子战", mine_warfare:"水雷战" }
};

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
  renderMap(); renderUnits(); renderActions(); renderEvents();
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
    const button=document.createElement("button"); button.className=`unit-card ${unit.unit_id===app.selectedUnit?"active":""}`;
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
  $("#event-log").innerHTML=events.length?events.map(event=>`<article class="event-card ${event.random_record?"random":""}"><span class="event-meta">T${event.turn} · ${escapeHtml(app.phaseNames[event.phase]||event.phase)}</span><h3>${escapeHtml(event.event_type)}</h3><p>${escapeHtml(event.actors.join(" → ")||"系统事件")}</p>${event.random_record?`<p>D6: ${escapeHtml(event.random_record.rolls.join(", "))}</p>`:""}</article>`).join(""):`<div class="empty-state">尚无事件。提交命令并推进阶段开始推演。</div>`;
}

function selectUnit(unitId) { app.selectedUnit=unitId; renderMap(); renderUnits(); renderActions(); activateTab("units"); }
function activateTab(name) { $$(".tab").forEach(t=>t.classList.toggle("active",t.dataset.tab===name)); $$(".tab-content").forEach(v=>v.classList.toggle("active",v.id===`${name.slice(0,-1)}-view`)); }

function openAction(actionId) {
  const action=app.actions.find(a=>a.action_id===actionId); if(!action) return; app.selectedAction=action;
  $("#dialog-title").textContent=app.actionNames[action.action_type]||action.action_type; const fields=$("#action-fields"); fields.innerHTML="";
  if(action.action_type==="move") fields.innerHTML=`<div class="field"><label>新航向 · 0–359°</label><input name="course_deg" type="number" min="0" max="359.99" step="1" value="${app.state.units.find(u=>u.unit_id===action.unit_id)?.course_deg||0}" required></div><div class="field"><label>新速度 · 节</label><input name="speed_knots" type="number" min="0" step="1" value="${app.state.units.find(u=>u.unit_id===action.unit_id)?.speed_knots||0}" required></div>`;
  if(action.action_type==="fire") fields.innerHTML=`<div class="field"><label>目标</label><select name="target_id" required>${action.candidate_targets.map(t=>`<option value="${escapeHtml(t)}">${escapeHtml(t)}</option>`).join("")}</select></div>`;
  $("#dialog-rule").innerHTML=`<strong>${escapeHtml(action.rule.rule_id)} · ${escapeHtml(action.rule.title_zh)}</strong><br>${escapeHtml(action.rule.explanation)}`;
  $("#action-dialog").showModal(); showRule(action.rule);
}

function showRule(rule) { $("#rule-detail").innerHTML=`<span class="rule-id">${escapeHtml(rule.rule_id)} · ${escapeHtml(rule.verification).toUpperCase()}</span><h3>${escapeHtml(rule.title_zh)}</h3><p>${escapeHtml(rule.explanation)}</p><div class="rule-source">${escapeHtml(rule.source_document)} · § ${escapeHtml(rule.source_section)}</div>`; }

async function submitAction(event) { event.preventDefault(); const action=app.selectedAction; if(!action)return; const form=new FormData(event.currentTarget); const parameters={}; for(const [key,value] of form.entries()) parameters[key]=key==="target_id"?value:Number(value); try { const result=await api("/api/submit",{method:"POST",body:JSON.stringify({command_id:`ui-${app.state.turn}-${Date.now()}`,side_id:app.side,action_id:action.action_id,observation_version:app.state.version,parameters})}); if(!result.accepted) throw new Error(result.errors.join("；")); $("#action-dialog").close(); toast("命令已加入计划"); await refresh(); } catch(error){toast(error.message,true);} }

async function mutate(path,payload={},message="操作完成") { try { await api(path,{method:"POST",body:JSON.stringify(payload)}); toast(message); await refresh(false); } catch(error){toast(error.message,true);} }
function openAdjudication(weapon) { const hit=confirm(`${weapon.weapon_id} 已抵达 ${weapon.target_id}\n\n确定：命中并掷伤害骰\n取消：未命中`); mutate("/api/adjudicate",{entity_id:weapon.entity_id,hit},hit?"命中裁决完成":"未命中裁决完成"); }
function toast(message,error=false){const node=$("#toast");node.textContent=message;node.className=error?"show error":"show";clearTimeout(app.toastTimer);app.toastTimer=setTimeout(()=>node.className="",2600);}

$$(".side-button").forEach(button=>button.onclick=()=>{app.side=button.dataset.side;app.selectedUnit=null;refresh(false);});
$$(".tab").forEach(button=>button.onclick=()=>activateTab(button.dataset.tab));
$("#lock-button").onclick=()=>mutate("/api/lock",{side_id:app.side},"本方命令已锁定");
$("#resolve-button").onclick=()=>mutate("/api/resolve",{},"阶段已推进");
$("#reset-button").onclick=()=>{if(confirm("确定重置 Tattletail 想定？当前对局将被清空。"))mutate("/api/reset",{},"想定已重置");};
$("#dialog-close").onclick=$("#dialog-cancel").onclick=()=>$("#action-dialog").close();
$("#action-form").addEventListener("submit",submitAction);
refresh(false);
