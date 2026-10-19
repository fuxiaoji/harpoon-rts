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
  const api={seedValue,gameRequest,safeRecording,duration};
  if(typeof module!=='undefined')module.exports=api;root.HarpoonLobbyModel=api;
})(typeof window!=='undefined'?window:globalThis);
