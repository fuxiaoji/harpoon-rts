# 鱼雷来源执行续包：已验收子集

本包仅适用于显式 `hv2026-rts-v4`。历史next包的 `artifacts/rules-next-20261023/validation/workspace-final.log` 记录53个目标823/0/5、鱼雷28项；following包另以56目标877/0/5封存，鱼雷40项包含本方声明、真实管状态和共享布雷管。两份冻结证据保持原义。下表增加当前覆盖层，本章及全书仍未完成。

2026-10-24在建after-following定向日志`contact-mine-green-wake-group-red-20261024.log`中旧鱼雷40项和航空Mk54十八项通过，Wake为17绿/共骰分组1红；后续安全分组pending门已落，最终执行结果由主代理新增日志后补记。这里没有提前登记完整工作区通过数、HTTP或历史重演。当前Wake详细范围见`torpedo-wake-20261024.md`，航空精确ContactId/互扰披露及R-054见`aviation-asw-admission-20261023.md`。

来源工作流：先查中文核心RAG“鱼雷攻击表”，再视觉复核英文2026 PDF17、28、79–80、110–115；参数附件为US Navy2025 PDF84/F2。新补检索“10.3”“潜射”“尺寸分级”分别交叉定位水下发射和物理C/D均Small。结构数据在 `data/rules/torpedo-2026.json`；转录脚本及原页在 `artifacts/rules-all-20261023/torpedoes`，PDF79–80图在其同级 `aaw` 目录。默认附件只绑定Mk48 Mod7 CBASS、Mk54 mod0/1的已核参数，不据旧同ID推导其他改型。

实际入口为 `weapons::manual_fire/fire_control_scan/advance_torpedo` → `torpedo2026`；直航支路在 `torpedo2026/straight.rs`。源信息不足时拒发或保存完整弹体与既有原始骰待裁，不回落旧3.5nm探测、0.55诱骗乘数或def.pk。平台管数、初装、储备、通道、水文与目标特征必须有明确来源或scenario_assumption。

| 规则 | Source / Data | 已接执行与通过案例 | 保留范围 |
|---|---|---|---|
| 10.1 管内装填/重装 | PDF110/10-1；显式初装、储备、180s重装、540s换弹及并行配置 | 实际发射清空一管；真实180秒重装、弹库守恒、存档恢复；following补稳定本方来源管/储备/装填钟、布雷共享同一物理管 | 完整来源平台/VLS装配和更多换弹/并行极限仍未验全 |
| 10.2 目标与源门 | PDF110；源附件目标域/潜深、现时定位/分类、声明FCS/水文 | 缺源解不把contact quality升为Good；三玩家入口`fire_at_contact`贯穿所选报告资格/估计位置/释放身份，禁止真alive/depth/class或同body其他报告补资格；compat入口多候选拒绝 | 普通共享contact/TDL不能冒充本机FCS；声明fixture不等于真实型号附件；R-054空投入水后立即就位/攻击仍开放 |
| 10.2.1 发射角 | PDF110全部Gyro1/2、声导、线导、Shkval偏置 | 管轴和逐弹声明方位检查；声导至少15度及同180秒追加源约束已接 | Wake偏置不在打印表，缺源绑定阻断；并非全部角度边界已独立验完 |
| 10.2.2–10.2.3 直航 | PDF110–113；672概率格、aspect、closest距离、平局取长、静止上移 | 真实Gyro1/2稳定salvo；相对扫掠、友舰/非预定舰、一组一D100、部分命中余弹拆单与全失保组、普通损伤队列、保存恢复均通过；官方22→1中、D10029/30和D级Small锚通过 | 非自导线导未接；可变DangerZone选项未转录启用，不计作本包完成 |
| 10.2.4 逐弹放置/声导 | PDF113及79–80；逐弹placement、±60度、四代搜索、噪声/跨层/浅水混响 | 真实Fire源80%替代旧pk0、原实体/原骰保存；4000yd≈1.974946nm、1000yd≈0.4937365nm独立单位锚通过 | 航空150yd圆/500yd每圈续航已接；完整纵向螺旋、机会目标重获仍未完 |
| 10.2.5 声学命中 | PDF114；16行64格 | 有效EW对象给源代ACM列；探测后实际满足至少两项才享规避；51%/31%官方锚、真实Gen4+ACM2+规避的D10058命中/59失误通过 | 未把普通ACM失误直接认作拖曳器成功诱爆；完整诱饵重获仍未完 |
| 10.2.6 尾流 | PDF74/80/114；明确作者几何、半程后placement+1、75%闭合、规避列+0.05 | 单弹真实扫掠穿流/连续链跟随、探测后25kt且90度规避、Gen3/4拖曳ACM-only再独立终段、真实consume_towed和伤害队列已接；初探测命令前保留Threat基线 | 完整同型共骰未明确，多弹/缺账安全pending；早穿半程、多有效拖曳、实际他舰分叉/缺链均不猜继续；分叉尚未自动ACM1/2/3改跟 |
| 10.2.7 再攻击 | PDF114–115 | 声学失误后1000yd直航/按代右圆及wire40%断线；Wake真实miss后500yd=0.2468682505399568nm、右180度返回/保存原骰已接 | Poor floor/NoAttack来源冲突仍保存原骰pending；完整失锁/重获、机会目标、自航诱饵和分叉优先级未完 |
| 10.3 水下导弹/ASW standoff | PDF115及28；结构化浅水/潜望深度、3–10kt | manual/auto/低层launch统一必要门；真实拒发不扣弹、底层不绕门通过；停止全图flash，真实发射以稳定ProjectileData ID进入探测瞬态 | 只是必要门；源VLS/导弹初装、视觉/SS雷达发射暴露整链仍未完；运行时0m代表浮航，缺独立源程序仍阻断 |
| 10.4 Shkval | PDF115 | 未实施 | 核/定距爆炸/噪声致盲不以普通自导鱼雷替代 |
| 10.5 硬杀伤 | PDF115；RBU/RKPTZ1/ATT1/ATT2共8格 | source Hardkill动作要求本方已探测/分类、显式源系统/射程/射速/弹库；真扣弹、冷却、源骰与退役已接；8格边界锚通过 | 实际来源平台部署案例仍待验收，不能按helper通过认定全机制完成 |
| 7.5.4 / 6.4 线导 | PDF79–80/75；源发射/速度/转向/深度表、通道 | 实际发射断线、180秒条件检查、WireOrder/CutWire、同管未切线禁止重装已接 | 中途机动按采样检查、非自导线导zig分支、单线/双线回传粒度与本方弹体视图仍需完善 |
| 7.5.7 双速 | PDF80及US PDF84/F2 | 按各档源射程归一化余程，锁定后最快档；Mk48为65/15、40/22.6、28/25，Mk54为45/6、30/8 | 独立换档比例/余程边界案例仍需补，不把普通真实Fire当作全部换档验证 |

直航source表只给命中数，未规定具体哪枚命中；本引擎以比例运动的最近接近距离、再以稳定ID分配中弹成员。这是明确的RTS身份适配，不改变原始D100、源概率或命中数。射程量至运行中的鱼雷；默认0.25nm危险区是独立条件，不能无据再加目标中心射程门。near/far及真实run不超附件射程、到程退役、未知介入舰整组pending与证据跨tick/save稳定均已通过。

原始placement、acquisition、terminal、runout、reattack、hardkill及pending原因只写Referee；本方声明/控制事件保留本方。following40已移除原始本方鱼雷UnitObs和真alive在途统计，稳定发射声明不随秘密退役消失，未知在途数保持null。after-following十八项再关闭精确ContactId串轨和秘密另一枚空投弹存活改变即时Side回执的窄项，物理互扰事件给Referee。R049的bda_to_shooter自动精确HP/DP/hits投影在next包已收口；完整BDA、击沉确认、其他事件及DualWire精确报告粒度仍未全部核验，不宣称全路径信息隔离完成。旧 `torpedo-wake-own-status-draft-20261023.md` 保留来源草案含义，当前范围以独立新说明为准。

关键先红后绿证据：

- `torpedo-red.log`：隐藏raw trace与全图flash两红。
- `second-core-run.log`、`third-core.log`：码/海里错误1000倍的独立物理锚先红再绿；源概率未变。
- `third-core.log`、`fourth-core.log`：物理D级误作VSmall先红，源§2.1与aspect结构化修复后绿；真实straight旧pending四红已记录。
- `torpedo-hidden-admission-red.log`：敌真实潜深/foil决定命令许可的两条真实红。
- `fifth-core.log`：straight真实执行已通；夹具总HP断言未计合法次生进水/严重损伤，改查受害方普通Impact=1而不抹生产次生效果。
- `torpedo-percent-red.log`：D10029@29%与真实D10058@58%误失、59正确失；统一比较D100/100<=源Ph，避免Ph×100的二进制舍入。执行修订进入v4规则指纹。
- `workspace-final.log`：最终53目标823通过/0失败/5忽略，鱼雷28项全部通过。

待裁 `R-TORP-POOR-REATTACK`：10-5正文Good→Fair→Poor并保持Poor，与10-6例初始Poor退NoAttack不一致，原骰后该分支继续阻断。Wake仍未自动完成同型共骰、他舰诱偏/改跟、多分叉/多拖曳优先级、VSmall映射、失锁/机会目标、53-65K特殊发射深度与完整Oscar实装；当前65-76A为历史精确武器参数配作者初装/离轴/几何条件，不能外推全部附件或I引信损伤元数据已全核。以上均不改变全章及全书未完成的结论。

2026-10-24最终联合验收：完整工作区59目标933通过/0失败/5忽略，本分项日志及HTTP/历史重演范围见[本包报告](all-rules-after-following-20261023.md)。前文分项与中间红绿为历史记录；整章/全书和明列待裁项仍未完成。
