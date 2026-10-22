const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { translateText } = require('../web/i18n.js');

test('individual aircraft recall uses the individual order without recalling nearby patrols',()=>{
  const app=fs.readFileSync(path.join(__dirname,'../web/app.js'),'utf8');
  assert.match(app,/unit.kind === 'Aircraft' \? 'return_to_base' : 'recover_aircraft'/);
  assert.equal(translateText('RETURN TO BASE','zh-CN'),'单机返航');
});

test('AI orders have their own log filter and stay visible in important reports',()=>{
  const app=fs.readFileSync(path.join(__dirname,'../web/app.js'),'utf8');
  const html=fs.readFileSync(path.join(__dirname,'../web/command.html'),'utf8');
  const css=fs.readFileSync(path.join(__dirname,'../web/style.css'),'utf8');
  assert.match(app,/includes\(event.domain\)\) return 'tactical'/);
  assert.match(html,/data-log-filter="tactical"/);
  assert.doesNotMatch(css,/data-filter="important"\]\s+\[data-category="tactical"/);
});

test('language adapter translates dynamic presentation without changing numeric values', () => {
  assert.equal(translateText('NEWEST FIRST · 37 EVENTS', 'zh-CN'), '最新在前 · 37 条记录');
  assert.equal(translateText('12 KT · 5 NM', 'zh-CN'), '12 节 · 5 海里');
  assert.equal(translateText('Long-Range Air Radar', 'zh-CN'), '远程对空雷达');
});
test('mission panel, launch feedback and follow-CAP are wired', () => {
  const app = fs.readFileSync(path.join(__dirname, '../web/app.js'), 'utf8');
  assert.match(app, /renderMissionsPanel/);
  assert.match(app, /missions-list/);
  assert.match(app, /mission-feedback/);
  assert.match(app, /LAUNCH REFUSED/);
  assert.match(app, /event\.reason/);
  assert.match(app, /radius_nm: 25, follow: true/);
  const html = fs.readFileSync(path.join(__dirname, '../web/command.html'), 'utf8');
  assert.match(html, /missions-panel/);
  assert.match(html, /任务队列 \/ TASKS/);
});

test('kill breakdown, weighted score and WS stall watchdog are wired', () => {
  const app = fs.readFileSync(path.join(__dirname, '../web/app.js'), 'utf8');
  assert.match(app, /own_kills_by_kind/);
  assert.match(app, /kindBreakdownText/);
  assert.match(app, /WEIGHTED SCORE|加权比分/);
  assert.match(app, /own_in_flight/);
  assert.match(app, /lastWsFrameAt/);
  assert.match(app, /WS_STALL_MS/);
  const theater = fs.readFileSync(path.join(__dirname, '../web/theater.js'), 'utf8');
  assert.match(theater, /kill_weights/);
  assert.match(theater, /own_kill_points/);
});

test('battle result scoreboard, popups and agent auto-run controls are wired', () => {
  const html = fs.readFileSync(path.join(__dirname, '../web/command.html'), 'utf8');
  for (const id of ['score-strip', 'score-kills', 'score-losses', 'battle-popups', 'battle-result-dialog', 'battle-result-close', 'agent-auto', 'agent-speed']) {
    assert.ok(html.includes(`id="${id}"`), `missing #${id}`);
  }
  const js = fs.readFileSync(path.join(__dirname, '../web/app.js'), 'utf8');
  for (const token of ['announceBattleEvents', 'showBattleResultDialog', 'auto_run', 'auto_speed', 'changeGameSpeed']) {
    assert.ok(js.includes(token), `missing ${token}`);
  }
});
test('English source and stable technical identifiers remain exact', () => {
  const original = 'Long-Range Air Radar · 12 KT';
  assert.equal(translateText(original, 'en'), original);
  assert.equal(translateText('gun_5in / academy_bridge / 777101', 'zh-CN'), 'gun_5in / academy_bridge / 777101');
});
test('Chinese text is stable under repeated presentation passes', () => {
  const translated = translateText('Surface Search Radar · 6 NM', 'zh-CN');
  assert.equal(translateText(translated, 'zh-CN'), translated);
});
test('every course has a unique scenario, complete bilingual guidance and a valid quiz', () => {
  const data = JSON.parse(fs.readFileSync(path.join(__dirname, '../data/training/courses.json'), 'utf8'));
  const ids = new Set(), scenarios = new Set();
  for (const course of data.courses) {
    assert.ok(!ids.has(course.id)); ids.add(course.id);
    assert.ok(!scenarios.has(course.scenario)); scenarios.add(course.scenario);
    const scenario = JSON.parse(fs.readFileSync(path.join(__dirname, '../scenarios', course.scenario + '.json'), 'utf8'));
    assert.equal(scenario.id, course.scenario);
    const steps = new Set();
    for (const step of course.steps) {
      assert.ok(!steps.has(step.id)); steps.add(step.id);
      for (const field of ['title', 'body', 'why']) for (const lang of ['zh', 'en']) assert.ok(step[field][lang].trim());
      // The three practical lessons added upstream use a shorter optional hint
      // ladder. Keep the bilingual guarantee without forcing an exact count.
      for (const hint of step.hints || []) for (const lang of ['zh', 'en']) assert.ok(hint[lang].trim());
      if (step.objective === 'question') assert.ok(step.options.some(o => o.id === step.answer));
    }
  }
  assert.equal(ids.size, 11);
  assert.equal(data.courses.reduce((n, c) => n + c.steps.length, 0), 76);
  assert.ok(!fs.existsSync(path.join(__dirname, '../web/training/courses.json')), 'answer keys must stay outside static web root');
});
