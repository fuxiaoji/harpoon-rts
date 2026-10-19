/* Shared bilingual catalogue. Protocol values, IDs and simulation data stay unchanged. */
(function (root) {
  'use strict';
  const catalogue = Object.fromEntries(`
FLEET COMMAND // TACTICAL|舰队战术指挥
SIDE|阵营
BLUE|蓝方
RED|红方
Blue|蓝方
Red|红方
SCENARIO|想定
TRAINING|训练
TUTORIAL|训练中心
HOW TO PLAY|怎么玩
BATTLE REPLAY|战局复盘
RETURN TO LOBBY|返回大厅
Return to lobby|返回大厅
Pause and explain firing, detection and automatic actions|暂停并查看开火、探测与自主行动说明
AUTO-ENGAGE TARGET|指定目标自动交战
AUTO-ENGAGING TARGET|正在指定目标自动交战
CANCEL ASSIGNMENT|取消指派
FIRE 1 ROUND|人工射击1发
AUTO TARGET & FIRE|自动选敌开火
AUTO SELECT ON|自动选敌：开
AUTO SELECT OFF|自动选敌：关
One order for one round; wait for cooldown. A queued order is not a confirmed shot.|单次命令，每次1发；请等冷却。命令排队不等于成功发射。
Standing order: automatically engage this target, even when automatic target selection is off.|持续命令：自动攻击此目标；关闭自动选敌仍会攻击指派目标。
Toggle automatic target selection. Assigned targets are engaged independently.|切换自动选敌；指派目标仍会独立持续交战。
TIME CONTROL|时间控制
TICK|模拟秒
ACTIVE SCENARIO|当前任务
OWN UNIT|己方单位
NO UNIT SELECTED|尚未选择单位
Click a friendly unit on the map.|点击海图上的己方单位。
Select a friendly unit on the map.|在海图上选择己方单位。
TARGET DESIGNATION|目标指派
NO CONTACT SELECTED|尚未选择接触
Select a contact on the map.|在海图上选择接触。
ORDERS|指挥命令
WEAPONS|武器
SENSORS|传感器
MISSION|任务
FLIGHT DECK|飞行甲板
DAMAGE CONTROL|损伤与系统
EVENT FEED|作战记录
NO TRAFFIC|暂无战报
Detection and combat reports will appear here.|探测和交战报告将在这里出现。
FIT|全图
SCALE|比例尺
CONTACT|接触
UNIT|单位
ALL|公开
ONLINE|在线
ACTIVE|启用
HOLD|限制
FREE|自由
STANDBY|待机
LISTENING|监听
SURFACE|水面
SUBMERGED|潜航
AIRBORNE|飞行中
ASHORE|陆上
FIXED SITE|固定设施
STATUS|状态
DEPTH|深度
ALTITUDE|高度
COURSE|航向
SPEED|航速
HULL|舰体
ORD|指令
SELECT|选择
INPUT|输入
NO TRACK|没有航迹
BEARING ONLY|仅方位
BEARING|方位
TRACK|航迹
STALE|陈旧
UNKNOWN|未知
Unknown|未知
Surface|水面
Air|空中
Subsurface|水下
Missile|导弹
Torpedo|鱼雷
Land|陆上
Ship|舰艇
Aircraft|航空器
Sub|潜艇
Base|基地
Weapon|武器
Sensor|传感器
UNNAMED|未命名
WEAPONS FREE|自由开火
WEAPONS HOLD|限制开火
WEAPONS STATE|武器状态
DEPLOY CHAFF|释放箔条
CHAFF|箔条
STOP|停止
ALL STOP|停止航行
RECOVER AIRCRAFT|召回舰载机
ASSIGN TARGET|指派目标
TARGET ASSIGNED|已指派目标
CLEAR TARGET|取消目标指派
NO TARGET ASSIGNED|未指派目标
DESIGNATED TARGET|已指派目标
FIRE|开火
FIRE CTRL|火控
MOBILITY|机动
RADAR|雷达
SONAR|声纳
AMMO|弹药
COOLDOWN|冷却
RANGE|射程
LAST SEEN|最后更新
QUALITY|航迹质量
NO WEAPONS|没有武器
NO VALID MOUNT|没有适用武器座
NO WEAPON MOUNTS|没有武器座
NO SENSOR SETS|没有传感器
NO FLIGHT DECK|没有飞行甲板
NO TASKING|未分配任务
READY|就绪
RDY|就绪
EMPTY|空
NO FIT|不适用
ASW SWEEP|反潜搜索
CAP|战斗空中巡逻
RECON|侦察
STRIKE|突击
RTB|返航
LAUNCH|出动
JUST NOW|刚刚
OVER|已结束
WIN|获胜
VICTORY|胜利
DRAW|平局
DRAW / CEASEFIRE|平局 / 停火
FINAL SCORE UNAVAILABLE|最终得分暂不可用
COMMAND OBJECTIVE MET|指挥目标达成
DAY|白昼
NIGHT|夜间
No game is running on the server.|服务端尚未开始对局。
Request timed out.|请求超时，请重试。
Network error.|网络连接异常。
No scenarios|没有可用想定
No scenario|没有想定
Scenario feed unavailable|想定列表暂不可用
No scenarios are available in the server scenario directory.|服务端目录中没有可用想定。
No game running — choose a scenario|尚未开局，请选择想定
SELECT A SCENARIO TO DEPLOY|选择想定以开始部署
Awaiting deployment orders.|等待部署命令。
Double-blind tactical picture.|双盲战术态势。
AWAITING TACTICAL PICTURE|等待战术态势
AWAITING FEED|等待数据
Start selected scenario|开始所选想定
Restart selected scenario|重新开始所选想定
That scenario is not present on the server.|服务端没有这个想定。
No tactical feed is available yet.|战术态势暂未就绪。
The scenario is over; movement orders are locked.|想定已结束，不能再下达机动命令。
Select a friendly unit before plotting a course.|先选择己方单位，再规划航路。
Waypoint order rejected by the referee.|裁判拒绝了航路命令。
Select a friendly unit before assigning or firing.|先选择己方单位，再指派目标或开火。
Select a friendly unit, then a contact.|先选择己方单位，再选择接触。
Select a contact|选择接触
Select a unit|选择单位
Left-click a contact to build a firing solution.|左键选择接触，查看射击条件。
No weapon is compatible with this contact category.|没有适用于此类接触的武器。
Selected mount is empty.|所选武器座的弹药已耗尽。
Scenario is over. Weapons are locked.|想定已结束，武器已锁定。
Simulation paused; the order will wait for the next tick.|模拟已暂停；命令将在下一模拟秒执行。
SIMULATION PAUSED|模拟已暂停
ORDER|命令
QUEUED|已排队
LINKING|连接中
LINK LOST|连接中断
DOWN|离线
WS LIVE|实时连接
POLL 2HZ|轮询连接
NO GAME|未开局
Connecting to the WebSocket feed…|正在连接实时态势…
WebSocket feed connected|实时态势已连接
WebSocket lost — polling /api/state every 500 ms|实时连接断开，已切换轮询
WebSocket unavailable — polling /api/state every 500 ms|实时连接不可用，已切换轮询
Referee clock is running. Double-blind contact picture initialised.|裁判已建立对局，接触态势遵循双盲规则。
Track decayed past the stale threshold.|航迹已失去足够更新，接触信息过时。
Target removed from the contact picture.|目标已从接触态势中移除。
Localised obscuration active.|局部箔条防御已启用。
DECOY FOOLED SEEKER|诱饵成功干扰导引头
Incoming weapon seduced by countermeasures.|来袭武器被反制措施诱偏。
Terminal engagement|末段交战
Torpedo wake acquired by own sensors.|己方传感器发现鱼雷威胁。
Hull destroyed. Unit removed from the picture.|舰体被摧毁，单位已从态势移除。
Airframe lost after fuel or damage failure.|航空器因燃油耗尽或损伤而损失。
Flight operations already in progress.|甲板正在执行其他航空作业。
Facility removed from the order of battle.|设施已被摧毁。
Air tasking assigned by the flight deck.|航空任务由甲板指挥系统分配。
No air mission is currently assigned.|当前没有航空任务。
Scenario complete|想定结束
Scenario complete.|想定已结束。
hostile air defences|敌方防空系统
the enemy|敌方
unknown class|未知型号
SCENARIO STARTED|想定开始
SCENARIO DEPLOYED|想定已部署
DETECTION|发现接触
CONTACT IDENTIFIED|接触已识别
CONTACT LOST|接触丢失
CONTACT SUNK|确认目标沉没
COUNTER-DETECTED|己方被反探测
WEAPONS RELEASE|武器释放
INCOMING|来袭威胁
SAM LAUNCH|防空导弹发射
INTERCEPT|拦截
SUCCESS|成功
FAILED|失败
CHAFF DEPLOYED|箔条已释放
GUN ACTION|舰炮射击
TORPEDO DETECTED|发现鱼雷
IMPACT|命中
CRITICAL HIT|致命损伤
UNIT LOST|单位损失
AIRCRAFT DITCHED|航空器迫降损失
AIRCRAFT DOWN|航空器被击落
AIR LAUNCH|航空器起飞
AIR RECOVERED|航空器回收
DECK BUSY|甲板忙碌
BASE DAMAGED|基地受损
BASE DESTROYED|基地被摧毁
SCENARIO ENDED|想定结束
RULES EVENT|规则结算
Fleet Carrier|舰队航空母舰
Aegis Destroyer|宙斯盾驱逐舰
Frigate|护卫舰
Missile Boat|导弹艇
Patrol Boat|巡逻艇
Diesel Submarine|柴电潜艇
Multifighter|多用途战斗机
Strike Fighter|攻击战斗机
Strike Bomber|攻击轰炸机
AEW Aircraft|预警机
ASW Helicopter|反潜直升机
Tutorial 01 — First Watch|教学 01：初次值更
Operation Crimson Tide|赤潮行动
Operation Pacific Storm|太平洋风暴行动
Gunnery Dual (server smoke)|舰炮对决
Training Academy|舰队训练中心
MANOEUVRE|机动与传感器
Apply|下达命令
Active sonar|主动声纳
On|开启
Off|关闭
Go to point|驶向坐标
X (NM)|东向 X（海里）
Y (NM)|北向 Y（海里）
Language|语言
Simulation controls|模拟控制
Friendly units|己方单位
Map controls|海图控制
Event feed|作战记录
Scenario selection|想定选择
Select commanding side|选择指挥阵营
Start the guided tutorial|打开训练中心
Zoom in|放大
Zoom out|缩小
Fit all units|显示全部单位
Fit tactical picture|适配战术态势
Training|训练
Free play|自由指挥
Pause|暂停
Resume|继续
Next|下一步
Retry|重试
Close|关闭
Cancel|取消
Loading…|正在载入…
SYMBOLOGY|图例
FRIENDLY|友方
AIR|空中
SUB|水下
MISSILE|导弹
TORPEDO|鱼雷
STALE / DASHED|陈旧 / 虚线
CEASEFIRE|停火
CLEAR|取消
WEAPON|武器
COMMAND|指挥命令
SET 0 KT|下令0节
RECOVER AIR|召回航空器
SYSTEM STATUS|系统状态
ESTIMATE QUALITY|估计质量
Orders not assigned.|尚未分配命令。
TRAINING COMMAND|训练指挥单位
TRAINING OPPONENT|训练对手
LMB SELECT · OPEN WATER = GOTO · DRAG PAN · WHEEL ZOOM|左键选择 · 点击空海区设航点 · 拖拽平移 · 滚轮缩放
Left-click a friendly unit on the map to take command.|左键选择海图上的己方单位，接管指挥。
Left-click a contact to inspect the estimate and assign a weapon.|左键选择接触，查看估计信息与武器选项。
Select a friendly unit and a contact to open the firing solution.|先选择己方单位和接触，再查看射击条件。
Tactical map|战术海图
Map tools|海图工具
Map scale|海图比例尺
Fit picture to map|适应全图
Scenario|想定
Simulation speed|模拟倍速
Pause simulation|暂停模拟
Selected unit and target details|所选单位和目标详情
Map legend|海图图例
Operational map. Select a unit or contact; click open water to append a waypoint.|战术海图。选择单位或接触；点击空白海域追加航点。
Long-Range SAM|远程防空导弹
Point-Defense SAM|点防御导弹
5in Gun|5英寸舰炮
Land-Attack Cruise Missile|对陆巡航导弹
Surface-Search Radar|对海搜索雷达
Air-Search Radar|对空搜索雷达
ESM|电子支援
Visual|目视
ASW|反潜
DECK CLEAR · NO READY AIRCRAFT|甲板空闲，没有就绪航空器
MIN|最小
MAX|最大
GUIDANCE|制导
COOLING|冷却中
IN RANGE|射程内
OUT OF RANGE|射程外
ROUTE|航路
WAYPOINT|航点
DAYLIGHT|白昼
NO CONTACT|暂无接触
Adjudication trace|裁决详情
Tdl Share|战术数据链共享
FIRECTL|火控
VIS|目视
JAM|干扰
CD|冷却
ESM Intercept|电子支援侦收
X-band Noise Jammer|X波段噪声干扰机
Long-Range Air Radar|远程对空雷达
Medium Air Radar|中程对空雷达
AEW Radar|预警雷达
Navigation Radar|导航雷达
Medium Surface Radar|中程对海雷达
Surface Search Radar|对海搜索雷达
Active Sonar|主动声纳
Dipping Sonar|吊放声纳
Passive Sonobuoy|被动声纳浮标
Passive Sonar|被动声纳
Towed Passive Array|被动拖曳阵列
Visual Lookouts|目视瞭望
IR AAM|红外制导空空导弹
Anti-Ship Cruise Missile|反舰巡航导弹
Air-Launched ASM|空射反舰导弹
ASW Rocket|反潜火箭
Mk 82 500 lb Bomb|Mk 82型500磅炸弹
CIWS|近防武器系统
100mm Gun|100毫米舰炮
130mm Gun|130毫米舰炮
30mm Gun|30毫米舰炮
76mm Gun|76毫米舰炮
Lightweight ASW Torpedo|轻型反潜鱼雷
Supersonic Anti-Ship Missile|超声速反舰导弹
CV-16 LIAONING|CV-16 辽宁号
CV-17 SHANDONG|CV-17 山东号
CVN-78 FORD|CVN-78 福特号
CVN-68 NIMITZ|CVN-68 尼米兹号
LIAONING|辽宁编队
SHANDONG|山东编队
FORD|福特编队
NIMITZ|尼米兹编队
Ex-Russian Project 1143.5/Type 001 [Kuznetsov] CV|001型航母
Type 002 [Kuznetsov Mod] CV|002型航母
Type 055 [Renhai] CG|055型驱逐舰
Type 052D [Luyang III] DDG|052D型驱逐舰
Type 054A [Jiangkai II] FFG|054A型护卫舰
Type 093A/09IIIA [Shang] SSN|093A型攻击核潜艇
J-15 Feisha Fighter (ceil 17000 m)|歼-15舰载战斗机（升限17000米）
Z-18F Haiying ASW Helicopter (ceil 3150 m)|直-18F反潜直升机（升限3150米）
Z-18J Haibianfu AEW Helicopter (ceil 3150 m)|直-18J预警直升机（升限3150米）
Gerald R. Ford CVN|福特级核动力航母
Nimitz CVN|尼米兹级核动力航母
F/A-18E/F Super Hornet (ceiling 15240 m)|F/A-18E/F超级大黄蜂（升限15240米）
F/A-18E/F Super Hornet (anti-ship fit)|F/A-18E/F超级大黄蜂（反舰挂载）
30× simulation speed|30倍模拟速度
57 mm Unguided Rocket|57毫米无制导火箭
Medium-Range SAM|中程防空导弹
Active-Homing Torpedo|主动声自导鱼雷
Seeker Gen 1 Torpedo|第一代自导鱼雷
Seeker Gen 2 Torpedo|第二代自导鱼雷
Seeker Gen 3 Torpedo|第三代自导鱼雷
Wake-Homing Torpedo|尾流自导鱼雷
AI 参谋 / AGENT|AI 参谋 / AGENT
ENABLE|启用
DISABLE|停用
ALLOW ORDERS|允许下令
REPORT NOW|立即汇报
EXECUTE PROPOSALS|执行建议
MEMORY|记忆
SEND|发送
Message the AI staff|给AI参谋发消息
No memory yet|尚无记忆
No staff reports yet. Enable the staff or request a report.|尚无参谋汇报。启用参谋或点击“立即汇报”。
ERROR|错误
NO KEY|未配置密钥
WORKING|分析中
NEXT REPORT|下次汇报
REJECTED|已拒绝
EXECUTED|已执行
SCHEDULED|定时
MANUAL|手动
Staff report|参谋汇报
Assistant|参谋
You|你
Six-hour kill-count exercise|六小时击杀数演习
At 06:00:00 the side that destroyed more enemy platforms wins.|06:00:00 时击毁敌方平台更多的一方获胜。
`.trim().split('\n').map(line => line.split('|')));
  const escape = s => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  let phrasePattern;
  function rebuild() {
    const words = Object.keys(catalogue).sort((a, b) => b.length - a.length);
    phrasePattern = new RegExp('(?<![A-Za-z])(' + words.map(escape).join('|') + ')(?![A-Za-z])', 'g');
  }
  rebuild();
  function translateText(value, language) {
    const s = String(value ?? '');
    if (language !== 'zh-CN' || !/[A-Za-z]/.test(s)) return s;
    const trimmed = s.trim();
    if (catalogue[trimmed]) return s.replace(trimmed, catalogue[trimmed]);
    const patterns = [
      [/^(\d+) IN PICTURE$/, '$1 个己方单位'],
      [/^Deploy one chaff bundle \((\d+) remaining\)$/, '释放一组箔条（剩余 $1 组）'],
      [/^([\d.]+)× simulation speed$/, '$1× 模拟倍速'],
      [/^NEWEST FIRST · (\d+) EVENTS?$/, '最新在前 · $1 条记录'],
      [/^(\d+) MOUNTS?$/, '$1 个武器座'], [/^(\d+) SETS?$/, '$1 组传感器'],
      [/^(\d+)s AGO$/, '$1 秒前'], [/^(\d+)m (\d+)s AGO$/, '$1 分 $2 秒前'], [/^(\d+)h AGO$/, '$1 小时前'],
      [/^(\d+) hits? on (.*)$/, '命中 $1 次，目标 $2'],
      [/^Impact estimate in (.*)\.$/, '预计抵达时间：$1。'],
      [/^Classified as (.*)$/, '识别为 $1'], [/^Engagement against (.*)$/, '拦截目标 $1'],
      [/^Emissions exposed this unit to (.*)\.$/, '本舰辐射信号使其被 $1 发现。'],
      [/^Shot down by (.*)\.$/, '被 $1 击落。'], [/^Recovered aboard (.*)\.$/, '已回收至 $1。'],
      [/^Track (.*) left the threat axis\.$/, '接触 $1 已偏离威胁方向。'],
      [/^Unable to reach the referee: (.*)$/, '无法连接裁判服务：$1'],
      [/^Unable to start scenario: (.*)$/, '无法开始想定：$1'],
      [/^Order rejected: (.*)$/, '命令被拒绝：$1'],
      [/^(.*) degraded or offline\.$/, '$1 已降级或失效。'],
      [/^(.*) · ([\d.]+) damage · ([\d.]+) HP remaining$/, '$1 · 造成 $2 损伤 · 剩余 $3 DP'],
      [/^([\d.]+) damage · ([\d.]+) HP remaining\.$/, '造成 $1 损伤 · 剩余 $2 DP。'],
    ];
    let out = trimmed;
    for (const [pattern, replacement] of patterns) if (pattern.test(out)) { out = out.replace(pattern, replacement); break; }
    out = out.replace(phrasePattern, key => catalogue[key]);
    out = out.replace(/\bNM\b/g, '海里').replace(/\bKT\b/g, '节').replace(/\bHP\b/g, 'DP');
    return s.replace(trimmed, out);
  }
  if (typeof module !== 'undefined' && module.exports) module.exports = { catalogue, translateText };
  if (!root.document) return;
  let language = 'zh-CN';
  try { language = localStorage.getItem('harpoon.language') || 'zh-CN'; } catch { /* optional */ }
  const original = new WeakMap();
  const originalAttributes = new WeakMap();
  const scenarios = {
    dual_carrier_hotseat: { title:{zh:'双盾行动：六小时双边手操',en:'Twin Shields: six-hour hotseat'},summary:{zh:'双方自动参谋关闭，四艘航母起始停航，由玩家切换指挥。六小时虚构演习，胜负条件与区域拒止版相同。',en:'Both automated staffs off. Four carriers hold until commanded. A six-hour fictional hotseat exercise with the same denial conditions.'} },
    dual_carrier_kills: { title:{zh:'双盾行动：六小时击杀数',en:'Twin Shields: six-hour kill count'},summary:{zh:'辽宁、山东对福特、尼米兹的六小时虚构演习。取消区域拒止，06:00:00 时击毁敌方平台更多的一方获胜；可由双方游戏内 AI 参谋完整对局。',en:'A six-hour fictional exercise: Liaoning and Shandong against Ford and Nimitz. Area denial is removed; at 06:00:00 the side with more destroyed enemy platforms wins. Two in-game AI staff officers can play it end to end.'} },
    dual_carrier_denial: { title:{zh:'双盾行动：双航母区域拒止',en:'Twin Shields: dual-carrier area denial'},summary:{zh:'辽宁、山东守卫45海里任务区；福特、尼米兹尝试连续占据3分钟。两小时虚构演习，使用RTS改编与已声明的数据库估算。',en:'Liaoning and Shandong deny a 45-nm circle against Ford and Nimitz. A two-hour fictional exercise with RTS adaptations and declared database estimates.'} },
    gunnery_dual: { title: {zh:'舰炮对决',en:'Gunnery duel'}, summary: {zh:'两方水面舰艇逐渐接近，练习接触判断、舰炮与导弹交战。',en:'Surface combatants close for a gun and missile engagement. Practise contact assessment and weapon control.'} },
    crimson_tide: { title: {zh:'赤潮行动',en:'Operation Crimson Tide'}, summary: {zh:'指挥航母编队应对水面、潜艇与航空威胁，并开展对陆打击。',en:'Command a carrier group against surface, submarine and air threats while conducting land strikes.'} },
    pacific_storm: { title: {zh:'太平洋风暴行动',en:'Operation Pacific Storm'}, summary: {zh:'八小时联合战役：协调舰队、航空与反潜力量，面对敌方基地和多方向威胁。',en:'An eight-hour joint campaign: coordinate fleet, air and ASW forces against hostile bases and multiple threat axes.'} },
    tattletail_1991: { title: {zh:'Tattletail：1991年北大西洋',en:'Tattletail: North Atlantic, 1991'}, summary: {zh:'北约编队与苏联驱逐舰的近距离危机。本实现使用已声明的替代舰型与RTS改编。',en:'A close encounter between a NATO formation and a Soviet destroyer. This implementation uses declared substitute classes and RTS adaptations.'} },
    tutorial_01: { title: {zh:'旧版引导：初次值更',en:'Legacy tutorial: First watch'}, summary: {zh:'旧版单想定练习。详细分课教学请打开训练中心。',en:'The original single-scenario exercise. Open the academy for detailed, objective-based lessons.'} },
  };
  let observer;
  function localize(scope = document.body) {
    if (!scope) return;
    observer?.disconnect();
    const nodes = [];
    if (scope.nodeType === 3) nodes.push(scope);
    else {
      const walker = document.createTreeWalker(scope, NodeFilter.SHOW_TEXT);
      while (walker.nextNode()) nodes.push(walker.currentNode);
    }
    for (const node of nodes) {
      if (node.parentElement?.closest('script,style,textarea,[data-i18n-native]')) continue;
      let item = original.get(node);
      if (!item || node.nodeValue !== item.rendered) item = { source: node.nodeValue };
      const rendered = translateText(item.source, language);
      if (node.nodeValue !== rendered) node.nodeValue = rendered;
      original.set(node, { source: item.source, rendered });
    }
    const elements = scope.nodeType === 1 ? [scope, ...scope.querySelectorAll('[title],[aria-label],[placeholder]')] : [];
    for (const el of elements) {
      if (el.closest('[data-i18n-native]')) continue;
      for (const attribute of ['title', 'aria-label', 'placeholder']) {
        if (!el.hasAttribute(attribute)) continue;
        const saved = originalAttributes.get(el) || {};
        const current = el.getAttribute(attribute);
        const source = saved[attribute]?.rendered === current ? saved[attribute].source : current;
        const rendered = translateText(source, language);
        if (current !== rendered) el.setAttribute(attribute, rendered);
        saved[attribute] = { source, rendered }; originalAttributes.set(el, saved);
      }
    }
    observer?.observe(document.body, { childList: true, characterData: true, attributes: true, attributeFilter: ['title','aria-label','placeholder'], subtree: true });
  }
  function setLanguage(value) {
    language = value === 'en' ? 'en' : 'zh-CN';
    document.documentElement.lang = language;
    try { localStorage.setItem('harpoon.language', language); } catch { /* optional */ }
    const select = document.getElementById('language-select');
    if (select) select.value = language;
    localize();
    window.dispatchEvent(new CustomEvent('harpoon:language', { detail: { language } }));
  }
  root.HarpoonI18n = {
    get language() { return language; },
    text: value => translateText(value, language),
    bilingual: value => typeof value === 'string' ? translateText(value, language) : value?.[language === 'zh-CN' ? 'zh' : 'en'] || '',
    setLanguage, localize,
    register(entries) { Object.assign(catalogue, Object.fromEntries(entries)); rebuild(); localize(); },
    registerScenarios(entries) { Object.assign(scenarios, entries); window.dispatchEvent(new Event('harpoon:catalogue')); },
    scenario(id) {
      const item = scenarios[id];
      return item ? { title: item.title[language === 'zh-CN' ? 'zh' : 'en'], summary: item.summary[language === 'zh-CN' ? 'zh' : 'en'] } : null;
    },
    bindCanvas(ctx) {
      if (ctx.__harpoonLocalized) return;
      ctx.__harpoonLocalized = true;
      for (const key of ['fillText', 'strokeText', 'measureText']) {
        const method = ctx[key].bind(ctx);
        ctx[key] = (text, ...args) => method(translateText(text, language), ...args);
      }
    },
  };
  document.addEventListener('DOMContentLoaded', () => {
    observer = new MutationObserver(records => {
      const scopes = new Set();
      for (const record of records) {
        if (record.type === 'characterData' || record.type === 'attributes') scopes.add(record.target);
        else for (const node of record.addedNodes) if (node.nodeType === 1 || node.nodeType === 3) scopes.add(node);
      }
      for (const scope of scopes) if (scope.isConnected) localize(scope);
    });
    document.getElementById('language-select')?.addEventListener('change', e => setLanguage(e.target.value));
    setLanguage(language);
  });
})(typeof window === 'undefined' ? globalThis : window);
