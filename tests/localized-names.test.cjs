const test=require('node:test');
const assert=require('node:assert/strict');
const names=require('../web/localized-names.js');
test('Chinese names distinguish weapon variants and leave IDs untouched',()=>{
  assert.equal(names.name('weapon','cn_yj18','YJ-18','zh-CN'),'鹰击-18反舰导弹');
  assert.notEqual(names.name('weapon','cn_yj12','YJ12','zh-CN'),names.name('weapon','cn_yj12_air','YJ12','zh-CN'));
  assert.equal(names.name('weapon','cn_yj18','YJ-18','en'),'YJ-18');
  assert.equal(names.text('gun_5in / 777101','zh-CN'),'gun_5in / 777101');
});
test('Authored callsigns translate while custom and unknown-contact names stay intact',()=>{
  const unit={id:4,name:'TYPE-055 A',class:'cn_055'};
  assert.equal(names.unit(unit,'zh-CN'),'055型 A');
  assert.equal(names.unit({name:'BLUE SSN-1'},'zh-CN'),'蓝方 攻击核潜艇1');
  assert.equal(names.unit({name:'My custom name'},'zh-CN'),'My custom name');
  assert.equal(names.text('UNKNOWN C17','zh-CN'),'UNKNOWN C17');
  assert.equal(unit.name,'TYPE-055 A');
});
