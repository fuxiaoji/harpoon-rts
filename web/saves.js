/* Local hotseat save manager. Authoritative payloads remain server-side. */
(() => {
  'use strict';
  const t=(zh,en)=>localStorage.getItem('harpoon.language')==='en'?en:zh;
  const button=document.getElementById('manage-saves'); if(!button)return;
  const dialog=document.createElement('dialog'); dialog.id='save-dialog';
  dialog.style.cssText='background:#0a1c2b;color:#dcecf3;border:1px solid #4b778c;max-width:620px;width:90vw;padding:24px';
  dialog.innerHTML='<h2></h2><p class="save-help"></p><label class="save-label"><span></span> <input id="save-label" maxlength="100"></label><p><button id="save-now"></button> <button id="save-refresh"></button></p><select id="save-list" size="7" style="width:100%;min-height:160px" aria-label="存档 / Saves"></select><p><button id="save-load"></button> <button id="save-replay"></button> <button id="save-verify"></button> <button id="save-close"></button></p><p id="save-status" role="status" aria-live="polite"></p>';
  document.body.append(dialog);
  const $=id=>document.getElementById(id); let saves=[],busy=false;
  const status=message=>{$('save-status').textContent=message;};
  function lock(value){busy=value;for(const id of ['save-now','save-refresh','save-load','save-replay','save-verify'])$(id).disabled=value;}
  async function request(url,body){const r=await fetch(url,body===undefined?{}:{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});if(!r.ok)throw Error(await r.text());return r.json();}
  async function refresh(){
    const previous=$('save-list').value;const data=await request('/api/saves');saves=data.saves||[];
    $('save-list').replaceChildren(...saves.map(s=>{const o=new Option(s.invalid?`${s.id} · ${t('文件损坏','Invalid file')}`:`${s.label||s.scenario} · T+${s.tick}s · ${s.over?t('已终局','Completed'):t('进行中','In progress')}`,s.id);o.disabled=!!s.invalid;return o;}));
    if(saves.some(s=>s.id===previous))$('save-list').value=previous;
    if(!saves.length)status(t('还没有存档。点击“保存并暂停”创建第一份。','No saves yet. Choose Save and pause.'));
  }
  function localize(){
    dialog.querySelector('h2').textContent=t('存档与读档','Save and load');
    dialog.querySelector('.save-help').textContent=t('保存作战世界、待执行命令及状态机记忆。读档会替换当前战局并保持暂停；大模型记忆保留，需手动重新启用。训练仍使用课程检查点。','Save the operation, queued orders and FSM memory. Loading replaces the current battle and pauses it. LLM memory is retained; enable the assistant manually. Training uses course checkpoints.');
    dialog.querySelector('.save-label span').textContent=t('存档名称','Save name');
    for(const [id,zh,en] of [['save-now','保存并暂停','Save and pause'],['save-refresh','刷新列表','Refresh'],['save-load','读取所选存档','Load selected'],['save-replay','打开终局复盘','Replay completed save'],['save-verify','校验命令重演','Verify replay'],['save-close','返回','Back']])$(id).textContent=t(zh,en);
  }
  button.onclick=async()=>{localize();dialog.showModal();status('');lock(true);try{await refresh();}catch(e){status(t('读取失败：','Cannot read saves: ')+e.message);}finally{lock(false);}};
  $('save-close').onclick=()=>dialog.close();
  $('save-refresh').onclick=async()=>{lock(true);try{await refresh();}catch(e){status(e.message);}finally{lock(false);}};
  $('save-now').onclick=async()=>{lock(true);status(t('正在保存…','Saving…'));try{const s=await request('/api/saves',{label:$('save-label').value});await refresh();$('save-list').value=s.id;status(t(`已保存并暂停：T+${s.tick}s`,`Saved and paused at T+${s.tick}s`));}catch(e){status(t('保存失败，当前战局保留：','Save failed; battle retained: ')+e.message);}finally{lock(false);}};
  $('save-load').onclick=async()=>{
    if(busy)return;const s=saves.find(s=>s.id===$('save-list').value);if(!s||s.invalid)return;
    if(!confirm(t(`读取“${s.label||s.scenario}”将替换当前战局。请先保存需要保留的进度。继续？`,`Load “${s.label||s.scenario}” and replace the current battle? Save your current progress first.`)))return;
    lock(true);try{await request('/api/saves/'+encodeURIComponent(s.id)+'/load',{});location.href='command.html?side='+((new URLSearchParams(location.search).get('side')||localStorage.getItem('harpoon.side'))==='Red'?'Red':'Blue');}catch(e){status(t('读档失败，当前战局保留：','Load failed; battle retained: ')+e.message);lock(false);}
  };
  $('save-verify').onclick=async()=>{const id=$('save-list').value;if(!id)return;lock(true);status(t('正在按原始命令重演…','Replaying recorded commands…'));try{const result=await request('/api/saves/'+encodeURIComponent(id)+'/verify',{});status(t(`重演一致：T+${result.tick}s · ${result.world_fnv64}`,`Replay verified: T+${result.tick}s · ${result.world_fnv64}`));}catch(e){status(t('校验失败：','Verification failed: ')+e.message);}finally{lock(false);}};
  $('save-replay').onclick=()=>{const s=saves.find(s=>s.id===$('save-list').value);if(!s?.over){status(t('完整双方复盘仅在终局后开放。','Full replay opens after the battle ends.'));return;}location.href='replay.html?save='+encodeURIComponent(s.id);};
})();
