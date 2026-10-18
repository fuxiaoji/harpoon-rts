const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { translateText } = require('../web/i18n.js');

test('language adapter translates dynamic presentation without changing numeric values', () => {
  assert.equal(translateText('NEWEST FIRST · 37 EVENTS', 'zh-CN'), '最新在前 · 37 条记录');
  assert.equal(translateText('12 KT · 5 NM', 'zh-CN'), '12 节 · 5 海里');
  assert.equal(translateText('Long-Range Air Radar', 'zh-CN'), '远程对空雷达');
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
      assert.equal(step.hints.length, 3);
      for (const hint of step.hints) for (const lang of ['zh', 'en']) assert.ok(hint[lang].trim());
      if (step.objective === 'question') assert.ok(step.options.some(o => o.id === step.answer));
    }
  }
  assert.equal(ids.size, 8);
  assert.equal(data.courses.reduce((n, c) => n + c.steps.length, 0), 55);
  assert.ok(!fs.existsSync(path.join(__dirname, '../web/training/courses.json')), 'answer keys must stay outside static web root');
});
