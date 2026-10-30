const test=require('node:test');
const assert=require('node:assert/strict');
const {batteryText,ruleReport,traceZh}=require('../web/combat-help.js');

test('v4 source-unknown battery displays unadjudicated rather than zero/full or the legacy speed rule',()=>{
  for(const battery of [null,undefined,0,5,100]) {
    const own={kind:'Sub',battery};
    const zh=batteryText(own,'hv2026-rts-v4','zh-CN');
    const en=batteryText(own,'hv2026-rts-v4','en');
    assert.match(zh,/尚未裁定.*不代表无限/);
    assert.match(en,/unadjudicated.*does not mean unlimited/);
    for(const text of [zh,en]) assert.doesNotMatch(text,/\/100|半速|halves|below20|<20/);
  }
  assert.equal(batteryText({kind:'Ship',battery:null},'hv2026-rts-v4'),'');
});

test('historical charge values retain their normalized semantics without turning missing charge into zero',()=>{
  for(const profile of ['rts-legacy-v1','hv2026-rts-v2','hv2026-rts-v3']) {
    assert.equal(batteryText({kind:'Sub',battery:null},profile),'');
    assert.equal(batteryText({kind:'Sub'},profile),'');
    assert.match(batteryText({kind:'Sub',battery:0},profile),/历史.*0\/100/);
    assert.match(batteryText({kind:'Sub',battery:95},profile,'en'),/Historical.*95\/100/);
  }
});

test('old battery-low event explains the historical model rather than claiming a v4 source gate',()=>{
  const event={domain:'asw',rule:'battery_low',detail:'battery 5.0% below 20%; max speed halved to 10.0 kt'};
  assert.match(ruleReport(event).meaning,/旧规则.*不是v4.*待裁定/);
  assert.match(ruleReport(event,'en').meaning,/historical.*not v4.*unadjudicated/);
  assert.match(traceZh(event.detail),/历史模型.*不是v4来源/);
});
