const test = require('node:test');
const assert = require('node:assert/strict');
const { firingState, ruleReport, traceZh, eventNote } = require('../web/combat-help.js');
const { translateText } = require('../web/i18n.js');

test('assigned targets still auto-engage with free selection off, including contact zero', () => {
  assert.match(firingState({assigned:0,fire_on:false}), /C0.*持续自动交战/);
  assert.doesNotMatch(firingState({assigned:7,fire_on:false}), /人工攻击/);
  assert.match(firingState({assigned:7,fire_on:true},'en'), /free-target fallback/);
  assert.match(firingState({assigned:null,fire_on:true}), /自动选敌开火/);
  assert.match(firingState({assigned:null,fire_on:false}), /人工攻击/);
});
test('close-intercept explanation preserves probability, die and damage values', () => {
  const event={rule:'air_burst',detail:'PD-band intercept at 1.56 nmi; fragment strike chance 59%; airburst critical d20=17: fire +30s -> 68s'};
  const zh=ruleReport(event);
  assert.match(zh.meaning,/不等于整枚导弹直接命中/);
  for(const value of ['1.56','59%','17','30','68']) assert.ok(zh.detail.includes(value));
  assert.match(zh.detail,/火灾持续时间增加 30 秒，累计 68 秒/);
  assert.equal(ruleReport(event,'en').detail,event.detail);
});
test('system ratios, lost ammunition and unmodelled effects stay distinct', () => {
  assert.match(traceZh('damage control restored mobility/fire_control on DDG-101; systems now mobility 0.20, radar 1.00, sonar 1.00, fire_control 0.20'), /机动 0.20、雷达 1.00、声纳 1.00、火控 0.20/);
  const prefix='PD-band intercept at 0.50 nmi; fragment strike chance 68%; airburst critical d20=1: ';
  assert.match(traceZh(prefix+'weapon mount rounds 32 -> 16 (half)'), /32 减至 16/);
  assert.match(traceZh(prefix+'cargo/superstructure; no modeled effect'), /未施加额外效果/);
  assert.match(traceZh(prefix+'sensor (sonar) -50% -> 0.50'), /声纳能力降低50个百分点，剩余比例 0.50/);
});
test('zero relayed tracks do not become an attack or a new detection', () => {
  const report=ruleReport({rule:'tdl_share',detail:'TDL net 2 unit(s): 0 track(s) relayed (q×0.9), 0 delayed to non-TDL units (q×0.5), 4 track(s) re-shared'});
  assert.match(report.detail,/转发 0 条接触/);
  assert.match(report.detail,/重新共享 4 条/);
  assert.match(report.meaning,/不是发射或命中/);
  assert.match(report.detail,/不是命中率/);
});
test('interception failure is never described as a successful interception', () => {
  assert.match(eventNote({event:'intercept',intercepted:false},'zh-CN'),/未成功/);
  assert.doesNotMatch(eventNote({event:'intercept',intercepted:false},'en'),/succeeded/);
  assert.match(eventNote({event:'intercept',intercepted:true},'en'),/succeeded/);
});
test('unknown rule or trace formats are disclosed without invented adjudication', () => {
  const unknown=ruleReport({rule:'new_rule',detail:'future trace 123'});
  assert.equal(unknown.known,false);
  assert.equal(unknown.detail,null);
  assert.match(unknown.meaning,/暂未提供/);
  assert.equal(ruleReport({rule:'air_burst',detail:'new fragment model 123'}).detail,null);
});
test('dogfight reports distinguish probability from actual hit and retain numeric state', () => {
  const text=traceZh('pass 2: F-18 (maneuver 3, 400 kt, 2000 m) vs MiG (maneuver 2, 450 kt, 3000 m); gun pk 0.120; hit=false; target hp 20.0/30.0');
  assert.match(text,/命中概率 12.0%；本次未命中/);
  for(const value of ['400','2000','450','3000','20.0 / 30.0']) assert.ok(text.includes(value));
});
test('firing labels tell players which operation is single or automatic', () => {
  assert.equal(translateText('FIRE 1 ROUND','zh-CN'),'人工射击1发');
  assert.equal(translateText('AUTO-ENGAGE TARGET','zh-CN'),'指定目标自动交战');
  assert.equal(translateText('AUTO SELECT OFF','zh-CN'),'自动选敌：关');
});
