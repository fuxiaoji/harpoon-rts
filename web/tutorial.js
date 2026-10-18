/* Fleet Training Academy: the server owns objectives and checkpoints. */
(() => {
  'use strict';
  const I = window.HarpoonI18n;
  const b = (zh, en) => I.bilingual({ zh, en });
  const text = value => I.bilingual(value);
  const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({ '&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;' }[c]));
  let courses = [], view = { session: null, completed: [] }, selectedCourse = null;
  let busy = false, polling = false, hintLevel = 0, renderKey = '', minimized = false;
  let modal, panel, lastFocused;

  async function request(path, payload) {
    const response = await fetch('/api/tutorial/' + path, payload === undefined ? {} : {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(payload),
    });
    if (!response.ok) {
      const code = await response.text();
      const error = new Error(code); error.status = response.status; throw error;
    }
    return response.json();
  }
  function course() { return courses.find(c => c.id === view.session?.course_id); }
  function currentStep() { return course()?.steps[view.session?.step]; }
  function feedback(code) {
    const messages = {
      incorrect_answer: b('再想一想。可以展开“为什么”或提示后重答。', 'Try again. Review Why or reveal a hint before answering.'),
      checkpoint_restored: b('已恢复当前检查点，命令队列已清空，模拟已暂停。', 'Checkpoint restored. The order queue is clear and time is paused.'),
      training_unit_lost: b('训练单位已损失。查看己方战报，然后从当前检查点重试。', 'Your training unit was lost. Review its reports, then retry this checkpoint.'),
      scenario_ended: b('训练想定已结束。请重试检查点或重新开始本课。', 'The training scenario ended. Retry a checkpoint or restart the lesson.'),
      training_threat_missing: b('训练波次没有形成有效练习机会。请重试当前检查点；本次不记作玩家失败。', 'The training wave did not create a valid practice opportunity. Retry this checkpoint; this is not a learner failure.'),
      stale_attempt: b('训练状态已更新，正在同步当前步骤。', 'Training has advanced. Synchronizing the current objective.'),
      objective_requires_evidence: b('本步骤需要实际操作及执行证据，不能直接跳过。', 'This objective requires an actual action and execution evidence.'),
    };
    return messages[code] || b('操作暂未完成，请检查连接并重试。', 'The action could not complete. Check the connection and retry.');
  }
  function announce(message) {
    let live = document.getElementById('training-announcement');
    if (!live) { live = document.createElement('div'); live.id = 'training-announcement'; live.className = 'sr-only'; live.setAttribute('aria-live', 'polite'); document.body.append(live); }
    live.textContent = message;
  }
  async function load() {
    const data = await request('catalog'); courses = data.courses;
    I.register(courses.flatMap(c => [
      [`Academy ${c.number} — ${c.title.en}`, `训练 ${c.number}：${c.title.zh}`],
      [c.summary.en, c.summary.zh],
    ]));
    I.registerScenarios(Object.fromEntries(courses.map(c => [c.scenario, {
      title: {zh:`训练 ${c.number}：${c.title.zh}`,en:`Academy ${c.number}: ${c.title.en}`}, summary:c.summary,
    }])));
    selectedCourse ||= courses[0]?.id;
    await poll();
  }
  async function poll() {
    if (polling || busy) return;
    polling = true;
    try {
      const next = await request('status');
      const key = JSON.stringify(next.session && [next.session.attempt, next.session.step, next.session.status, next.session.feedback, next.session.retries, next.session.mistakes]);
      const changed = key !== renderKey;
      if (changed) { hintLevel = 0; renderKey = key; }
      view = next;
      if (changed) {
        renderPanel();
        if (view.session) {
          announce(text(currentStep()?.title) || b('课程已完成', 'Lesson complete'));
          await window.HarpoonUI?.refresh();
        }
      }
      const progress = document.getElementById('training-time');
      if (progress && view.session) progress.textContent = b('模拟用时 ', 'Simulation time ') + Math.max(0, view.session.elapsed_s) + b(' 秒', ' seconds');
    } catch {
      if (panel) {
        const link = panel.querySelector('#training-connection');
        if (link) { link.hidden = false; link.textContent = b('训练状态暂时离线；恢复连接后继续。', 'Training status is offline; it will resume after reconnecting.'); }
      }
    } finally { polling = false; }
  }
  async function start(id) {
    if (busy) return;
    busy = true;
    try {
      view = await request('start', { course: id }); hintLevel = 0; minimized = false; renderKey = '';
      await window.HarpoonUI.adoptTraining();
      closeCatalog(); renderPanel();
    } catch (error) { showError(feedback(error.message)); }
    finally { busy = false; }
  }
  async function action(name, extra = {}) {
    const session = view.session;
    if (!session || busy) return;
    busy = true;
    try {
      view = await request('action', { action: name, attempt: session.attempt, step: session.step, ...extra });
      hintLevel = 0; renderKey = '';
      if (name === 'retry') await window.HarpoonUI.adoptTraining();
      else await window.HarpoonUI.refresh();
      renderPanel();
    } catch (error) {
      if (error.message !== 'stale_attempt') showError(feedback(error.message));
    } finally { busy = false; await poll(); }
  }
  function showError(message) {
    const target = document.getElementById('training-error') || document.getElementById('training-connection');
    if (target) { target.hidden = false; target.textContent = message; }
    announce(message);
  }
  function ensureModal() {
    if (modal) return;
    modal = document.createElement('div'); modal.id = 'academy-modal'; modal.className = 'academy-overlay'; modal.hidden = true;
    modal.dataset.i18nNative = ''; modal.innerHTML = '<section class="academy-dialog" role="dialog" aria-modal="true" aria-labelledby="academy-title"></section>';
    document.body.append(modal);
    modal.addEventListener('click', event => {
      const button = event.target.closest('button'); if (!button) return;
      if (button.dataset.course) { selectedCourse = button.dataset.course; renderCatalog(); }
      if (button.dataset.start) void start(button.dataset.start);
      if (button.dataset.close !== undefined) closeCatalog();
    });
    modal.addEventListener('change', event => {
      if (event.target.matches('[data-academy-language]')) I.setLanguage(event.target.value);
    });
    modal.addEventListener('keydown', event => {
      if (event.key === 'Escape') { event.preventDefault(); closeCatalog(); return; }
      if (event.key !== 'Tab') return;
      const items = [...modal.querySelectorAll('button:not([disabled]),select:not([disabled]),[href],[tabindex="0"]')].filter(el => !el.hidden);
      const first = items[0], last = items.at(-1);
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
      if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
    });
  }
  async function openCatalog() {
    ensureModal(); lastFocused = document.activeElement; modal.hidden = false;
    if (!courses.length) {
      modal.firstElementChild.innerHTML = `<h1 id="academy-title">${b('正在连接训练中心…', 'Connecting to training…')}</h1><button data-close>${b('关闭','Close')}</button>`;
      try { await load(); } catch { modal.firstElementChild.innerHTML = `<h1 id="academy-title">${b('暂时无法载入课程', 'Curriculum unavailable')}</h1><p>${b('检查服务连接后重新打开训练中心。','Check the server connection, then reopen the academy.')}</p><button data-close>${b('关闭','Close')}</button>`; return; }
    }
    renderCatalog(); modal.querySelector('[data-close]')?.focus();
  }
  function closeCatalog() { if (modal) modal.hidden = true; lastFocused?.focus(); }
  function renderCatalog() {
    if (!modal || modal.hidden) return;
    const focused = document.activeElement;
    const focusSelector = focused?.matches('[data-academy-language]') ? '[data-academy-language]' :
      focused?.dataset.course ? `[data-course="${focused.dataset.course}"]` :
      focused?.hasAttribute('data-close') ? '[data-close]' : null;
    const selected = courses.find(c => c.id === selectedCourse) || courses[0];
    modal.firstElementChild.innerHTML = `
      <header class="academy-header"><div><span class="academy-mark">HARPOON / ACADEMY</span><h1 id="academy-title">${b('舰队训练中心', 'Fleet Training Academy')}</h1><p>${b('从第一次值更，到独立指挥。', 'From your first watch to independent command.')}</p></div><div class="academy-header-controls"><select data-academy-language aria-label="语言 / Language"><option value="zh-CN" ${I.language==='zh-CN'?'selected':''}>简体中文</option><option value="en" ${I.language==='en'?'selected':''}>English</option></select><button data-close aria-label="${b('关闭课程目录','Close course catalogue')}">×</button></div></header>
      <div class="academy-layout"><nav class="academy-courses" aria-label="${b('课程目录','Course catalogue')}">${courses.map(c => `
        <button class="academy-course ${c.id === selected.id ? 'is-current' : ''}" data-course="${c.id}" aria-pressed="${c.id === selected.id}"><span class="course-number">${String(c.number).padStart(2,'0')}</span><span><b>${esc(text(c.title))}</b><small>${c.steps.length} ${b('个目标','objectives')} · ${b('约','about ')}${c.duration} ${b('分钟','min')}</small></span><span class="course-status">${view.completed.includes(c.id) ? '✓' : '›'}</span></button>`).join('')}</nav>
      <article class="academy-preview"><span class="academy-phase">${b('指挥员训练 / ', 'Commander training / ')}${view.completed.length} / ${courses.length} ${b('已结业','completed')}</span>
        <h2>${esc(text(selected.title))}</h2><p class="academy-summary">${esc(text(selected.summary))}</p>
        <h3>${b('本课你将完成','Your objectives')}</h3><ol>${selected.steps.map(s => `<li>${esc(text(s.title))}</li>`).join('')}</ol>
        <div class="academy-contract"><b>${b('真实引擎，明确的学习证据','A real engine, clear learning evidence')}</b><p>${b('操作目标由服务端核验。说明页可确认，操作步骤不能跳过；命中与损伤保留真实随机结果。操作达成后暂停；时间控制练习等待你主动暂停。','The server verifies your objectives. Briefings can be acknowledged; practical objectives cannot be skipped. Hits and damage keep their real random outcomes. Practical objectives pause on completion; the time-control exercise waits for you to pause.')}</p></div>
        <p class="academy-reset-note">${b('开始课程会重置当前战局。结业记录保留；进行中的课程可通过关闭目录继续。','Starting a lesson resets the current battle. Completion records are retained; close this catalogue to resume an active lesson.')}</p>
        <div id="training-error" class="training-alert" role="alert" hidden></div>
        <footer><button class="academy-primary" data-start="${selected.id}">${view.completed.includes(selected.id) ? b('再次训练','Train again') : b('开始本课','Begin lesson')}</button>${view.session ? `<button data-close>${b('继续当前训练','Resume current training')}</button>` : ''}</footer>
      </article></div>`;
    if (focusSelector) modal.querySelector(focusSelector)?.focus();
  }
  function removeHighlight() { window.HarpoonTrainingGuide?.update(null); }
  function focusTarget(scroll = false) {
    window.HarpoonTrainingGuide?.update({ session: view.session, step: currentStep() });
    if (scroll) window.HarpoonTrainingGuide?.locate();
  }
  function renderPanel() {
    if (!panel) {
      panel = document.createElement('section'); panel.id = 'training-panel'; panel.dataset.i18nNative = '';
      panel.setAttribute('aria-label', b('当前训练任务','Current training objective'));
      document.querySelector('.workspace').before(panel);
      panel.addEventListener('click', event => {
        const button = event.target.closest('button'); if (!button) return;
        const op = button.dataset.training;
        if (op === 'ack') void action('ack');
        if (op === 'step') void action('step');
        if (op === 'retry') void action('retry');
        if (op === 'exit') void action('exit');
        if (op === 'restart') void start(view.session.course_id);
        if (op === 'catalogue') void openCatalog();
        if (op === 'pause') void window.HarpoonUI.setSpeed(0);
        if (op === 'resume') void window.HarpoonUI.setSpeed(15);
        if (op === 'fast') void window.HarpoonUI.setSpeed(60);
        if (op === 'focus') focusTarget(true);
        if (op === 'hint') { hintLevel = Math.min(3, hintLevel + 1); renderPanel(); }
        if (op === 'minimize') { minimized = !minimized; renderPanel(); }
        if (button.dataset.answer !== undefined) void action('answer', { answer: button.dataset.answer });
      });
    }
    const session = view.session, c = course(), step = currentStep();
    panel.setAttribute('aria-label', b('当前训练任务','Current training objective'));
    panel.hidden = !session; document.body.classList.toggle('training-active', !!session);
    document.getElementById('side-toggle').querySelectorAll('button').forEach(el => { el.disabled = !!session; });
    document.getElementById('scenario-select').disabled = !!session;
    document.getElementById('btn-start').disabled = !!session;
    if (!session || !c) { removeHighlight(); return; }
    if (session.status === 'completed') {
      removeHighlight();
      panel.innerHTML = `<div class="training-result"><div class="training-pass">✓</div><div><span>${b('本课结业','Lesson complete')}</span><h2>${esc(text(c.title))}</h2><p>${session.step_count} ${b('个目标已核验','verified objectives')} · ${session.mistakes} ${b('次知识检查重答','quiz retries')} · ${session.retries} ${b('次检查点重试','checkpoint retries')}</p><p>${b('你完成了指挥流程训练。该成绩不代表随机交战必胜。','You completed this command procedure. This result does not imply guaranteed combat success.')}</p>${!view.progress_saved ? `<p class="training-alert">${b('记录未写入磁盘，本次会话中仍可查看。','The record could not be written to disk; it remains available in this session.')}</p>` : ''}</div><div class="training-result-actions"><button class="academy-primary" data-training="catalogue">${b('选择下一课','Choose next lesson')}</button><button data-training="restart">${b('再次训练','Train again')}</button><button data-training="exit">${b('自由指挥','Free play')}</button></div></div>`;
      return;
    }
    if (!step) return;
    const objectiveKey = `${session.attempt}:${session.step}`;
    if (panel.dataset.objective !== objectiveKey) {
      minimized = !['ack','report','question'].includes(step.objective);
      panel.dataset.objective = objectiveKey;
    }
    const manual = ['ack','report'].includes(step.objective);
    panel.classList.toggle('is-minimized', minimized);
    panel.innerHTML = `
      <header class="training-heading"><div><span>${String(c.number).padStart(2,'0')} / ${esc(text(c.title))}</span><h2>${session.step+1}. ${esc(text(step.title))}</h2></div><div class="training-heading-actions"><span>${session.step} / ${session.step_count}</span><button data-training="focus">${b('显示操作高光','Show action highlight')}</button><button data-training="minimize" aria-expanded="${!minimized}">${minimized ? b('展开讲解','Show explanation') : b('收起讲解','Hide explanation')}</button><button data-training="catalogue">${b('课程目录','Courses')}</button></div></header>
      <div class="training-progress" role="progressbar" aria-label="${b('课程进度','Lesson progress')}" aria-valuenow="${session.step}" aria-valuemin="0" aria-valuemax="${session.step_count}"><i style="width:${100*session.step/session.step_count}%"></i></div>
      <div class="training-body" ${minimized ? 'hidden' : ''}><div class="training-instruction"><p class="training-objective">${esc(text(step.body))}</p>
        <div id="training-connection" class="training-alert" role="status" hidden></div>
        ${session.feedback === 'incorrect_answer' || session.status === 'failed' ? `<p class="training-alert" role="alert">${feedback(session.feedback)}</p>` : ''}
        ${step.objective === 'question' ? `<div class="training-answers">${step.options.map(option => `<button data-answer="${option.id}" ${session.status === 'failed' ? 'disabled' : ''}>${esc(text(option.text))}</button>`).join('')}</div>` : ''}
        <details><summary>${b('为什么要这样做？','Why does this matter?')}</summary><p>${esc(text(step.why))}</p></details>
        <div class="training-hints">${step.hints.slice(0,hintLevel).map((h,i) => `<p><b>${b('提示','Hint')} ${i+1}</b> ${esc(text(h))}</p>`).join('')}</div>
        <div class="training-actions"><button class="academy-primary" data-training="ack" ${!manual || session.status !== 'active' ? 'disabled' : ''}>${manual ? b('我已阅读，继续','Acknowledge and continue') : b('等待实际操作达成','Waiting for actual execution')}</button><button data-training="focus">${b('定位控件','Locate control')}</button><button data-training="hint" ${hintLevel>=3 ? 'disabled' : ''}>${b('再给一个提示','Reveal a hint')} (${hintLevel}/3)</button></div>
      </div><aside class="training-checklist"><h3>${b('训练目标','Lesson objectives')}</h3><ol>${c.steps.map((s,i) => `<li class="${i<session.step?'is-done':i===session.step?'is-active':''}"><span>${i<session.step?'✓':i+1}</span>${esc(text(s.title))}</li>`).join('')}</ol><details><summary>${b('规则与实现依据','Rules and implementation')}</summary><p>${b('本课使用已声明的RTS改编，不代表严格桌面规则已完整录入。训练几何和起始姿态是想定设计。','This lesson uses the declared RTS adaptation; it does not claim complete strict tabletop implementation. Training geometry and initial posture are scenario design.')}</p>${c.source_refs.map(ref=>`<p class="training-source">${esc(ref)}</p>`).join('')}</details></aside></div>
      <footer class="training-toolbar"><span id="training-time">${b('模拟用时 ','Simulation time ')}${Math.max(0,session.elapsed_s)}${b(' 秒',' seconds')}</span><div><button data-training="pause">${b('暂停','Pause')}</button><button data-training="resume">15×</button><button data-training="fast">60×</button><button data-training="step" id="training-step" ${session.status !== 'active' ? 'disabled' : ''}>${b('单步执行 1 秒','Advance 1 second')}</button></div><div><button data-training="retry">${b('重试检查点','Retry checkpoint')}</button><button data-training="exit">${b('退出训练','Exit training')}</button></div></footer>`;
    focusTarget();
  }
  window.addEventListener('harpoon:selection', event => {
    const step = currentStep(); if (!step || busy || view.session?.status !== 'active') return;
    if (step.objective === 'select_unit' && event.detail.unit === view.session.unit) void action('select_unit', { unit: event.detail.unit });
    if (step.objective === 'select_contact' && event.detail.contact !== null) void action('select_contact', { contact: event.detail.contact });
  });
  window.addEventListener('harpoon:language', () => { renderCatalog(); renderPanel(); if (view.session) announce(text(currentStep()?.title) || b('课程已完成','Lesson complete')); });
  window.HarpoonTraining = { goalPoint: () => currentStep()?.objective === 'arrive' ? currentStep().point : null };
  document.getElementById('btn-tutorial').addEventListener('click', () => void openCatalog());
  void load().catch(() => {});
  window.setInterval(() => void poll(), 750);
})();
