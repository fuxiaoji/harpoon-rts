/* The instructor points at real controls. Only the referee advances objectives. */
(function (root) {
  'use strict';
  const both = (zh, en) => ({ zh, en });
  const cue = (id, selector, title, body, kind = 'observe', extra = {}) => ({ id, selector, title, body, kind, ...extra });
  const tick = () => cue('execute', '#training-step', both('最后，点击这里执行 1 秒', 'Now execute one simulated second'), both('命令已收到，还没有执行。点击高光按钮；服务器确认实际结果后，才会进入下一目标。', 'The order is queued, not yet executed. Click the highlighted button. The referee advances only after checking the actual result.'), 'execute');
  function planFor(step, context = {}) {
    const unit = Number(context.unit || 1);
    const own = cue('own', `[data-select-unit="${unit}"]`, both('点击这艘蓝方训练舰', 'Click your blue training unit'), both('单击高光里的舰名。选中后，右侧会展开航速、武器和传感器；不需要先在海图上找小图标。', 'Click the highlighted unit name. Its speed, weapons and sensors will open on the right. You do not need to find the small map symbol first.'));
    const contact = cue('contact', '#contact-list [data-select-contact]', both('点击这里选择接触', 'Select the contact here'), both('C1 这样的编号表示己方传感器建立的接触。点击高光里的接触，展开目标详情。', 'A label such as C1 is a contact from your own sensors. Click the highlighted contact to open its details.'));
    const enter = (id, selector, value, title, body) => cue(id, selector, title, body, 'input', { value });
    const order = (id, selector, command, title, body, extra = {}) => cue(id, selector, title, body, 'command', { command, ...extra });
    const inputSpeed = () => enter('speed-value', '#nav-speed', step.target || 12, both(`在这里填入 ${step.target || 12} 节`, `Enter ${step.target || 12} knots here`), both('点击高光输入框，输入要求的航速。若当前数值已经正确，点提示框里的“数值已填好”。', 'Click the highlighted input and enter the requested speed. If it already has the correct value, choose Value is ready below.'));
    const applySpeed = () => order('speed-order', '[data-nav="speed"]', 'set_speed', both('点击“下达命令”', 'Click Apply on the speed row'), both('只改数字不会让舰艇行动。点击这里，等命令回执确认已排队。', 'Editing the number does not move the ship. Click here and wait for the queued-order receipt.'), { field: 'speed_kts', value: step.target || 12 });
    const run = (speed = 15) => cue('run', `#speed-group [data-speed="${speed}"]`, both(`点击 ${speed}×，让时间前进`, `Click ${speed}× to advance time`), both('这是时间倍速，不是舰艇航速。观察右上角时钟；本目标满足时会自动暂停。', 'This changes simulation time, not ship speed. Watch the clock at the upper right. Training pauses when this objective is met.'), 'wait');
    const acknowledge = () => cue('ack', '[data-training="ack"]', both('读完说明，点击这里继续', 'Read the instructions, then click here'), both(step.body.zh + ' 读完后点击金框里的“我已阅读，继续”。', step.body.en + ' Then click Acknowledge and continue in the gold frame.'), 'ack');
    let plan;
    switch (step.objective) {
      case 'ack': plan = [acknowledge()]; break;
      case 'report': plan = [
        cue('read-report', '#event-log', both('先看这块战报区域', 'Review this report area first'), both('最新事件在上方。找出刚才的炮击、箔条或航空记录；展开裁决详情可查看依据。读完后点下面的按钮。', 'Newest events are at the top. Find the gun, chaff or aviation event you just caused. Expand its trace for evidence, then confirm below.'), 'read'),
        cue('ack', '[data-training="ack"]', both('战报看完后，点击这里继续', 'Continue after reviewing the reports'), both('下一步会检查你是否理解了命令与实际结果的区别。', 'The next step checks your understanding of orders and actual outcomes.'), 'ack'),
      ]; break;
      case 'question': plan = [cue('answer', '.training-answers', both('在高光区域选择一个答案', 'Choose an answer in the highlighted area'), both('根据刚才的操作作答。答错可以重选；已完成的操作不会丢失。', 'Answer using the procedure you just performed. You can retry without losing completed practical objectives.'), 'choice')]; break;
      case 'select_unit': plan = [own]; break;
      case 'select_contact': plan = [contact]; break;
      case 'running': plan = [run(15)]; plan[0].body = both('点击后，右上角模拟秒数会开始增加。紧接着的下一步会高光“暂停”，请亲自停止时钟。', 'The tick counter at the upper right will begin increasing. The next step highlights Pause so you can stop the clock yourself.'); break;
      case 'paused': plan = [cue('pause', '#speed-group [data-speed="0"]', both('现在点击这个暂停按钮', 'Now click this Pause button'), both('金色 II 就是暂停。点击后确认时钟停止，再进入知识检查。', 'The highlighted II button pauses time. Click it and check that the clock stops before the knowledge check.'))]; break;
      case 'speed': plan = [inputSpeed(), applySpeed(), tick()]; break;
      case 'course': plan = [
        enter('course-value', '#nav-course', step.target, both('在这里填入航向 90°', 'Enter heading 90° here'), both('北方是 000°，东方是 090°。填入 90；如果已经正确，点“数值已填好”。', 'North is 000° and east is 090°. Enter 90, or confirm below if it is already correct.')),
        order('course-order', '[data-nav="course"]', 'set_course', both('下达航向命令', 'Apply the heading order'), both('点击高光里的按钮。收到排队回执后，还需要推进一秒。', 'Click the highlighted button. After the queued receipt, you still need to advance one second.'), { field: 'course_deg', value: step.target }), tick(),
      ]; break;
      case 'route': plan = [
        ...(context.course === 'assessment' ? [inputSpeed(), applySpeed()] : []),
        enter('point-x', '#nav-x', step.point[0], both('把东向坐标 X 填为 1', 'Set the east coordinate X to 1'), both('X 是东西方向，正数表示向东，单位海里。数值已正确时可以直接确认。', 'X is the east-west coordinate; positive values are east, in nautical miles. Confirm below if it is already correct.')),
        enter('point-y', '#nav-y', step.point[1], both('把北向坐标 Y 填为 0', 'Set the north coordinate Y to 0'), both('Y 是南北方向。X=1、Y=0 表示当前位置东侧的训练点。', 'Y is the north-south coordinate. X=1, Y=0 is the training point east of the starting position.')),
        order('route-order', '[data-nav="goto"]', 'goto', both('点击这里下达航点', 'Click here to order the waypoint'), both('使用这个按钮可以精确设置航点，不用在海图上估计点击位置。', 'This button places the waypoint exactly, without estimating a click position on the map.'), { point: step.point }), tick(),
      ]; break;
      case 'arrive': plan = [run(60)]; plan[0].body = both('点击 60× 后等待舰艇进入海图上的金色训练圈。实际抵达才通过，途中会自动暂停；无需一直按单步。', 'Click 60× and let the ship reach the gold training circle on the map. Actual arrival is required and pauses training automatically. You do not need repeated single-steps.'); break;
      case 'contact': case 'identified_contact': plan = [run(15)]; plan[0].body = both('推进时间，等待右侧接触列表出现 C1。本步需要继续识别时，请等型号出现；达标后自动暂停。', 'Advance time and wait for C1 in the contact list. If identification is required, wait for its class. Training pauses when the required evidence exists.'); break;
      case 'incoming': plan = [run(15)]; plan[0].body = both('点击后等待真实“来袭”警报。探测到来袭会自动暂停，下一步会指向箔条按钮。', 'Wait for an actual INCOMING warning. Detection pauses time; the next step points to the chaff button.'); break;
      case 'depth': plan = [
        enter('depth-value', '#nav-depth', step.target, both('在这里输入潜深 60 米', 'Enter depth 60 metres here'), both('这是命令潜深。填好后还要下达命令，并等待潜艇逐渐到达这个深度。', 'This is ordered depth. Apply the order, then allow time for actual depth to change.')),
        order('depth-order', '[data-nav="depth"]', 'set_depth', both('点击下达潜深命令', 'Apply the depth order'), both('先等命令回执，下一步再加快时间等待下潜完成。', 'Wait for the queued receipt. Next, accelerate time until the descent completes.'), { field: 'depth_m', value: step.target }), run(60),
      ]; break;
      case 'sonar_on': case 'sonar_off': {
        const on = step.objective === 'sonar_on';
        plan = [order('sonar-order', `[data-nav="sonar-${on ? 'on' : 'off'}"]`, 'set_sonar', both(`点击这里${on ? '开启' : '关闭'}主动声纳`, `Click here to turn active sonar ${on ? 'on' : 'off'}`), both('这个按钮只下达命令。收到回执后，再按高光提示执行一秒，检查传感器状态。', 'This button queues the order. After its receipt, follow the next highlight to execute one second and check sensor state.'), { field: 'active', value: on }), tick()];
        break;
      }
      case 'gun_fire': plan = [
        ...(!context.contactSelected ? [contact] : []),
        enter('weapon-value', '#fire-weapon', 'gun_5in', both('在下拉框选择“5英寸舰炮”', 'Choose 5in Gun in this list'), both('点击高光下拉框，选择舰炮。保持限制开火，也不要点击“指派目标”；本课练习人工射击。', 'Open the highlighted list and select the gun. Keep weapons on hold and avoid ASSIGN TARGET; this is a manual-fire exercise.')),
        order('fire-order', '#btn-fire', 'fire', both('点击高光的“射击”按钮', 'Click the highlighted FIRE button'), both('这会提交人工炮击命令。命中或落空都可以完成过程训练。', 'This submits a manual gun order. A hit or miss can both complete the procedure.'), { field: 'weapon', value: 'gun_5in' }), tick(),
      ]; break;
      case 'stop': plan = [order('stop-order', '#btn-stop', 'stop', both('点击这里“停止航行”', 'Click ALL STOP here'), both('这个命令会停车并清空航点。收到命令回执后，再执行一秒核对结果。', 'This stops the ship and clears its waypoints. After the receipt, execute one second to check the result.')), tick()]; break;
      case 'chaff': plan = [order('chaff-order', '#btn-chaff', 'deploy_chaff', both('点击这里释放箔条', 'Click DEPLOY CHAFF here'), both('对来袭作出手动反制。收到排队回执后还要执行一秒；释放成功不保证导弹必定脱靶。', 'Order a manual countermeasure. After the receipt, execute one second. Deployment does not guarantee the missile will miss.')), tick()]; break;
      case 'launch': plan = [order('launch-order', '[data-deck-launch][data-class="helo_asw"]', 'launch_aircraft', both('点击反潜直升机这一行的放飞按钮', 'Launch from the ASW Helicopter row'), both('已经为你滚动到飞行甲板。点击高光按钮，收到回执后推进一秒。', 'The flight deck has been scrolled into view. Click the highlighted launch button, then execute one second after the receipt.'), { field: 'class', value: 'helo_asw' }), tick()]; break;
      case 'recall': plan = [order('recall-order', '#btn-recover', 'recover_aircraft', both('保持母舰选中，点击召回航空器', 'Keep the carrier selected and click RECOVER AIR'), both('这会开始返航流程。下一目标还会单独确认航空器实际回收。', 'This begins return-to-base. The next objective separately verifies actual recovery.')), tick()]; break;
      case 'recovered': plan = [run(15)]; plan[0].body = both('继续时间，等到战报出现真实回收记录。如果刚才已经立即回收，也可点击教学栏“单步执行1秒”核验。', 'Continue time until an actual recovery is reported. If it recovered immediately on recall, you can also use Advance 1 second to verify the evidence.'); break;
      default: plan = [cue('details', '#training-panel', step.title, step.body)];
    }
    const commands = ['speed','course','route','depth','sonar_on','sonar_off','gun_fire','stop','chaff','launch','recall'];
    if (commands.includes(step.objective) && !context.unitSelected) plan.unshift(own);
    return plan;
  }
  function inputMatches(cue, value) {
    return typeof cue.value === 'number' ? String(value).trim() !== '' && Number.isFinite(Number(value)) && Math.abs(Number(value) - cue.value) < 0.01 : value === cue.value;
  }
  function commandMatches(cue, result, unit) {
    const command = result?.command;
    return cue?.kind === 'command' && result.ok && command?.cmd === cue.command && Number(command.unit) === Number(unit)
      && (!cue.field || (typeof cue.value === 'number' ? Math.abs(command[cue.field] - cue.value) < 0.01 : command[cue.field] === cue.value))
      && (!cue.point || (Math.abs(command.waypoints?.at(-1)?.x - cue.point[0]) < 0.01 && Math.abs(command.waypoints?.at(-1)?.y - cue.point[1]) < 0.01));
  }
  function completedByCommand(plan, passed, result, unit) {
    const index = plan.findIndex(c => commandMatches(c, result, unit));
    if (index < 0 || plan.slice(0,index).some(c => c.kind === 'command' && !passed.has(c.id))) return [];
    return plan.slice(0,index+1).map(c => c.id);
  }
  const sessionKey = session => `${session.attempt}:${session.step}:${session.retries}`;
  function placeCoach(target, viewport, size) {
    const gap = 22, margin = 12, width = Math.min(size.width, viewport.width - margin * 2), height = Math.min(size.height, viewport.height - margin * 2);
    const clamp = (n, lo, hi) => Math.max(lo, Math.min(n, hi));
    const candidates = [
      { left: target.left - width - gap, top: target.top + target.height / 2 - height / 2 },
      { left: target.right + gap, top: target.top + target.height / 2 - height / 2 },
      { left: target.left + target.width / 2 - width / 2, top: target.bottom + gap },
      { left: target.left + target.width / 2 - width / 2, top: target.top - height - gap },
    ].map(p => ({ left: clamp(p.left, margin, viewport.width - width - margin), top: clamp(p.top, margin, viewport.height - height - margin), width, height }));
    const overlap = p => Math.max(0, Math.min(p.left + width, target.right) - Math.max(p.left, target.left)) * Math.max(0, Math.min(p.top + height, target.bottom) - Math.max(p.top, target.top));
    return candidates.sort((a, b) => overlap(a) - overlap(b))[0];
  }
  if (typeof module !== 'undefined' && module.exports) module.exports = { planFor, inputMatches, commandMatches, completedByCommand, placeCoach, sessionKey };
  if (!root.document) return;

  const I = root.HarpoonI18n, t = value => I.bilingual(value);
  const tr = (zh, en) => t(both(zh, en));
  let state, active, key = '', cueKey = '', dismissed = false, passed = new Set(), lastTarget;
  let overlay, ring, card, line, status, timer, error = '', unbindDescription;
  function ensure() {
    if (overlay) return;
    overlay = document.createElement('div'); overlay.id = 'training-guide'; overlay.dataset.i18nNative = ''; overlay.hidden = true;
    overlay.innerHTML = '<div class="guide-ring" aria-hidden="true"></div><svg class="guide-leader" aria-hidden="true"><path/><circle r="4"/></svg><section class="guide-card" role="region" aria-labelledby="guide-title"><header><span class="guide-instructor"></span><button type="button" data-guide="hide"></button></header><div class="guide-stage"></div><h2 id="guide-title"></h2><p id="guide-copy"></p><p class="guide-status"></p><p class="guide-error" role="alert" hidden></p><footer><button type="button" data-guide="next"></button><button type="button" data-guide="locate"></button></footer></section>';
    document.body.append(overlay); ring = overlay.querySelector('.guide-ring'); card = overlay.querySelector('.guide-card'); line = overlay.querySelector('svg'); status = overlay.querySelector('.guide-status');
    overlay.addEventListener('click', event => {
      const operation = event.target.closest('[data-guide]')?.dataset.guide;
      if (operation === 'hide') { dismissed = true; hide(); }
      if (operation === 'locate') { scrollTarget(); position(); }
      if (operation === 'next' && active) {
        if (active.kind === 'input') {
          const element = document.querySelector(active.selector);
          if (!element || !inputMatches(active, element.value)) { error = tr(`这里需要填写 ${active.value}。`, `Enter ${active.value} here.`); render(); element?.focus({ preventScroll: true }); return; }
        }
        passed.add(active.id); cueKey = ''; error = ''; sync();
      }
    });
    document.addEventListener('input', inputEvent); document.addEventListener('change', inputEvent);
    root.addEventListener('harpoon:command-result', event => {
      if (!state || !active) return;
      const completed = completedByCommand(planFor(state.step, snapshot()), passed, event.detail, state.session.unit);
      if (completed.length) { completed.forEach(id => passed.add(id)); error = ''; sync(); }
      else if (active.kind === 'command' && event.detail?.command?.cmd === active.command && !event.detail.ok) {
        error = tr('命令未被接受。检查当前选择和参数后再试：', 'The order was not accepted. Check selection and parameters: ') + (event.detail.message || ''); render();
      }
    });
    document.addEventListener('keydown', event => {
      if (event.key === 'Escape' && !overlay.hidden) { dismissed = true; hide(); }
    });
    timer = root.setInterval(sync, 160);
  }
  function inputEvent(event) {
    if (active?.kind === 'input' && event.target.matches(active.selector) && inputMatches(active, event.target.value)) { passed.add(active.id); error = ''; sync(); }
  }
  function snapshot() {
    const ui = root.HarpoonUI?.snapshot() || {};
    return { ...ui, unit: state.session.unit, unitSelected: ui.unit === state.session.unit, contactSelected: ui.contact != null, course: state.session.course_id };
  }
  function hide() {
    if (overlay) overlay.hidden = true;
    cueKey = '';
    document.querySelectorAll('.training-focus').forEach(el => el.classList.remove('training-focus'));
    unbindDescription?.(); unbindDescription = null; lastTarget = null;
  }
  function scrollTarget() {
    const element = targetElement();
    element?.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'instant' });
  }
  function targetElement() {
    const element = active && document.querySelector(active.selector);
    if (element?.closest('.training-body[hidden]')) return document.querySelector('[data-training="minimize"]');
    return element;
  }
  function render() {
    if (!active) return;
    card.querySelector('.guide-instructor').textContent = tr('指挥教官', 'Your instructor');
    const hideButton = card.querySelector('[data-guide="hide"]'); hideButton.textContent = '×'; hideButton.setAttribute('aria-label', tr('暂时收起高光引导', 'Hide guidance temporarily'));
    const plan = planFor(state.step, snapshot()), index = plan.findIndex(c => c.id === active.id);
    card.querySelector('.guide-stage').textContent = tr(`目标 ${state.session.step + 1}/${state.session.step_count} · 操作 ${index + 1}/${plan.length}`, `Objective ${state.session.step + 1}/${state.session.step_count} · Action ${index + 1}/${plan.length}`);
    card.querySelector('h2').textContent = t(active.title); card.querySelector('#guide-copy').textContent = t(active.body);
    const next = card.querySelector('[data-guide="next"]'); next.hidden = !['input','read'].includes(active.kind);
    next.textContent = active.kind === 'read' ? tr('我已查看战报，下一步', 'Reports reviewed, next') : tr('数值已填好，下一步', 'Value is ready, next');
    card.querySelector('[data-guide="locate"]').textContent = tr('重新定位高光', 'Locate highlight');
    const alert = card.querySelector('.guide-error'); alert.textContent = error; alert.hidden = !error;
  }
  function position() {
    if (!active || overlay.hidden) return;
    const element = targetElement();
    if (!element || !element.getClientRects().length) { ring.hidden = true; line.style.display = 'none'; return; }
    if (element !== lastTarget) {
      lastTarget?.classList.remove('training-focus'); unbindDescription?.();
      element.classList.add('training-focus'); lastTarget = element;
      const description = element.getAttribute('aria-describedby'); element.setAttribute('aria-describedby', `${description || ''} guide-copy`.trim());
      unbindDescription = () => { if (description) element.setAttribute('aria-describedby', description); else element.removeAttribute('aria-describedby'); };
      element.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'instant' });
    }
    const bounds = element.getBoundingClientRect();
    let top = Math.max(4, bounds.top - 6), bottom = Math.min(innerHeight - 4, bounds.bottom + 6);
    let left = Math.max(4, bounds.left - 6), right = Math.min(innerWidth - 4, bounds.right + 6);
    for (let parent = element.parentElement; parent && parent !== document.body; parent = parent.parentElement) {
      const style = getComputedStyle(parent);
      const rect = parent.getBoundingClientRect();
      if (/(auto|scroll|hidden|clip)/.test(style.overflowY)) { top = Math.max(top, rect.top); bottom = Math.min(bottom, rect.bottom); }
      if (/(auto|scroll|hidden|clip)/.test(style.overflowX)) { left = Math.max(left, rect.left); right = Math.min(right, rect.right); }
    }
    const visible = bottom > top && right > left;
    ring.hidden = !visible; line.style.display = visible ? '' : 'none';
    if (!visible) { card.style.left = '12px'; card.style.top = '12px'; status.textContent = tr('目标在视窗外，点击“重新定位高光”。', 'The target is outside the viewport. Choose Locate highlight.'); return; }
    Object.assign(ring.style, { left: `${left}px`, top: `${top}px`, width: `${right-left}px`, height: `${bottom-top}px` });
    const rect = { top, bottom, left, right, width:right-left, height:bottom-top };
    const placed = placeCoach(rect, {width:innerWidth,height:innerHeight}, {width:Math.min(340,innerWidth-24),height:card.offsetHeight});
    card.style.left = `${placed.left}px`; card.style.top = `${placed.top}px`;
    const center = { x:(left+right)/2,y:(top+bottom)/2 };
    const start = { x:Math.max(placed.left,Math.min(center.x,placed.left+placed.width)),y:Math.max(placed.top,Math.min(center.y,placed.top+placed.height)) };
    const end = { x:Math.max(left,Math.min(start.x,right)),y:Math.max(top,Math.min(start.y,bottom)) };
    line.querySelector('path').setAttribute('d', `M ${start.x} ${start.y} L ${end.x} ${end.y}`);
    line.querySelector('circle').setAttribute('cx', end.x); line.querySelector('circle').setAttribute('cy', end.y);
    const ui = snapshot(), own = ui.observation?.units?.find(u=>u.id===state.session.unit);
    if (active.kind === 'wait' && ui.speed > 0) status.textContent = state.step.objective === 'running'
      ? tr('时钟已经运行，下一步请亲自暂停。', 'The clock is running; pause it yourself in the next step.')
      : tr(`时间运行中 ${ui.speed}× · 达标后自动暂停`, `Time running ${ui.speed}× · Auto-pause on objective`);
    else if (state.step.objective === 'depth' && own) status.textContent = tr(`当前潜深 ${Number(own.depth_m).toFixed(0)} 米`, `Actual depth ${Number(own.depth_m).toFixed(0)} m`);
    else status.textContent = active.kind === 'execute' ? tr('点击真实控件执行；这张提示卡不会代你操作。', 'Use the actual control; this card does not execute orders.') : tr('金色边框内，就是这一步要操作的位置。', 'The gold frame marks the control for this action.');
    status.dataset.disabled = element.disabled ? 'true' : 'false';
    if (element.disabled) status.textContent = tr('控件暂不可用，请检查选择、弹药或前置条件。', 'This control is unavailable. Check selection, ammunition or prerequisites.');
    if (element.matches('[data-training="minimize"]')) status.textContent = tr('操作区已收起，先点击高光中的“展开讲解”。', 'The controls are collapsed. First click Show explanation in the gold frame.');
  }
  function sync() {
    if (!state || !state.step || state.session.status !== 'active' || dismissed || (document.getElementById('academy-modal') && !document.getElementById('academy-modal').hidden)) { hide(); return; }
    const plan = planFor(state.step, snapshot());
    const next = plan.find(c => !passed.has(c.id)) || plan.at(-1);
    if (!next) return;
    active = next; overlay.hidden = false;
    const nextKey = key + ':' + active.id + ':' + I.language;
    if (nextKey !== cueKey) { cueKey = nextKey; render(); scrollTarget(); }
    position();
  }
  root.HarpoonTrainingGuide = {
    update(value) {
      state = value;
      if (!state?.session || !state.step) { hide(); return; }
      ensure();
      const nextKey = sessionKey(state.session);
      if (nextKey !== key) { key = nextKey; passed = new Set(); cueKey = ''; dismissed = false; error = ''; }
      sync();
    },
    locate() { dismissed = false; cueKey = ''; sync(); scrollTarget(); position(); },
  };
})(typeof window === 'undefined' ? globalThis : window);
