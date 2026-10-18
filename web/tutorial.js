/* Tutorial overlay — guided first-watch lessons for new commanders. */
(function () {
  'use strict';

  const API = '';
  let stepIndex = 0;
  let active = false;
  let pollTimer = null;
  let prevState = { speed: 0, contacts: 0, assigned: null, gunSeen: false, chaffSeen: false, lastEventSeq: 0 };

  const STEPS = [
    {
      id: 'time',
      title: '欢迎来到舰桥，舰长',
      text: '这是实时兵棋：时间可以暂停、加速。先让时间流动起来 —— 点击上方的 <b>1×</b> 或 <b>60×</b>（推荐 15×，教学节奏刚好）。',
      hint: '点击 TIME CONTROL 里任意速度按钮',
      check: async () => (await fetchTime()).speed > 0,
    },
    {
      id: 'select',
      title: '接管你的战舰',
      text: '海图上的蓝色符号是你的 <b>TUTOR DDG</b>（宙斯盾驱逐舰）。<b>左键点击它</b>，右侧就会展开这艘船的武器与传感器面板。',
      hint: '左键点击蓝色驱逐舰符号',
      check: async () => {
        const st = await fetchState();
        return document.querySelector('#unit-name') && /TUTOR|Aegis|DDG/i.test(document.querySelector('#unit-name').textContent || '');
      },
    },
    {
      id: 'course',
      title: '下达第一个航向',
      text: '敌导弹艇正在逼近。保持选舰状态，<b>左键点击海图上的开阔海域</b> —— 会让战舰转向该点。去迎敌：点你舰艏前方约 20 海里的位置。',
      hint: '左键点击开阔海域 = 追加航路点',
      check: async () => {
        const st = await fetchState();
        const me = st.units && st.units[0];
        return me && Math.abs(me.ordered_course_deg - 45) > 5;
      },
    },
    {
      id: 'detect',
      title: '雷达开机，等待接触',
      text: '保持时间流动。当敌舰进入雷达地平（约 12 海里），左侧海图会出现 <b>CONTACT</b> 接触符号，事件流会弹出 DETECTION —— 双盲规则：你看到的只是估计位置，不是真身。',
      hint: '等待 DETECTION 事件（可加速时间）',
      check: async () => (await fetchState()).contacts.length > 0,
    },
    {
      id: 'assign',
      title: '指派目标',
      text: '<b>左键点击接触符号</b>，右侧 TARGET DESIGNATION 面板会显示火控解质量（Q 值）。点 <b>ASSIGN TARGET</b> 把它交给火控系统。',
      hint: '点接触 → ASSIGN TARGET',
      check: async () => {
        const st = await fetchState();
        const me = st.units && st.units[0];
        return me && me.assigned !== null && me.assigned !== undefined;
      },
    },
    {
      id: 'fire',
      title: '开火',
      text: '敌舰进入 11 海里舰炮射程后（目标面板显示 IN RANGE），点 <b>WEAPONS FREE</b> 解除射击限制，再在武器表里选 <b>5in Gun</b> 点 <b>FIRE</b>。火控解质量越高，命中越稳。',
      hint: 'WEAPONS FREE → 选 gun_5in → FIRE',
      check: async () => {
        const evs = await fetchEvents();
        return evs.some((e) => e.event === 'gun_action');
      },
    },
    {
      id: 'defense',
      title: '敌袭防御（可跳过）',
      text: '挨打是教学的一部分。若事件流出现 <b>INCOMING</b>（来袭导弹），立刻点 <b>DEPLOY CHAFF</b> 施放箔条，并保持武器自由让防空系统自动拦截。若敌舰已被击沉，直接进入下一课。',
      hint: 'INCOMING 时点 DEPLOY CHAFF（或敌沉后继续）',
      check: async () => {
        const st = await fetchState();
        const evs = await fetchEvents();
        const chaff = evs.some((e) => e.event === 'chaff_deployed');
        const sunk = !st.contacts.length && st.units.length >= 1 && evs.some((e) => e.event === 'contact_sank' || e.event === 'gun_action');
        return chaff || sunk || st.over;
      },
    },
    {
      id: 'done',
      title: '课程完成',
      text: '你已经掌握了 航行-探测-指派-开火-防御 的完整循环。接下来试试 <b>Operation Crimson Tide</b>：航母战斗群全要素作战（舰载机、反潜、对岸打击）。祝武运昌隆，舰长。',
      hint: '点 FREE PLAY 返回自由指挥',
      check: async () => false,
    },
  ];

  function el(tag, cls, html) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (html !== undefined) e.innerHTML = html;
    return e;
  }

  function buildCard() {
    const card = el('div', 'tutorial-card');
    card.id = 'tutorial-card';
    card.innerHTML =
      '<div class="tutorial-head"><span class="tutorial-kicker">TUTORIAL</span>' +
      '<span class="tutorial-step" id="tutorial-step"></span></div>' +
      '<h3 class="tutorial-title" id="tutorial-title"></h3>' +
      '<div class="tutorial-text" id="tutorial-text"></div>' +
      '<div class="tutorial-hint" id="tutorial-hint"></div>' +
      '<div class="tutorial-actions">' +
      '<button class="cmd-btn" id="tutorial-next" type="button">NEXT ▸</button>' +
      '<button class="cmd-btn tutorial-skip" id="tutorial-skip" type="button">FREE PLAY</button>' +
      '</div>';
    document.body.appendChild(card);
    document.getElementById('tutorial-next').addEventListener('click', () => advance(true));
    document.getElementById('tutorial-skip').addEventListener('click', stop);
  }

  function render() {
    const step = STEPS[stepIndex];
    document.getElementById('tutorial-step').textContent = `STEP ${stepIndex + 1} / ${STEPS.length}`;
    document.getElementById('tutorial-title').textContent = step.title;
    document.getElementById('tutorial-text').innerHTML = step.text;
    document.getElementById('tutorial-hint').textContent = '▸ ' + step.hint;
    const last = stepIndex === STEPS.length - 1;
    document.getElementById('tutorial-next').textContent = last ? 'FREE PLAY ▸' : 'NEXT ▸';
  }

  async function fetchTime() {
    try {
      return await (await fetch(API + '/api/time')).json();
    } catch {
      return { speed: 0, tick: 0 };
    }
  }

  async function fetchState() {
    try {
      return await (await fetch(API + '/api/state?side=Blue&max_events=50')).json();
    } catch {
      return { units: [], contacts: [], events: [] };
    }
  }

  async function fetchEvents() {
    const st = await fetchState();
    return Array.isArray(st.events) ? st.events : [];
  }

  async function tickCheck() {
    if (!active) return;
    try {
      const step = STEPS[stepIndex];
      if (step && step.check && (await step.check())) {
        advance(false);
      }
    } catch {
      /* tutorial never blocks the game */
    }
  }

  function advance(manual) {
    if (!active) return;
    if (stepIndex >= STEPS.length - 1) {
      stop();
      return;
    }
    stepIndex += 1;
    render();
    void tickCheck();
  }

  function start() {
    if (active) return;
    active = true;
    stepIndex = 0;
    buildCard();
    render();
    // 教程想定自动开局（若当前不是教程局则切换）
    fetch(API + '/api/scenarios')
      .then((r) => r.json())
      .then((list) => {
        const t = list.find((s) => s.id === 'tutorial_01');
        if (!t) return;
        return fetch(API + '/api/game', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({ scenario: 'tutorial_01', speed: 0 }),
        });
      })
      .catch(() => {});
    pollTimer = setInterval(tickCheck, 1500);
  }

  function stop() {
    active = false;
    if (pollTimer) clearInterval(pollTimer);
    const card = document.getElementById('tutorial-card');
    if (card) card.remove();
  }

  // 暴露给 index.html 的入口按钮
  window.HarpoonTutorial = { start, stop };

  // 自绑定入口按钮（defer 脚本运行时 DOM 已就绪，只绑一次）
  const wire = () => {
    const btn = document.getElementById('btn-tutorial');
    if (btn && !btn.dataset.tutorialWired) {
      btn.dataset.tutorialWired = '1';
      btn.addEventListener('click', () => (active ? stop() : start()));
    }
    if (new URLSearchParams(window.location.search).get('tutorial') === '1') start();
  };
  if (document.readyState === 'loading') {
    window.addEventListener('DOMContentLoaded', wire, { once: true });
  } else {
    wire();
  }
})();
