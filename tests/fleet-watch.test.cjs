const test=require('node:test'),assert=require('node:assert/strict'),{picture}=require('../web/fleet-watch.js');
test('Attention remains while own casualty exists and does not read contacts or foreign truth',()=>{
 const obs={your_side:'Blue',units:[{id:1,side:'Blue',kind:'Ship',functional_damage:{'damage.fire_pct':3}},{id:2,side:'Red',kind:'Ship',functional_damage:{'damage.fire_pct':7}}],contacts:[{id:9,category:'Ship'}]};
 assert.deepEqual(picture(obs).attention.map(a=>a.unit.id),[1]);assert.equal(picture(obs).attention.length,1);obs.units[0].functional_damage={};assert.equal(picture(obs).attention.length,0);
});
test('Air task groups preserve individual members and distinct missions',()=>{
 const obs={your_side:'Blue',units:[{id:1,side:'Blue',kind:'Aircraft',class:'a',mission:'cap'},{id:2,side:'Blue',kind:'Aircraft',class:'a',mission:'cap',manual_control:true},{id:3,side:'Blue',kind:'Aircraft',class:'a',mission:'rtb'}]};
 assert.deepEqual(picture(obs).tasks.map(g=>g.units.map(u=>u.id)),[[1,2],[3]]);
});
test('Same aircraft and mission type with different own targets remain separate tasks',()=>{
 const obs={your_side:'Blue',units:[{id:1,side:'Blue',kind:'Aircraft',class:'a',mission:'strike',air_task:{Strike:{contact:17,released:false}}},{id:2,side:'Blue',kind:'Aircraft',class:'a',mission:'strike',air_task:{Strike:{contact:19,released:false}}}]};
 assert.equal(picture(obs).tasks.length,2);assert.equal(picture(obs).tasks[0].task.Strike.contact,17);
});
