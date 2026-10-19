const test=require('node:test');const assert=require('node:assert/strict');
const db=require('../database/consolidated.json');const weapons=new Map(Object.entries(db.weapons));
const {roles}=require('../web/aircraft-roles.js');
test('volume aircraft receive executable tasks from equipment rather than English ID fragments',()=>{
  for(const id of ['cn_j15','us_f18e_asm']){assert.equal(roles(db.platforms[id],weapons).primary,'cap');assert.equal(roles(db.platforms[id],weapons).strike,true);}
  assert.equal(roles(db.platforms.us_f18e,weapons).strike,false);
  for(const id of ['cn_z18f','us_mh60r'])assert.equal(roles(db.platforms[id],weapons).primary,'asw');
  for(const id of ['cn_z18j','us_e2d'])assert.equal(roles(db.platforms[id],weapons).primary,'recon');
  assert.equal(roles(db.platforms.us_e2d,weapons).strike,false);
});
