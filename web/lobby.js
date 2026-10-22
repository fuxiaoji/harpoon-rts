/* Local game front end. Browsing is read-only; launch requires explicit confirmation. */
(() => {
  'use strict';
  const I=window.HarpoonI18n,M=window.HarpoonLobbyModel,$=id=>document.getElementById(id);
  const en=()=>I.language==='en',t=(zh,eng)=>en()?eng:zh;
  const text=value=>typeof value==='string'?value:value?.[en()?'en':'zh']||'';
  const esc=value=>String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const clock=n=>[Math.floor(n/3600),Math.floor(n/60)%60,Math.floor(n)%60].map(v=>String(v).padStart(2,'0')).join(':');
  const store=(key,value)=>{try{localStorage.setItem(key,value);}catch{}};
  const read=key=>{try{return localStorage.getItem(key);}catch{return null;}};
  const state={scenarios:[],metadata:{},courses:[],training:{completed:[],session:null},trainingKnown:false,recordings:[],current:null,time:null,online:false,currentKnown:false,selectedScenario:null,selectedCourse:null,seat:'Blue',seed:'',opponent:M.opponentValue(read('harpoon.opponent'),read('harpoon.agentOpponent')==='1'?'llm':'human'),tacticalTakeover:read('harpoon.tacticalTakeover')==='1',assistantMode:['off','semi','full'].includes(read('harpoon.assistantMode'))?read('harpoon.assistantMode'):'off',matchSpeed:Number(read('harpoon.matchSpeed'))||15,pending:null,busy:false,loading:false};
  let launchFocus=null;
  const title=s=>I.scenario(s?.id)?.title||s?.name||'';
  const summary=s=>I.scenario(s?.id)?.summary||String(s?.description||'').slice(0,600);
  const mode=m=>!m?t('控制方式未标注','Control mode unspecified'):m.automated_side?t('规则参谋对抗','Rule-based staff opposition'):t('双边指挥','Two-seat command');
  const modeFor=s=>s?.id==='dual_carrier_duel'?t('可配置 AI 对手','Configurable AI opponent'):mode(state.metadata[s?.id]);
  const group=s=>s.id==='dual_carrier_hotseat'?'hotseat':['dual_carrier_denial','fleet_decisive_3h','carrier_duel_3h','crimson_tide','pacific_storm'].includes(s.id)?'large':'practice';
  const operations=()=>state.scenarios.filter(s=>!s.id.startsWith('academy_'));
  function notice(message,error=false){$('lobby-notice').hidden=!message;$('lobby-notice').textContent=message;$('lobby-notice').dataset.error=String(error);}
  async function request(path,payload,allowEmpty=false){
    const abort=new AbortController(),timer=setTimeout(()=>abort.abort(),8000);
    try{const response=await fetch(path,{cache:'no-store',signal:abort.signal,...(payload===undefined?{}:{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(payload)})});
      if(allowEmpty&&response.status===409)return null;
      if(!response.ok){const error=Error('HTTP_'+response.status);error.status=response.status;throw error;}
      return await response.json();
    }finally{clearTimeout(timer);}
  }
  async function currentData(){
    const data=await Promise.allSettled([request('/api/health'),request('/api/state?side=Blue&max_events=0',undefined,true),request('/api/time'),request('/api/tutorial/status')]);
    state.online=data[0].status==='fulfilled'&&data[0].value.ok===true;
    state.currentKnown=data[1].status==='fulfilled';
    if(state.currentKnown)state.current=data[1].value;
    if(data[2].status==='fulfilled')state.time=data[2].value;
    state.trainingKnown=data[3].status==='fulfilled';
    if(state.trainingKnown)state.training=data[3].value;
    renderCurrent();return data;
  }
  async function load(){
    if(state.loading)return;state.loading=true;$('refresh-lobby').disabled=true;
    const data=await Promise.allSettled([currentData(),request('/api/scenarios'),request('/api/tutorial/catalog'),request('lobby-catalog.json'),request('replays/catalog.json'),request('lobby-local.json'),request('/api/saves')]);
    if(data[1].status==='fulfilled'&&Array.isArray(data[1].value))state.scenarios=data[1].value.filter(s=>typeof s.id==='string'&&typeof s.name==='string');
    if(data[2].status==='fulfilled'&&Array.isArray(data[2].value.courses))state.courses=data[2].value.courses;
    if(data[3].status==='fulfilled')state.metadata=data[3].value.scenarios||{};
    if(data[4].status==='fulfilled')state.recordings=(data[4].value.replays||[]).filter(M.safeRecording);
    if(data[6].status==='fulfilled')state.recordings.push(...(data[6].value.saves||[]).filter(s=>s.over&&!s.invalid).map(s=>({id:'save_'+s.id,save_id:s.id,title:s.label||s.scenario})));
    if(data[5].status==='fulfilled'){
      try{const url=new URL(data[5].value.tutorial_video?.url);if(url.protocol==='http:'&&['127.0.0.1','localhost'].includes(url.hostname)){$('tutorial-video').href=url.href;$('tutorial-video').hidden=false;}}catch{}
    }
    state.selectedScenario ||= operations().find(s=>s.id==='dual_carrier_denial')?.id||operations()[0]?.id;
    state.selectedCourse ||= state.courses.find(c=>!state.training.completed?.includes(c.id))?.id||state.courses[0]?.id;
    const partial=[data[1],data[2]].some(d=>d.status==='rejected')||!state.trainingKnown;
    notice(!state.online?t('裁判服务暂时离线。请检查一键启动服务，然后刷新状态。','Referee offline. Check the one-click launcher, then refresh status.'):partial?t('部分任务、课程或进度未能同步。已保留可用入口，请刷新重试。','Some operations, lessons or progress could not sync. Available entries are preserved; refresh to retry.'):'',partial||!state.online);
    state.loading=false;$('refresh-lobby').disabled=false;render();
  }
  function renderCurrent(){
    const current=state.current,known=state.currentKnown&&state.online;
    $('server-status').dataset.state=state.online?'online':'offline';$('server-status').textContent=state.online?t('本地裁判在线','Local referee online'):t('裁判连接中断','Referee disconnected');
    $('continue-game').disabled=!known||!current;
    for(const id of ['prepare-game','prepare-course'])if($(id))$(id).disabled=!state.online;
    $('current-title').textContent=current?(I.scenario(current.scenario_id)?.title||current.scenario_name):known?t('尚未创建战局','No current battle'):t('战局状态暂不可用','Battle status unavailable');
    $('current-description').textContent=current?t('返回当前指挥席，保留已下达的命令与状态。','Return to your command seat with existing orders and state.'):t('选择一项任务，或从训练学院开始。','Choose an operation or start at the academy.');
    $('current-clock').textContent=current?clock(current.tick):'—';
    $('current-speed').textContent=current&&state.time?(state.time.speed===0?t('已暂停','Paused'):state.time.speed+'×'):'—';
    $('battle-state').textContent=!known?t('未同步','Unsynced'):!current?t('待命','Ready'):current.over?t('已结束','Ended'):state.time?.speed===0?t('已暂停','Paused'):t('进行中','Active');
    const training=state.training.session,course=state.courses.find(c=>c.id===training?.course_id);
    $('current-training').textContent=training&&course?t(`训练中：${text(course.title)} · ${training.step}/${training.step_count}目标`,`Training: ${text(course.title)} · ${training.step}/${training.step_count} objectives`):'';
  }
  function renderOperations(){
    const all=operations(),query=$('scenario-search').value.trim().toLowerCase(),filter=$('scenario-filter').value;
    const list=all.filter(s=>(filter==='all'||group(s)===filter)&&`${title(s)} ${s.name} ${s.id}`.toLowerCase().includes(query));
    if(!list.some(s=>s.id===state.selectedScenario))state.selectedScenario=list[0]?.id||null;
    $('operation-count').textContent=t(`${all.length}项任务`,`${all.length} operations`);const container=$('scenario-list');container.replaceChildren();
    for(const s of list){const m=state.metadata[s.id],b=document.createElement('button');b.type='button';b.className='catalog-item';b.dataset.scenario=s.id;b.setAttribute('aria-pressed',String(s.id===state.selectedScenario));
      b.innerHTML=`<span><strong>${esc(title(s))}</strong><small>${esc(M.duration(m?.duration_s,en()))} · ${esc(modeFor(s))}</small></span>`;
      b.onclick=()=>{state.selectedScenario=s.id;renderOperations();container.querySelector(`[data-scenario="${CSS.escape(s.id)}"]`)?.focus({preventScroll:true});};container.append(b);}
    if(!list.length){container.innerHTML=`<p class="empty-panel">${esc(all.length?t('没有匹配的任务。清空搜索或切换任务类型。','No matching operations. Clear the search or change the filter.'):t('任务目录尚未载入。请刷新状态。','Operation catalogue unavailable. Refresh status.'))}</p>`;}
    const s=all.find(s=>s.id===state.selectedScenario),box=$('scenario-preview');
    if(!s){box.innerHTML=`<p>${esc(t('载入任务目录后可查看简报。','Load the catalogue to view an operation briefing.'))}</p>`;return;}
    const m=state.metadata[s.id];
    const oppSide=state.seat==='Blue'?t('红方','Red'):t('蓝方','Blue');
    box.innerHTML=`<span class="section-label">${esc(t('任务简报','Operation briefing'))}</span><h2>${esc(title(s))}</h2><p>${esc(summary(s))}</p><div class="briefing-facts"><span>${esc(M.duration(m?.duration_s,en()))}</span>${m?`<span>${m.initial_units} ${esc(t('个初始单位','initial units'))}</span><span>${esc(modeFor(s))}</span>${m.satellite?`<span>${esc(t('离线卫星海图','Offline satellite map'))}</span>`:''}`:''}</div><div class="deployment-form"><label>${esc(t('指挥席','Command seat'))}</label><div class="side-choices" role="group" aria-label="${esc(t('选择阵营','Choose side'))}"><button type="button" data-seat="Blue" aria-pressed="${state.seat==='Blue'}">${esc(t('蓝方','Blue'))}</button><button type="button" data-seat="Red" aria-pressed="${state.seat==='Red'}">${esc(t('红方','Red'))}</button></div><label for="scenario-seed">${esc(t('随机种子（可选）','Random seed (optional)'))}</label><div class="seed-row"><input id="scenario-seed" type="text" inputmode="numeric" value="${esc(state.seed)}" placeholder="${esc(t('使用想定默认值','Use scenario default'))}" autocomplete="off"></div><small class="seed-hint">${esc(t('留空采用想定默认种子。所有新局从暂停开始，准备好后再推进时间。','Leave blank for the scenario seed. Every new battle starts paused so you can prepare before advancing time.'))}</small><div class="agent-setup"><label for="opponent-mode">${esc(t('对战模式（PvE / 双人）','Opposition mode (PvE / two-seat)'))}</label><select id="opponent-mode"><option value="human" ${state.opponent==='human'?'selected':''}>${esc(t(`人类${oppSide} · 双人同机`,`Human ${state.seat==='Blue'?'Red':'Blue'} · hotseat`))}</option><option value="tactical" ${state.opponent==='tactical'?'selected':''}>${esc(t('状态机 AI 对手 · PvE（推荐，无需模型）','State-machine AI · PvE (recommended, no model)'))}</option><option value="llm" ${state.opponent==='llm'?'selected':''}>${esc(t('LLM Agent 对手（需配置模型）','LLM agent opponent (needs a model)'))}</option></select><label for="tactical-takeover">${esc(t('本席托管（状态机参谋 · 无需模型）','Own-seat takeover (state-machine staff, no model)'))}</label><select id="tactical-takeover"><option value="off" ${!state.tacticalTakeover?'selected':''}>${esc(t('关闭（亲自指挥）','Off (you command)'))}</option><option value="on" ${state.tacticalTakeover?'selected':''}>${esc(t('开启（进对局后可随时收回）','On (reclaim any time in-game)'))}</option></select><label for="assistant-mode">${esc(t('LLM 助手（需配置模型）','LLM assistant (needs a model)'))}</label><select id="assistant-mode"><option value="off" ${state.assistantMode==='off'?'selected':''}>${esc(t('关闭','Off'))}</option><option value="semi" ${state.assistantMode==='semi'?'selected':''}>${esc(t('半自动：对话与建议','Semi-auto: chat and advice'))}</option><option value="full" ${state.assistantMode==='full'?'selected':''}>${esc(t('全自动：LLM 托管','Full-auto: LLM command'))}</option></select><label for="match-speed">${esc(t('规划窗初始速度','Initial planning-window speed'))}</label><select id="match-speed"><option value="1" ${state.matchSpeed===1?'selected':''}>1×</option><option value="15" ${state.matchSpeed===15?'selected':''}>15×</option><option value="30" ${state.matchSpeed===30?'selected':''}>30×</option><option value="60" ${state.matchSpeed===60?'selected':''}>60×</option></select><small class="agent-note">${esc(t('严格双盲：AI 只读取本方观察。“本席托管”与“状态机 AI 对手”由服务器内建状态机参谋执行、无需模型，进对局后可随时在“状态机托管”面板开关。LLM 对手/助手每 300 模拟秒规划，全自动双方各报速度取较慢值。','Strict double-blind: AI reads only its own observation. Own-seat takeover and the state-machine opponent run on the built-in server staff (no model needed); toggle them any time in the Tactical autopilot panel. LLM opponent/assistant plans every 300 simulated seconds; full-auto uses the slower of both requested speeds.'))}</small></div><button id="prepare-game" class="primary" ${!state.online?'disabled':''}>${esc(t('准备进入任务','Prepare operation'))}</button></div>`;
    box.querySelectorAll('[data-seat]').forEach(b=>b.onclick=()=>{state.seat=b.dataset.seat;renderOperations();});
    $('scenario-seed').oninput=e=>{state.seed=e.target.value;};$('opponent-mode').onchange=e=>{state.opponent=M.opponentValue(e.target.value,state.opponent);store('harpoon.opponent',state.opponent);};$('tactical-takeover').onchange=e=>{state.tacticalTakeover=e.target.value==='on';store('harpoon.tacticalTakeover',state.tacticalTakeover?'1':'0');};$('assistant-mode').onchange=e=>{state.assistantMode=e.target.value;store('harpoon.assistantMode',state.assistantMode);};$('match-speed').onchange=e=>{state.matchSpeed=Number(e.target.value)||15;store('harpoon.matchSpeed',String(state.matchSpeed));};
    $('prepare-game').onclick=()=>{try{const payload=M.gameRequest(s,state.seed);notice('');openLaunch({type:'game',scenario:s,payload,seat:state.seat,opponent:state.opponent,tacticalTakeover:state.tacticalTakeover,assistantMode:state.assistantMode,matchSpeed:state.matchSpeed});}catch(error){notice(error.message==='INVALID_SEED'?t('随机种子请输入0至9007199254740991之间的整数；也可留空。','Enter an integer seed from 0 to 9007199254740991, or leave it blank.'):t('想定文件入口无效，请刷新任务目录后重试。','Invalid scenario entry; refresh the operation catalogue and retry.'),true);$('scenario-seed').focus();}};
  }
  function renderAcademy(){
    const courses=state.courses,completed=state.training.completed||[];$('academy-progress').textContent=state.trainingKnown?t(`${courses.filter(c=>completed.includes(c.id)).length} / ${courses.length}已结业`,`${courses.filter(c=>completed.includes(c.id)).length} / ${courses.length} completed`):t('结业状态未同步','Completion status unsynced');
    const container=$('course-list');container.replaceChildren();
    for(const c of courses){const b=document.createElement('button');b.type='button';b.className='catalog-item';b.dataset.course=c.id;b.setAttribute('aria-pressed',String(c.id===state.selectedCourse));b.innerHTML=`<span class="lesson-number">${String(c.number).padStart(2,'0')}</span><span><strong>${esc(text(c.title))}</strong><small>${esc(t(`约${c.duration}分钟 · ${c.steps.length}个目标`,`About ${c.duration} min · ${c.steps.length} objectives`))}</small></span>${completed.includes(c.id)?`<span class="completed-mark">${esc(t('已结业','Completed'))}</span>`:''}`;
      b.onclick=()=>{state.selectedCourse=c.id;renderAcademy();container.querySelector(`[data-course="${CSS.escape(c.id)}"]`)?.focus({preventScroll:true});};container.append(b);}
    const c=courses.find(c=>c.id===state.selectedCourse),box=$('course-preview');if(!c){container.innerHTML=`<p class="empty-panel">${esc(t('训练目录尚未载入。请刷新状态重试。','Training catalogue unavailable. Refresh status to retry.'))}</p>`;box.replaceChildren();return;}
    const active=state.training.session?.course_id===c.id;
    box.innerHTML=`<span class="section-label">${esc(t('课程预览','Lesson preview'))}</span><h2>${esc(text(c.title))}</h2><p>${esc(text(c.summary))}</p><ol>${c.steps.map(s=>`<li>${esc(text(s.title))}</li>`).join('')}</ol>${active?`<a class="secondary" href="command.html?side=Blue">${esc(t('继续当前课程','Resume current lesson'))}</a> `:''}<button id="prepare-course" class="primary" ${!state.online?'disabled':''}>${esc(completed.includes(c.id)||active?t('重新训练本课','Train again'):t('开始本课','Start lesson'))}</button>`;
    $('prepare-course').onclick=()=>openLaunch({type:'course',course:c,seat:'Blue'});
  }
  function renderReplays(){
    const box=$('recording-list');box.replaceChildren();
    if(!state.recordings.length){box.innerHTML=`<p class="empty-panel">${esc(t('尚未发现本地复盘档案。点击“导入 / 管理复盘”即可导入已有文件。','No local replay recording found. Choose Import / manage replays to open an existing file.'))}</p>`;return;}
    for(const entry of state.recordings){const row=document.createElement('article');row.className='replay-entry';row.innerHTML=`<div><span class="section-label">${esc(t('已归档的对局','Archived battle'))}</span><h2>${esc(text(entry.title))}</h2><p>${esc(t('蓝方 / 红方 / 上帝视角 · 播放与事件定位','Blue / Red / God view · Playback and event navigation'))}</p><small>${esc(t('按原始观察采样回放；精度在复盘页标注。','Replays original observation samples; precision is shown in the viewer.'))}</small></div><a class="primary" href="replay.html?${entry.save_id?'save='+encodeURIComponent(entry.save_id):'replay='+encodeURIComponent(entry.id)}">${esc(t('打开复盘','Open replay'))}</a>`;box.append(row);}
  }
  function render(){renderCurrent();renderOperations();renderAcademy();renderReplays();}
  function localize(){
    for(const el of document.querySelectorAll('[data-zh]'))el.textContent=el.dataset[en()?'en':'zh'].replace(/\\n/g,'\n');
    document.title=t('鱼叉 · 指挥大厅','Harpoon · Command lobby');$('scenario-search').placeholder=t('搜索任务','Search operations');$('settings-language').value=I.language;render();if(state.pending)renderLaunch();renderFullscreen();
  }
  function showPage(focus=false){
    let page=location.hash.slice(1)||'home';if(!['home','operations','academy','replays','settings'].includes(page)){page='home';history.replaceState(null,'','#home');}
    document.querySelectorAll('.lobby-page').forEach(section=>section.hidden=section.id!=='page-'+page);
    document.querySelectorAll('[data-page]').forEach(a=>{if(a.dataset.page===page)a.setAttribute('aria-current','page');else a.removeAttribute('aria-current');});
    if(focus){const heading=$('page-'+page).querySelector('h1');heading.tabIndex=-1;heading.focus({preventScroll:true});window.scrollTo(0,0);}
  }
  function renderLaunch(){
    const pending=state.pending;if(!pending)return;
    $('launch-title').textContent=pending.type==='game'?title(pending.scenario):text(pending.course.title);
    const seed=pending.payload?.seed;
    let ai='';
    if(pending.type==='game'){
      const parts=[];
      if(pending.type==='game'&&pending.opponent==='tactical')parts.push(t('对手：状态机 AI（PvE）','Opponent: state-machine AI (PvE)'));
      if(pending.opponent==='llm')parts.push(t('对手：LLM Agent','Opponent: LLM agent'));
      if(pending.tacticalTakeover)parts.push(t('本席：状态机托管','Own seat: tactical autopilot'));
      if(pending.assistantMode!=='off')parts.push(t(`LLM 助手${pending.assistantMode==='full'?'全自动':'半自动'}`,`LLM assistant ${pending.assistantMode==='full'?'full-auto':'semi-auto'}`));
      if(parts.length)ai=` · ${parts.join(' · ')} · ${pending.matchSpeed}×`;
    }
    const starts=pending.type==='game'&&M.agentMatchStartsAtSpeed(pending);
    const base=pending.type==='game'?t(`${pending.seat==='Blue'?'蓝方':'红方'}指挥席 · ${starts?'AI 对局':'暂停开局'} · ${seed===undefined?'想定默认种子':'种子 '+seed}`,`${pending.seat} command · ${starts?'AI match':'Starts paused'} · ${seed===undefined?'Scenario default seed':'Seed '+seed}`)+ai:t('从本课第一步开始，沿用权威目标核验与检查点。','Start at the first objective with authoritative checks and checkpoints.');
    $('launch-description').textContent=base;
    $('replace-warning').textContent=state.current?t(`当前战局“${I.scenario(state.current.scenario_id)?.title||state.current.scenario_name}”将被替换。当前进行中的进度不会另存；已有结业记录和复盘档案保留。`,`This replaces “${I.scenario(state.current.scenario_id)?.title||state.current.scenario_name}”. Current in-progress state is not saved separately. Completion records and replay archives are retained.`):t('准备完成后进入指挥台，新局保持暂停。','Enter the command console with the new battle paused.');
    $('confirm-launch').textContent=state.busy?t('正在创建…','Creating…'):t('确认开始','Confirm start');
  }
  function openLaunch(pending){if(state.busy)return;state.pending=pending;launchFocus=document.activeElement;$('launch-error').hidden=true;renderLaunch();$('launch-dialog').showModal();$('cancel-launch').focus();}
  function closeLaunch(){if(state.busy)return;$('launch-dialog').close();state.pending=null;launchFocus?.focus();}
  $('cancel-launch').onclick=closeLaunch;
  $('launch-dialog').addEventListener('cancel',e=>{if(state.busy)e.preventDefault();else state.pending=null;});
  $('launch-form').onsubmit=async e=>{
    e.preventDefault();const pending=state.pending;if(state.busy||!pending)return;state.busy=true;$('confirm-launch').disabled=true;$('cancel-launch').disabled=true;$('launch-error').hidden=true;renderLaunch();
    $('launch-form').setAttribute('aria-busy','true');
    try{
      const result=await request(pending.type==='game'?'/api/game':'/api/tutorial/start',pending.type==='game'?pending.payload:{course:pending.course.id});
      if(pending.type==='game'&&!result.started||pending.type==='course'&&result.session?.course_id!==pending.course.id)throw Error('START_NOT_CONFIRMED');
      if(pending.type==='game'&&pending.opponent==='tactical'){
        const cfg=M.tacticalAutopilotConfig(pending);
        if(cfg)await request('/api/tactical/config',cfg);
      }
      if(pending.type==='game'&&pending.tacticalTakeover){
        const cfg=M.tacticalTakeoverConfig(pending);
        if(cfg)await request('/api/tactical/config',cfg);
      }
      if(pending.type==='game'&&(pending.opponent==='llm'||pending.assistantMode!=='off')){
        const configs=M.agentMatchConfigs(pending);
        for(const cfg of configs)await request('/api/agent/config',cfg);
        if(M.agentMatchStartsAtSpeed(pending))await request('/api/time',{speed:Number(pending.matchSpeed)||15});
      }
      store('harpoon.side',pending.seat);if(pending.type==='game')store('harpoon.scenario',pending.scenario.id);
      location.assign('command.html?side='+pending.seat);
    }catch{
      $('launch-error').hidden=false;$('launch-error').textContent=t('未能确认创建结果。请检查连接，返回刷新战局状态后再决定重试。','Creation could not be confirmed. Check the connection, go back and refresh battle status before retrying.');
      await currentData();
    }finally{state.busy=false;$('launch-form').setAttribute('aria-busy','false');$('confirm-launch').disabled=false;$('cancel-launch').disabled=false;renderLaunch();}
  };
  $('continue-game').onclick=()=>{if(state.current&&state.currentKnown&&state.online)location.assign('command.html?side='+(state.training.session||state.current.scenario_id.startsWith('academy_')?'Blue':read('harpoon.side')==='Red'?'Red':'Blue'));};
  $('refresh-lobby').onclick=()=>void load();$('scenario-search').oninput=renderOperations;$('scenario-filter').onchange=renderOperations;
  $('settings-language').onchange=e=>I.setLanguage(e.target.value);
  $('text-size').value=read('harpoon.lobby.text')==='large'?'large':'normal';
  const applySize=()=>{document.body.dataset.text=$('text-size').value;store('harpoon.lobby.text',$('text-size').value);};$('text-size').onchange=applySize;applySize();
  $('reduce-motion').checked=read('harpoon.lobby.motion')?read('harpoon.lobby.motion')==='reduced':matchMedia('(prefers-reduced-motion: reduce)').matches;
  const applyMotion=()=>{document.body.dataset.motion=$('reduce-motion').checked?'reduced':'normal';store('harpoon.lobby.motion',document.body.dataset.motion);};$('reduce-motion').onchange=applyMotion;applyMotion();
  $('fullscreen').onclick=async()=>{try{if(document.fullscreenElement)await document.exitFullscreen();else await document.documentElement.requestFullscreen();}catch{notice(t('当前窗口不支持网页全屏，可使用浏览器的全屏功能。','Page fullscreen is unavailable in this window; use browser fullscreen.'));}};
  function renderFullscreen(){$('fullscreen').textContent=document.fullscreenElement?t('退出全屏','Exit fullscreen'):t('全屏显示大厅','Show lobby fullscreen');$('fullscreen').setAttribute('aria-pressed',String(!!document.fullscreenElement));}
  document.addEventListener('fullscreenchange',renderFullscreen);
  addEventListener('hashchange',()=>showPage(true));addEventListener('harpoon:language',localize);
  $('main-navigation').addEventListener('keydown',e=>{if(!['ArrowDown','ArrowUp','ArrowRight','ArrowLeft','Home','End'].includes(e.key))return;const links=[...$('main-navigation').querySelectorAll('a')],index=links.indexOf(document.activeElement);if(index<0)return;e.preventDefault();const next=e.key==='Home'?0:e.key==='End'?links.length-1:(index+(['ArrowDown','ArrowRight'].includes(e.key)?1:-1)+links.length)%links.length;links[next].focus();});
  document.addEventListener('keydown',e=>{if(e.key==='Escape'&&!$('launch-dialog').open&&!/INPUT|SELECT|TEXTAREA/.test(e.target.tagName))location.hash='home';});
  document.addEventListener('visibilitychange',()=>{if(!document.hidden&&!state.busy)void currentData();});
  localize();showPage();void load();
  setInterval(()=>{if(!document.hidden&&!state.busy&&!state.loading)void currentData();},10000);
})();
