/* Presentation names only. IDs, observations, weapon performance and saves stay exact. */
(function(root){
  'use strict';
  const platforms={
    cn_001:'辽宁号航空母舰',cn_002:'山东号航空母舰',cn_003:'福建号航空母舰',cn_075:'075型两栖攻击舰',
    cn_055:'055型驱逐舰',cn_052d:'052D型驱逐舰',cn_052c:'052C型驱逐舰',cn_054a:'054A型护卫舰',cn_056a:'056A型护卫舰',
    cn_093:'093型攻击核潜艇',cn_093a:'093A型攻击核潜艇',cn_039a:'039A型常规潜艇',
    cn_j15:'歼-15舰载战斗机',cn_j15d:'歼-15D舰载电子战机',cn_z18f:'直-18F反潜直升机',cn_z18j:'直-18J预警直升机',
    cn_h6j_proxy:'轰-6J远程轰炸机（游戏代理）',cn_kj500:'空警-500预警机',cn_kj600:'空警-600舰载固定翼预警机',
    us_ford:'福特级核动力航母',us_nimitz:'尼米兹级核动力航母',us_reagan:'里根号航空母舰',
    us_burke:'阿利·伯克级驱逐舰',us_burke_1:'阿利·伯克级驱逐舰（I/II型）',us_burke_2a:'阿利·伯克级驱逐舰（IIA型）',us_burke_3:'阿利·伯克级驱逐舰（III型）',us_ticonderoga:'提康德罗加级巡洋舰',us_ticonderoga_cmp:'提康德罗加级巡洋舰（CMP改装）',us_virginia:'弗吉尼亚级攻击核潜艇',us_seawolf:'海狼级攻击核潜艇',
    us_america_lha:'美国级两栖攻击舰',us_wasp_lhd:'黄蜂级两栖攻击舰',
    us_f18e:'F/A-18E/F“超级大黄蜂”战斗机',us_f18e_b:'F/A-18E/F“超级大黄蜂”Block III（AIM-174B构型）',us_f18e_asm:'F/A-18E/F“超级大黄蜂”（反舰挂载）',us_f18e_lracm:'F/A-18E/F“超级大黄蜂”（LRASM打击挂载）',us_f35c:'F-35C舰载战斗机',
    us_e2d:'E-2D“先进鹰眼”预警机',us_mh60r:'MH-60R“海鹰”反潜直升机',
  };
  const weapons={
    cn_yj12:'鹰击-12舰射反舰导弹（高空巡航）',cn_yj12_surface_skim:'鹰击-12舰射反舰导弹（掠海巡航）',
    cn_yj12_air:'鹰击-12空射反舰导弹（高空巡航）',cn_yj12_air_skim:'鹰击-12空射反舰导弹（掠海巡航）',
    cn_yj18:'鹰击-18反舰导弹',cn_yj62:'鹰击-62反舰导弹',cn_yj82:'鹰击-82反舰导弹',cn_yj83:'鹰击-83反舰导弹',cn_yj83k:'鹰击-83K空射反舰导弹',
    cn_yj91:'鹰击-91反辐射导弹',cn_yj9:'鹰击-9导弹',cn_dh10:'东海-10巡航导弹',
    cn_hq9:'海红旗-9远程舰空导弹',cn_hq9b:'海红旗-9B远程舰空导弹',cn_hq16:'海红旗-16舰空导弹',cn_hq10:'海红旗-10近程舰空导弹',
    cn_pl12:'霹雳-12中距空空导弹',cn_pl15:'霹雳-15远程空空导弹',cn_pl8:'霹雳-8近距空空导弹',cn_pl10:'霹雳-10近距空空导弹',
    cn_yu6:'鱼-6重型鱼雷',cn_yu9:'鱼-9重型鱼雷',cn_yu10:'鱼-10重型鱼雷',
    us_harpoon:'RGM-84“鱼叉”舰射反舰导弹',us_agm84:'AGM-84“鱼叉”空射反舰导弹',us_agm158c:'AGM-158C“远程反舰导弹”LRASM',us_aim174b:'AIM-174B“标准-6空射型”超远程空空导弹',us_nsm:'NSM海军打击导弹',us_tomahawk:'“战斧”巡航导弹',
    us_sm2:'“标准”-2舰空导弹',us_sm6:'“标准”-6舰空导弹',us_essm:'ESSM改进型“海麻雀”舰空导弹',us_ram:'“拉姆”近程防空导弹',
    us_aim120:'AIM-120先进中距空空导弹',us_aim9x:'AIM-9X“响尾蛇”空空导弹',us_mk48:'Mk48重型鱼雷',us_mk54:'Mk54轻型反潜鱼雷',
    ciws_30mm:'30毫米近防炮',gun_30mm:'30毫米舰炮',gun_76mm:'76毫米舰炮',gun_5in:'127毫米舰炮',
  };
  const aliases={'CV-16 LIAONING':'CV-16 辽宁号','CV-17 SHANDONG':'CV-17 山东号','CVN-78 FORD':'CVN-78 福特号','CVN-68 NIMITZ':'CVN-68 尼米兹号',
    'FORD':'福特号','NIMITZ':'尼米兹号','LIAONING':'辽宁号','SHANDONG':'山东号','SHILOH':'希洛号','KIDD':'基德号',
    'YJ-18 SUPERSONIC-TERMINAL ASM':'鹰击-18反舰导弹','PL-12 ACTIVE-RADAR AAM':'霹雳-12中距空空导弹','YU-6 HEAVY TORPEDO':'鱼-6重型鱼雷',
    'DH-10 LAND-ATTACK CRUISE MISSILE':'东海-10巡航导弹','HARPOON IC':'“鱼叉”反舰导弹（IC型）','AGM-84D HARPOON IC':'AGM-84D“鱼叉”空射反舰导弹',
    'MK48 MOD 7 CBASS':'Mk48 Mod7重型鱼雷','AIM-120 AMRAAM':'AIM-120先进中距空空导弹'};
  const dictionary=new Map();
  function language(lang){return lang||root.HarpoonI18n?.language||'zh-CN';}
  function text(original,lang){
    const source=String(original??'');if(language(lang)!=='zh-CN')return source;
    if(dictionary.has(source))return dictionary.get(source);
    if(aliases[source.toUpperCase()])return aliases[source.toUpperCase()];
    let authored=source;
    if(/^(BLUE|RED)\b/.test(source)) {
      for(const [a,b]of [['AIR STATION','航空站'],['ZHEJIANG SECTOR','浙向'],['SHANGHAI SECTOR','沪向'],['MOORED','泊港'],['FICTIONAL','虚构'],['ESCORT','护航'],['OUTER','外线'],['WEST','西部'],['EAST','东部'],['NORTH','北部'],['SOUTH','南部'],['COAST','沿岸'],['BOMBER','轰炸航空'],['SUPPORT','支援'],['PORT','港口'],['CRUISE BATTERY','巡航导弹分队']])authored=authored.replace(new RegExp('\\b'+a+'\\b','g'),b);
    }
    // Translate authored callsign patterns only; never guess a custom ship name.
    return (root.HarpoonI18n?.text(authored)||authored).replace(/^TYPE[- ](\d+[A-Z]?)(?=\s|$)/i,(_,type)=>`${type.toUpperCase()}型`)
      .replace(/\bBLUE\b/g,'蓝方').replace(/\bRED\b/g,'红方')
      .replace(/\bAEW[- ]?(\d+)\b/g,'预警机$1').replace(/\bSSN[- ]?(\d+)\b/g,'攻击核潜艇$1')
      .replace(/\bCAP[- ]?(\d+)\b/g,'战斗空巡$1').replace(/\bASW[- ]?(\d+)\b/g,'反潜机$1')
      .replace(/\bDDG[- ]?([A-Z0-9]+)\b/g,'驱逐舰$1').replace(/\bFFG[- ]?([A-Z0-9]+)\b/g,'护卫舰$1');
  }
  function name(kind,id,original,lang){return language(lang)==='zh-CN'?(kind==='weapon'?weapons:platforms)[id]||text(original,lang):String(original??id??'');}
  function unit(u,lang){return text(u?.name||`#${u?.id}`,lang);}
  function register(db){
    dictionary.clear();
    for(const [id,value]of Object.entries(platforms)){if(db?.platforms?.[id]?.name)dictionary.set(db.platforms[id].name,value);}
    for(const [id,value]of Object.entries(weapons)){if(db?.weapons?.[id]?.name)dictionary.set(db.weapons[id].name,value);}
  }
  root.HarpoonNames={name,unit,text,register,platforms,weapons};
  if(typeof module!=='undefined')module.exports=root.HarpoonNames;
})(typeof window!=='undefined'?window:globalThis);
