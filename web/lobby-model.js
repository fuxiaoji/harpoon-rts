(function(root){
  'use strict';
  function seedValue(text){
    if(!String(text).trim())return undefined;
    if(!/^\d+$/.test(String(text).trim()))throw Error('INVALID_SEED');
    const seed=Number(text);if(!Number.isSafeInteger(seed)||seed<0)throw Error('INVALID_SEED');
    return seed;
  }
  function gameRequest(scenario,text=''){
    if(!scenario||typeof scenario.file!=='string'||!/^[a-zA-Z0-9_-]+\.json$/.test(scenario.file))throw Error('INVALID_SCENARIO');
    const payload={scenario:scenario.file.slice(0,-5),speed:0},seed=seedValue(text);
    if(seed!==undefined)payload.seed=seed;
    return payload;
  }
  function safeRecording(entry){return entry&&/^[a-zA-Z0-9_.-]+$/.test(entry.file)&&/^[a-zA-Z0-9_-]+$/.test(entry.id);}
  function duration(seconds,english=false){
    if(!Number.isFinite(seconds))return english?'Duration unspecified':'时限未标注';
    return seconds%3600===0 ? `${seconds/3600}${english?' simulated hours':'模拟小时'}` : `${Math.round(seconds/60)}${english?' simulated minutes':'模拟分钟'}`;
  }
  function agentMatchConfigs(options={}){
    const raw=Number(options.matchSpeed),speed=Number.isFinite(raw)?Math.min(60,Math.max(5,Math.round(raw))):15;
    const mode=options.assistantMode==='full'?'full':options.assistantMode==='semi'?'semi':'off';
    const out=[];
    if(options.agentOpponent)out.push({side:'Red',enabled:true,autonomy:'execute',auto_run:mode==='full',auto_speed:speed});
    if(mode==='semi')out.push({side:'Blue',enabled:true,autonomy:'advise',auto_run:false,auto_speed:speed});
    if(mode==='full')out.push({side:'Blue',enabled:true,autonomy:'execute',auto_run:true,auto_speed:speed});
    return out;
  }
  function agentMatchStartsAtSpeed(options={}){
    return options.assistantMode==='full' || !!options.agentOpponent;
  }
  const api={seedValue,gameRequest,safeRecording,duration,agentMatchConfigs,agentMatchStartsAtSpeed};
  if(typeof module!=='undefined')module.exports=api;root.HarpoonLobbyModel=api;
})(typeof window!=='undefined'?window:globalThis);
