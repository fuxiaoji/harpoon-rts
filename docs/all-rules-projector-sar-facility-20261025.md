# 投射器、救援发现与设施维修：声明子集最终验收

本批声明子集已完成最终联合验收：**Rust122目标、1560通过、0失败、5既有忽略，Node83/83**；实际HTTP、MCP stdio、排队检查点跨进程恢复/命令重演及双语界面均通过。状态为`tested_declared_subsets`、`final_signoff=true`，仅签收本报告明确范围。工作于2026-10-25开始、26日完成，路径保留批次开始日期。

本批证据独占 `artifacts/rules-book-next-projector-20261025/`。机器可读四路台账为 [rules-projector-sar-facility-20261025.json](rules-projector-sar-facility-20261025.json)。此前恢复批的330输入、622证据及报告保持冻结；新批不追改旧战局、旧来源表或旧验收结果。

**整章、全书、全部可选规则和全部真实Annex装配均为false**。没有完整原子分母，不给完成百分比。测试数不是规则数；待裁守卫通过不代表相应物理机制完成。

## 来源与实验边界

权威为2026-01-01英文数字更新版 `14-2.HpnV_Rules_1_Jan_26.pdf`，SHA256 `8196ffc715bbcccd862d8597c607de7816dac5baee31fa659f89dfaf4323700c`。三份中文核心资料用于强制RAG检索和交叉核对，表格依据原页视觉复核，英文优先。来源合同与原图分别在本批 `source-contract.md`、`sar/source-contract.md`、`facilities/source-contract.md` 及其来源清单；这些准备文档保留落盘时状态，由新运行日志记录后续事实。

- **RGB10**：英文PDF99/8-19、100/8-20及中国2025附件PDF75/E-1。H-only、6枚一组、最大0.5nm、最大Int IV；Contact命中率/DP不适用，Major为11%/11DP、Minor为20%/6DP，单次D100按绝对阈值比较。Fair最终正常舍入一次，Poor为合法耗弹后的自动脱靶。该行没有C成对架备注。P/S架来源另见中国附件PDF16–17。
- **火控与同时性**：英文PDF72/6-4、73/6-5、100/8-20及127/14-1。首域只用真实Gen4及以上主动搜索声纳、同一接收机的原生测量与运动，不安装虚构鱼雷导引头。`RTS-PROJECTOR-PHASE-01`把本tick接受的计划射击放在共同伤害batch关闭前结算，属于明确的实时阶段映射。
- **救援发现**：英文PDF136–137/14-10–11及PDF62–64的Visual程序。首域为白天水面舰Visual、开阔水面、无云冰、1–9名尚未救起者、损失后首六小时；G/Stealthy、no-wake范围乘0.5一次，实际距离小于0.3kyd按表注自动发现。没有从已知损失事件直接生成玩家可见标记。
- **设施维修**：英文PDF116–118/11-1–3。原书规定维修建筑损失降低修理速率，未给统一倍率；跑道资源和时间由想定声明。测试的每切口600秒/一队及0.5倍率均为明确的作者实验参数，不是书中通用值或真实基地性能。

原Ford船体/DP/source CS6搭载原956EM RBU架与库存、独立实验Gen5主动/被动声纳，是本批声明的实验装配；不声称真实Ford有RBU，也不声称真实956EM完整装配已经验收。每架1组已装、3组储备同样是明确初配；未印的最小射距、飞速、引信误差、实际重装时间保持未知，储备不自动装填。

## 四路实施与已观察测试

各行独立记录Source、Data、Execution、Tests，可共享测试文件，计数不能重复相加。下表按最终来源、测试、协议及界面证据签收明确子集，不外推整章或全书。

| 声明域 | Source | Data | Execution | Tests：当前实际范围 |
|---|---|---|---|---|
| RGB10模型与装配 | §8.5.3、E3及P/S架原页 | `asw-weapons-2026.json`精确H-only行；严格projectors配置、逐架loaded/reserve、reload:null | 全部验证后原子发布；唯一单位/聚合架/physical身份和原库存守恒 | 最终4项inline配置/纯表通过；全inline7/7，不外推全部projector/真实fit |
| 同接收机中性FCS | §6.3.4、8.5.3.1 | `firecontrol-2026.json`明列首域；没有torpedo_seeker | `own_projector_solution`核实际主动声纳/功能、原生measured aim、连续运动及源CS | 原生6/6含无虚构导引头对照；真实HTTP裁判保存独立核Good。MCP公开态没有Good字段，不冒称通过stdio读到质量 |
| 实际整组发射 | §8.5.3及阶段映射 | 一次Fire rounds1等于完整6枚；逐物理架Tac时钟、稳定pattern ID | 手动/自动在通用聚合冷却前分流；opaque permit重验；精确component、aggregate与loaded各扣一次 | 原生6/6、inline7/7；validate全World只读、克隆重投拒绝、两侧预许可各component4→3/aggregate8→6均过；真实HTTP/stdio Fire1和排队跨进程验证另列 |
| 共同伤害 | 英文14.1及扩伤中文PDF12/13明确列8.5.3 | `Hit.projector`可选真实Major/Minor元数据 | `queue_projector`进入同一primary DP/batch/Impact；实际`sub_major`与`sub_minor`，不伪装Mine/Torpedo，不走旧Pk弹体 | 自然Major11DP/Minor6DP及实际两列、完整World恢复通过；真实同tick浮航H效果待裁inline最终通过，保耗弹/noDP/noImpact，不放宽前置门 |
| SAR原生发现 | §14.8及Visual原表 | 私有幸存者物理点、真实测量快照、本方不透明别名 | 正常Full扫描期`scan_world`，恢复DetectionState后commit；不新增Entity/Contact/FCS/战果 | 最终SAR6/6：真实发现/JSON Begin、雷达/损失通知反例、9nm半程、未见group不改别名、完整World恢复、私有字段拒绝；实际运输未宣称发现正域 |
| SAR共用命令 | §14.8及明确权限/快照合同 | `search_and_rescue`五action，四无参变体为空struct；内部人工标志不属公共schema | 本方label/真实fit/已有task共同预检；实际apply成功才持续manual；HTTP/MCP共用核心入口 | MCP进程内新增2/2、MCP目标28/28；实际HTTP/stdio各13预期拒绝，Cancel实际apply后持续manual。真实运输不宣称完整发现/Begin/救起 |
| 设施绑定维修 | §11.1/11.5及Generic Land Targets维修行 | 可选`maintenance_provider`，可序列化work ledger；作者0.5与源事实分账 | 真实本方唯一维修设施；按elapsed tick累工；先结旧interval再损伤；未知/毁灭拒新工、暂停旧工，不沿旧due免费完工 | 最终设施9/9；真实绑定、未知/毁灭暂停、作者半速/动态工量、同tick幂等、旧无绑定及完整World恢复均过；源损伤及恢复明确为已裁边界，不冒称建筑修理 |
| 双语控件 | 共用命令及本方数据合同 | projector库存、本方survivors、rescue_fit与纯草稿 | 武器面板发完整一组；救援仅选择本方已发现label；显式按钮发请求，排队不伪造成功 | Node83/83；实际CUA一次Fire请求排2合法命令，真实pattern/库存/手控与中英两尺寸检查通过；SAR只验0本方report时3按钮禁用，实体平板未验 |
| Full死亡清理时钟 | 明确引擎持久化合同，非新增Harpoon战斗规则 | `persistence-2026.json`、序列化`rts.cleanup.death_tick`、原RTS120/0秒时长 | Full sink记录实际死亡tick，cleanup不用serde(skip)缓存；纳入Full指纹，Legacy/v2/v3原时钟/线形保持 | 先1真实红；Full实际死亡T200在原/恢复World中留至319、320同时清理；Legacy控制及设施9/9通过 |

SAR用例调用生产 `damage::sink_by`，作为**明确已裁损失的消费者边界**生成物理group，随后由真实传感器发现；没有手填幸存者、Contact、FCS、HP或原骰。它不代表完整武器命中→击沉→所有生还原因的自然链。设施用例通过生产`apply_damage`输入明确已裁设施损伤，恢复夹具也明确是裁判已裁输入；不冒称已有原生建筑修理。

旧装配与本方投影对照属于上面对应声明域的兼容验证，不另计规则：可选字段缺省省略，无绑定跑道保旧due，本方due未知为null，projector与SAR信息走own投影及失联遥测快照。设施旧形状/无绑定/同tick对照和Legacy死亡时钟控制均通过；本批不重复旧长历史局，前批622原证据SHA保持。

## 运行与失败记录

| 记录 | 已观察事实 | 不可外推的结论 |
|---|---|---|
| `projector-source-red-1.log` | 正常编译4/4来源红，最早为未知projectors配置 | 尚未到下游伤害断言 |
| `projector-integration-2.log` | stand-off8绿；projector1绿5前置报告红 | 固定时间不保证90%封顶声纳当窗成功；不能改源概率换绿 |
| `combined-source-1.log` | SAR候选漏observe分页参数，编译失败 | 编译错不算生产行为红 |
| `combined-source-2.log` | projector6/6；SAR2绿4红；设施1绿4红 | SAR真实缺发现与私有字段已复现；设施最早红仅provider字段未保存，不算clock断言已红 |
| `workspace-1.log` | 编译失败：冻结v2示例的穷尽拒绝表漏新Command | 根补入拒绝新命令，未扩大旧研究动作集 |
| `workspace-2.log` | 122目标1555通过/3失败/5忽略 | 不是最终通过，不给本批整体验收签字 |
| `node-1.log` | 83通过/0失败/0跳过 | 不是实际浏览器或实体平板证据 |

workspace-2为122目标1555/3/5：浮航inline固定启动窗口未保有合格的前移位源火控；设施毁灭案例末尾拒绝回执预期未区分残骸在场与120秒后实体已清理；旧LLM全schema计数仍预期40而当前41。浮航夹具最终使用正常SetDepth的真实启动时序及克隆预演，只在本体仍有合法原生Good证据且下一tick实际会到Surface时真正Fire，全部耗弹/pending/noDP/noImpact断言保留，源门未放宽。

设施回执修正后，workspace-3为122目标1556/2/5，又暴露**真实完整World存取错误**，不能继续解释为纯断言问题。独立诊断唯一差异是`/events/11/tick`原323、恢复204，命令日志相同；原因是cleanup把serde(skip)的`last_emit_tick`当作死亡时钟。根在`death-clock-source-red.log`正常编译复现1真实红后，给Full实际sink写入持久`rts.cleanup.death_tick`，cleanup消费它，原RTS平台120秒/弹体0秒时长不变，Legacy/v2/v3原时钟及线形保持。`persistence-2026.json`明确这是技术存取合同，非新增Harpoon数值。定向核心146/0、设施9/0及最终整库1560/0/5通过。详`facilities/persistence-diagnosis-20261026.json`与`failure-diagnosis-20261026.md`，早期猜测由实际因果更新。

candidate-2的352输入是历史候选，不作为最终冻结输入。设施代理曾在candidate-1 ready后追加rubble守卫/案例，已报告前后SHA并停止写入，过渡日志保留。经过明确重捕获和上述修复，最终候选为**353输入**，清单SHA256 `8cff647c7247b0a11c1625ebb445ac61c2c02a296b9a60938b933a4faced7115`，见`final-candidate-1-inputs.json`及`source-snapshot-final-1/`。根仍负责验后输入比对和最终总manifest。

实际HTTP、MCP stdio、跨进程及界面结果均已出，见下节。全部使用新冻结程序和隔离目录；准备脚本不等于运行，未调用旧冻结writer。

## 最终测试与真实协议

| 验证 | 实际结果 | 证据根下路径 |
|---|---|---|
| 整库/网页 | Rust122目标1560/0/5；Node83/0 | `final-workspace-1.log`、`final-node-1.log` |
| 冻结输入 | 353项，最终清单SHA为8cff647c…ed7115，完整源码快照另存 | `final-candidate-1-inputs.json`、`source-snapshot-final-1/` |
| 实际HTTP | PID9404，13非法不排队/暂停本方观察不变；另保存核完整World/inbox。Cancel实际apply后持续manual；原生分类/测量后Fire1，单架loaded减1、reserve不动；裁判原生Good，D100=98实际miss/noDP/未排伤害，T200 | `acceptance-preparation/http-runs/http-final-1/validation.json`及逐次rejection/save审计 |
| 实际MCP stdio | PID15452，92对请求响应ID对齐、0JSONRPC错误、13预期工具拒绝；实际Cancel/原生Fire1/本方库存，T200，子进程已退出 | `acceptance-preparation/mcp-runs/mcp-final-1/validation.json`、`protocol-and-process-closure.json` |
| 跨进程 | PID31140与9404同SHA；T199排队World/inbox完整相同，推进至T200完整World/inbox相同；2批31事件完整重演`e747f31f1d478787` | `acceptance-preparation/cross-runs/cross-final-1/validation.json` |
| 原件保护 | 原14DB+2旧表共16项SHA不变，前批622证据全部原SHA不变 | `protected-originals-final-1.json`、`prior-accepted-package-preserved-final-1.json` |
| 实际界面 | CUA IAB，T195→196；1次真实Fire请求排2合法cmd，实际pattern1、aggregate8→7、单架loaded1→0、两架reserve各3不变、持续manual；1批31事件重演`cdb8f14e7c049e11` | `acceptance-preparation/ui/validation.json`及其queued/applied/rejected/final-replay子记录 |

冻结server SHA256为`a153ed4ce6db45d582ca88ab87a8195cd60de23d1557ad9afed4f742f7307202`，MCP为`41819e03da9e61cb16a0fa10fd52bcea29c45c2be0595ec4faa60b880f26fe3b`，与实际进程回执一致。HTTP的Good仅来自只读裁判保存审计，不作为玩家输入；MCP没有公开本机质量字段，只签收源门接受和真实本方库存。两种运输的SAR正域是无任务幂等Cancel；真实发现/Begin在核心与进程内dispatcher独立验过，不能混作一次完整stdio救援。

HTTP只读裁判审计记录实际`raw_d100=98`、`severity=miss`、`dp=null`、`damage_queued=false`。因此实际协议只声明合法Good发射、整组扣库与隐私合同；Major/Minor命中及共同伤害正域由核心原生p02用例证明，不称本次HTTP射击命中或造成伤害。

实际UI的一次Fire请求通过既有玩家运输明确排入`set_unit_automation(false)`与`Fire`两条合法命令；暂停时**全部非inbox payload**不变，推进后两条实际应用，不能称为单条原子Fire。选舰/浏览保持完整payload不变。已耗架第二次UI尝试后完整payload仍不变，但没有单独记录该次后端运输响应，**不声称实际422回执**。

界面SAR范围仅为本方0报告时三个按钮禁用，不宣称浏览器完成发现、Begin或救起。中文/英文各1600×1000、1024×768的DOM核对无横向溢出，地图与时间可见，受测source/SAR按钮高度至少44px；观察到的浏览器error日志为空。CUA截图已在对话展示，不声称本地截图文件，也没有实体平板验收。

`ui/harness-corrections.md`保留辅助错误：初次helper误引用不存在的payload.record而在推进前KeyError，后改为比较完整非inbox payload；一次临时只读诊断漏utf-8编码失败。合法UI射击只提交一次，修辅助后没有重发合法射击。这些以及下面的cross CLI误参均非生产来源红，未覆盖原存档。

验后`after-acceptance-inputs.json`与最终353项清单逐字节相同，SHA256均为`8cff647c7247b0a11c1625ebb445ac61c2c02a296b9a60938b933a4faced7115`；原16输入及前批622证据SHA保持。

隔离目录保留13原JSON与manual.html共14原DB文件；其中一件下划线metadata依加载策略跳过，另外新增明确实验overlay。原文件数与运行加载数分列，不混算。第一次跨进程CLI给路径而参数只接受run-label，`cross-final-1.log`在任何HTTP请求前guard拒绝；改正后实际运行成功，保`cross-final-2.log`和新run目录，不算生产错误。本批没有重复旧长历史局，也没有新LLM对局。

## 保留余项与后续顺序

- 全部projector型号、Contact/Hedgehog/配对/多次攻击、所有引信、真实重装、反雷10.5；普通8.5.4深弹pattern/250yd/深度选择完整链；核ASW专门就位和毁伤效果仍开放。未印连续速度/深度等条件继续待裁，不回退旧Pk。
- SAR的FLIR与cue/测角测距组合、夜间/云冰/非舰域、30/100人数端点、六小时后漂流与地理、目视见证损失自动marker、全部实际损失/爆炸/坠海生还入口、完整平台救起与胜利计分仍开放。未知人数或未见点不能用作者信息变为玩家航迹。
- 设施GCI的真实producer/consumer、靠泊补给的指派与库存借贷、原生建筑修理、结构DP与飞机/舰船维修服务、全部真实装配和速率仍开放。不能把旁边建筑、武器冷却、航空turnaround或§14.6概率当作本次维修绑定。

用户后续顺序保持：**全书规则完成后**先无UI并行训练引擎、Lua TCP、蒙特卡洛分析、交互式独立仿真、剧本XML明文导出、DIS外接；之后才做综合近岸高端装备想定与收益/侦察反潜/饱和/航空迂回/EW护航共享FSM。本批不提前宣称这些交付完成，也未新开LLM比赛、提交或推送。

根另维护plan、havedone、catalog与最终总manifest。本报告只签收明确子集；全部原始失败、来源缺口和未覆盖分支继续保留。
