# 第14章及扩展损伤表覆盖台账（2026-10-21）

本表是审计结果，不是规则实现完成率。`source`表示原页核对，`data`/`execution`表示当前Rust状态；旧Python数据另外注明，不能据此替Rust报完成。每项完整条件仍需拆成最小验收案例。

共80个审计项：基础/工程77，明确可选3；扩展表14个域全部视觉核对。这里的计数是审计拆项数量，不代表原书的规则条数。

|ID|项目|原文位置（英文PDF）|数据/执行|当前差异|
|---|---|---|---|---|
|HV26-DMG-PHASE|阶段同时结算|2.3.3/14.1；20,127|missing/approximation_or_partial|当前1秒tick逐命中立即改HP、系统和alive；未建立规则阶段损伤账本。|
|HV26-DMG-DP|损伤点与估算浮动|14.1；127|partial/approximation_or_partial|所有命中使用scalar DP乘0.85–1.15随机浮动；不是书中固定DP程序。|
|HV26-DMG-SPEED|DP与最高航速|14.1.1；127|missing/approximation_or_partial|max_speed只乘mobility；Ford损伤834后仍32节。|
|HV26-DMG-MASSIVE|严重舰体损伤|14.2.1；132|missing/missing|未按剩余DP统一限制武器、潜艇深度和航母起降。|
|HV26-DMG-SMALL-DP|F/G不跨回合累计普通DP|14.1/14.1.2；127,129|missing/approximation_or_partial|所有舰艇沿用累计HP归零流程，没有size F/G语义。|
|HV26-DMG-RATIO|每阶段损伤比率|14.1.2；127,129|root_python_table_only/approximation_or_partial|每击10%–40%概率，最多一次常规关键损伤。|
|HV26-DMG-RATIO-MIN|1%原始DP门槛|14.1.2；129|root_python_table_only/approximation_or_partial|30mm单击8DP对1667DP目标，512个种子产生51个关键事件。|
|HV26-DMG-RATIO-HIGH|超过1与达到3的比率|14.1.2/14.2.1；129,132|root_python_table_only/missing|未执行上述边界；概率上限替代。|
|HV26-DMG-MISSILE-EXTRA|导弹附加关键损伤|14.1.5；130|root_python_partial/missing|未有extra_critical_dice字段与独立调用；不能把骰式加到舰体DP。|
|HV26-DMG-MISSILE-FIRE|导弹自动火灾与例外|14.1.2/14.1.5；129,130|root_python_partial/missing|未执行独立自动火灾；随机Fire行不能替代。|
|HV26-DMG-KEEL|单次大伤害龙骨折断|14.1.2；129|missing/missing|未执行龙骨检定；HP归零或任意武器殉爆不是替代。|
|HV26-DMG-LIGHT-GUN|小口径限制|14.1.3；129|root_source_index/approximation_or_partial|小口径沿用Gun全列，能产生轮机、声纳、进水、CIC等。|
|HV26-DMG-BOW-EXTRA|估算首尾附加10%|14.1.2/14.1.7；129,130,131|unsupported_estimate/approximation_or_partial|现有独立10% bow_collapse/rudder分支，注释为work-order估算。|
|HV26-DMG-CLASS-AVIATION|航空舰船识别|14.1.2 footnotes；128|missing/approximation_or_partial|以PlatformClass.deck.is_some()判定，直升机甲板护航舰也会进入航空判断。|
|HV26-DMG-CLASS-MISSILE|导弹命中航空舰船/小艇|14.1.2；128|missing/approximation_or_partial|critical_column_for只按武器kind选择Missile列，不查询目标类型。|
|HV26-DMG-CLASS-SUB|水面/水下潜艇与武器|14.1.2/expanded general；128,129|missing/approximation_or_partial|underwater或Torpedo始终使用同一水面水下攻击列，无潜艇重/轻列。|
|HV26-DMG-CLASS-ERA|年代/商辅/登陆舰列|14.1.2；128|missing/approximation_or_partial|仅Gun/Missile/Torpedo/Bomb四类，无这些选择依据。|
|HV26-DMG-STARS|带星号部位穿透|14.1.2/14.1.6；128,130|missing/approximation_or_partial|CriticalRow无星号/部位字段；不能用一个全局starred列表代替。|
|HV26-DMG-ARMOR-PEN|Pen与装甲厚度|14.1.6；130|missing/approximation_or_partial|无Pen、belt/deck cm字段，使用类名首侧尾估计倍率。|
|HV26-DMG-ARMOR-ASPECT|武器剖面选择装甲部位|14.1.6；130|missing/approximation_or_partial|只按类名与来向0/90/180估计，不按飞行剖面/炮射程选belt/deck。|
|HV26-DMG-BOMB-PEN|航空炸弹穿透条件|14.1.6；130|missing/missing|未有炸弹投放方式/Pen等字段。|
|HV26-DMG-TORP-TARGET-DP|鱼雷对舰/对潜DP|14.1/Annex F；127,131|missing/approximation_or_partial|单一damage=120同时用于船和潜艇。|
|HV26-DMG-TORP-FUZE|触发与感应引信|14.1.7；130,131|missing/approximation_or_partial|WeaponDef无C/I字段和攻击深度选择；统一Torpedo装甲倍率。|
|HV26-DMG-TPS|左右舷抗雷结构|14.1.7；131,132|missing/approximation_or_partial|仅常数乘0.75，无左右舷容量或部位消耗；us_ford未落入cvn估算表。|
|HV26-DMG-TORP-BOW|窄首尾触发命中|14.1.7；130,131|partial/approximation_or_partial|角度≤45/≥135一律视作首尾减伤，无引信/具体首尾条件。|
|HV26-DMG-TORP-FLOOD|触发鱼雷自动进水|14.1.2/14.1.7；129,131|missing/approximation_or_partial|只有随机Flooding行；无TPS穿透条件和独立自动结果。|
|HV26-DMG-TORP-WAKE|尾流制导先定损伤|14.1.2；129|missing/approximation_or_partial|缺少依据制导工作模式改写前两个关键结果的程序。|
|HV26-DMG-SUB-PRESSURE|鱼雷耐压壳检定|14.1.2；129|missing/missing|未有直径/船壳构型/尺寸与独立80/60、70/50等检定。|
|HV26-DMG-MINE|水雷部位与引信|14.1.7/13.6；130,131|missing/approximation_or_partial|无对应武器/引信/作用方式损伤模型。|
|HV26-DMG-AIRBURST-DP|空爆不扣普通舰体DP|14.1.4；129,130|missing/approximation_or_partial|Bomb列混合同普通炸弹/火箭，仍先扣HP。|
|HV26-DMG-ARM|反辐射专用程序|7.4.2/14.1.4；77,129,130|missing/approximation_or_partial|cn_yj91配置SeaSkimmer、damage10；无指定雷达关键与PRH行为。|
|HV26-DMG-AIRBURST-COUNT|炸弹/集束/火炮空爆|14.1.4；129,130|missing/missing|未区分这些来源，Bomb/Rocket直接选同一列。|
|HV26-DMG-AIRBURST-MAG|空爆武器行不殉爆|14.1.4/expanded general；130|partial/approximation_or_partial|Bomb列确有禁止殉爆分支和种子测试，但YJ91仍选Missile列；不能据此宣称全空爆完成。|
|HV26-DMG-BOMB-DECK|大型炸弹自动甲板关键|14.1.2；129|missing/approximation_or_partial|只有随机Bomb表FlightDeck行，没有该独立自动关键。|
|HV26-DMG-GUN-DECK|远距离大口径炮甲板关键|14.1.2；129|missing/missing|未执行该额外程序和射程段/口径条件。|
|HV26-DMG-DECK-LOC|飞行甲板位置与起降限制|14.5 Flight Deck；135|partial/approximation_or_partial|只写ext.flight_deck_hit=1并起火；air.rs不起降消费。|
|HV26-DMG-DECK-RECOVER|受损甲板回收|14.5 Flight Deck；135|missing/approximation_or_partial|compatible_recovery只查翼型、阻拦索及容量，无关键损伤检查。|
|HV26-DMG-HANGAR|机库与甲板停放飞机链|14.5 Flight Deck/Hangar；135|missing/approximation_or_partial|无停放位置、机库关键或被毁飞机链。|
|HV26-DMG-AIRCRAFT-STORED|舰载机关键与可选装弹状态|14.5 Aircraft；134|missing/missing|未有这套舰内飞机毁伤链；可选挂弹条款不能隐式开启。|
|HV26-DMG-AMMO-FUEL|航空弹药/燃料与处置|14.5 Ammo/Fuel；134|missing/approximation_or_partial|无Ammo/Fuel类型、相关状态或处置命令。|
|HV26-DMG-AIR-PLOT|舰桥/塔台分支|14.1.2/14.5；128,134|missing/approximation_or_partial|Bridge统一扣fire_control/mobility，无塔台及时间恢复分支。|
|HV26-DMG-FIRE-SEVERITY|百分比严重性|14.4；133,134|missing/approximation_or_partial|固定60/120秒时长、封顶180/300秒、每秒0.05/0.15DP。|
|HV26-DMG-FIRE-DELAY|二次损伤时序|14.1/14.4；127,133|missing/approximation_or_partial|新火灾第一秒即0.05DP损失；未区分例外。|
|HV26-DMG-DC-CATEGORY|合计严重性与级别表|14.4；133|missing/approximation_or_partial|每秒80%进度，火60/水120点，与尺寸/年代/合计严重性无关。|
|HV26-DMG-DC-ROLL|损管骰与恶化|14.4；133,134|missing/approximation_or_partial|按进水优先累积进度并清零时长；有限时长还能自然结束。|
|HV26-DMG-DC-OPS|重大灾情限制行动|14.4；133|missing/missing|未按灾情严重性联动飞行、航向、速度和潜艇深度。|
|HV26-DMG-DC-CATASTROPHE|失控火灾/进水|14.4；133|missing/missing|未有合计失控灾情、倾覆及应急注水行为。|
|HV26-DMG-DC-CREW|抽调人员与邻舰援助|14.4；134|missing/missing|未有损管人员配置、代价、靠帮协助或拖船能力。|
|HV26-DMG-ENG|轮机与电力|14.5 Engineering；135|missing/approximation_or_partial|随机降低mobility0.4–1，无实际伴随火灾、电力或累计次数。|
|HV26-DMG-BRIDGE|舰桥失能时序|14.5 Bridge；134|missing/approximation_or_partial|固定mobility/fire_control各扣0.3，无导航雷达/延迟/分类型恢复。|
|HV26-DMG-RUDDER|舵卡死/失灵与潜艇深度|14.5 Rudder；135|partial/approximation_or_partial|只写rudder_hit并扣mobility，movement未消费卡舵方向/转向/深度限制。|
|HV26-DMG-CIC|CIC与局部控制|14.5 CIC；134,135|partial/approximation_or_partial|fire_control置0、radar减0.5，声纳/单个设备/局部火控未按原文区分。|
|HV26-DMG-SENSOR|传感器与对抗设施|14.5 Sensor；135|partial/approximation_or_partial|按65/25%偏好将整体radar或sonar置0，未从实际设备清单均匀抽取；对抗装备未接入。|
|HV26-DMG-WEAPON|随机发射架与指挥仪|14.5 Weapon；135,136|partial/approximation_or_partial|从有弹项选mounts最少者；无指挥仪损伤，只把整项ammo清0。|
|HV26-DMG-REPEAT-MOUNT|空弹/已毁武器再中|14.5 Weapon；136|missing/approximation_or_partial|过滤ammo>0，已损/空弹项退出选择，改变后续命中分布。|
|HV26-DMG-MAGAZINE|甲板上爆炸与甲板下殉爆|14.5 Weapon；135,136|missing/approximation_or_partial|任一非Bomb关键的有弹挂载10%直接整舰归零；弹药种类/位置未区分。|
|HV26-DMG-FLOTATION|F/G浮力格|14.5 Flotation；135|missing/missing|未有size、浮力格与相应速度程序。|
|HV26-DMG-PERSONNEL|小艇6个人员因数|14.5 Personnel；135|missing/missing|未有人员因数和指令约束。|
|HV26-DMG-SUB-BATTERY|电池/AIP/核潜艇|14.5 Battery；134|missing/missing|未有Battery/AIP结果与核潜艇特例。|
|HV26-DMG-SUB-CONTROL|潜艇控制舱|14.5 Control；135|missing/missing|未有Control及轻重列。|
|HV26-DMG-SUB-DEFORM|船体变形与深度限制|14.5 Hull Deformation；135|missing/missing|未有耐压壳形状、损伤次数或深度联动。|
|HV26-DMG-SUB-HULL|船体穿透与上浮窗口|14.5 Hull Penetration；135|missing/missing|未有船体穿透程序，仅普通HP归零。|
|HV26-DMG-CARGO|货舱伤害链|14.5 Cargo；129,134|missing/missing|未有Cargo类型、位置、货物数量及连锁处置。|
|HV26-DMG-SINKING|沉没延迟与例外|14.3；133|missing/approximation_or_partial|hp<=0立即alive=false并记Sunk，120秒清理仅UI残留，不是规则沉没状态。|
|HV26-DMG-RCS|受损隐身舰特征|14.2.2；132|missing/missing|未按伤害改变rcs。|
|HV26-DMG-NOISE|水下损伤声学特征|14.2.3；132|missing/missing|未有对应损伤来源阈值和声学退化。|
|HV26-DMG-BEACHED|搁浅舰船毁伤|14.2.4；132|missing/approximation_or_partial|当前海岸路径保护没有该损伤状态；不应用照片颜色推水深。|
|HV26-DMG-CREW-TRANSFER|人员转移|14.2.5；132|missing/missing|未有小艇人员及转移程序。|
|HV26-DMG-REPAIRS|战后修理时间与概率|14.6；136|missing/approximation_or_partial|最后命中300秒后开始每60秒统一恢复0.2系统值。|
|HV26-DMG-REPAIR-TYPES|修理分类型与副作用|14.6；136|missing/missing|未有类型化修理、战斗结束基准、重燃、补片和舵恢复骰。|
|HV26-DMG-NUCLEAR|核武器效果|14.1.8；131,132|missing/missing|未有核武器效果执行。|
|HV26-DMG-AIR-HIT|飞机命中及战后受损回收|14.1/14.9；127,137|partial/approximation_or_partial|共享命中入口仍走HP/舰船关键；大量武器DP高会即时毁机，但未核验所有路径满足基础退出/战后分类。|
|HV26-DMG-SERVICEABILITY|装备完好率（可选）|14.7；136|missing/missing|未有该可选模式与状态，未以此标为现有常规局必需缺陷。|
|HV26-DMG-SAR|搜救（可选）|14.8；136,137|missing/missing|未有搜救流程，原文标optional；不与基础损伤遗漏计为一类。|
|HV26-DMG-AIR-DAMAGE-OPTION|可选即时飞机分级损伤（可选）|14.9；137|missing/missing|未有独立模式选择及完整分级状态。|
|HV26-DMG-RNG|每个命中唯一随机身份|repository deterministic replay；127|missing/approximation_or_partial|scope_rng(target,damage)重复创建同tick同target同purpose的流。|
|HV26-DMG-GEOMETRY|命中身份与有符号来向|14.1.6/14.1.7；130,131|partial/approximation_or_partial|nearest projectile匹配并abs角差，丢左右舷；近枪射击也估计最近发射者。|
|HV26-DMG-DICE-LOG|完整掷骰和规则追踪|repository deterministic replay；127,129,130|partial/approximation_or_partial|Impact/CriticalHit只记结果，没有每枚dice和阶段输入账本；世界可序列化不等于规则审计轨迹完整。|
|HV26-DMG-ROOT-DATA|旧Python表转录错误|14.1.2 Critical Hit Table；128|incorrect_root_python/approximation_or_partial|根data/rules/damage_tables.json的underwater只有19格，guided复制post列且两者与原表不符；verified:true不能作证。|
|HV26-DMG-VARIANT|变体与配置拆分|China Annex D1/H4/F1; America A4；130,131|partial/approximation_or_partial|YJ12舰空共用216nm/48/SeaSkimmer；YJ91普通反舰；Yu6/10折中120；Ford缺装甲/设施记录。|

## 扩展表全目录

`partial`只表示有部分D100区间和通用效果，选择条件、星号和完整效果均未通过严格验收。可选CaS表未开启。

|规则ID|中文PDF页|表|当前Rust|
|---|---:|---|---|
|HV-ECT-00|2|通用规则与轻型武器的致命损伤|index_only_not_rust_data；missing|
|HV-ECT-01|3|1955年前水面战斗舰船（尺寸A至E）|index_only_not_rust_data；missing|
|HV-ECT-02|4|1955年后水面战斗舰船（尺寸A至E）|partial_d100_bands_in_rust；partial_generic_effects_and_wrong_selection|
|HV-ECT-03|5|制导武器对水面战斗舰船（尺寸A至E）|partial_d100_bands_in_rust；partial_generic_effects_and_wrong_selection|
|HV-ECT-04|6|战斗小艇（尺寸F至G）|index_only_not_rust_data；missing|
|HV-ECT-05|7|航空舰船|index_only_not_rust_data；missing|
|HV-ECT-06|8|水下攻击|partial_d100_bands_in_rust；partial_generic_effects_and_wrong_selection|
|HV-ECT-07|9|空爆和弹片命中|partial_d100_bands_in_rust；partial_generic_effects_and_wrong_selection|
|HV-ECT-08|10|商船和辅助舰船（尺寸A至E）|index_only_not_rust_data；missing|
|HV-ECT-09|11|运输小艇（尺寸F至G）|index_only_not_rust_data；missing|
|HV-ECT-10|12|潜艇（重度损伤）|index_only_not_rust_data；missing|
|HV-ECT-11|13|潜艇（轻度损伤）|index_only_not_rust_data；missing|
|HV-ECT-12|14|《海上指挥》主要水面战斗舰船（尺寸A/B）（经调整）（可选）|index_only_not_rust_data；missing|
|HV-ECT-13|15|《海上指挥》次要水面战斗舰船（尺寸C/D/E）（经调整）（可选）|index_only_not_rust_data；missing|
