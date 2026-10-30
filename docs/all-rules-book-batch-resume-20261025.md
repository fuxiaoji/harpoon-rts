# 2026-10-25 全书余项批次恢复：声明子集最终验收

本批已经完成最终联合验收：Rust **1528 通过、0 失败、5 既有忽略，119 个目标**，Node **81/81**；实际 HTTP、独立进程恢复、MCP、历史兼容和双语界面验收均有最终文件。这里只签收下表明确列出的规则子集和接口范围。**整章、全书、全部可选规则、全部真实 Annex 装配仍为 false**；没有最终原子分母，不给完成百分比。测试数、表格数和目录数都不当作规则完成率。

这是线程 `01a16791-9b20-7242-b962-c352623bd7ff` 中断后继续的同一批，证据独占 `artifacts/rules-book-resume-20261025/`。先前通信 A 及其他冻结包保持原件；早期“candidate / implemented_unverified”报告和失败日志保留当时状态，由本新报告按最终运行证据签收声明域。机器可读四路记录见 [rules-book-batch-resume-all-mechanisms-20261025.json](rules-book-batch-resume-all-mechanisms-20261025.json)。

## 来源与判定口径

权威为 `14-2.HpnV_Rules_1_Jan_26.pdf`，2026-01-01 英文数字更新版，SHA256 `8196ffc715bbcccd862d8597c607de7816dac5baee31fa659f89dfaf4323700c`。三份中文核心资料负责强制 RAG 检索与交叉核对；表格亲视记录及对应原页在各来源包和本批域审计中。英文优先，原 PDF 未改。电池正文/表冲突、打印表格空隙、跨类别 MR、未知真实装配均显式保留，不填零或臆造曲线。

四路分别判定：Source 是来源及适用范围；Data 是精确字段、表格和显式 RTS 政策；Execution 是实际生产入口和消费者；Tests 是最终运行中的具体覆盖。纯函数已验只签收纯计算；拒绝待裁输入的守卫已绿只证明“不猜测”，不算待裁物理机制已实现。下表各行可共享同一测试文件，计数不能重复相加。

## 本批四路签收

| 声明域 | Source | Data | Execution | Tests 与精确边界 |
|---|---|---|---|---|
| 电池放电/充电、旧10kt政策 | 英文 PDF28–30，§3.2.1/4/5；中俄精确 Annex 行 | `battery-2026.json`、`battery-source-boundary-2026.json`；旧表15/IT与明确正文双倍20/IT不混同 | `battery2026` 实际 Tac/IT 时钟、充电条件及未知值门 | `battery_array_resume` 11、`battery_source_boundary` 5 均通过；70–71%、full-speed分母、化学、分数/表外速度未闭 |
| AIP预算 | PDF29–30，4/5/9kt源点与2000潜力示例 | 2/3/3单位/IT；实际运行秒数预留/结算是明示 RTS 政策 | 实际运行、关机/关键损伤后仍结算已用潜力、有限储量、存取 | 原生时钟和明确已裁 critical 子例通过；一般输出曲线、>=20kt单位换算、自动回港/补充仍待裁 |
| 精确 Annex overlay | 中国2025 PDF12/A-5、13/A-6；俄罗斯2025 PDF23/A-20 | `submarine-annex-2025.json` 保留11精确深度/浮航ID；电池事实只给 `cn_039a/cn_kilo/ru_kilo/ru_kilo_636` | `submarine_annex::capability` 与明确运行配置 | provenance/电池边界通过；不是所有艇容量/初值/锂电/通气管/储量已知 |
| 通气管与实际操纵 | PDF28/3-6、29/3-7 | 明示已装与25m/15kt源门 | Raise原子切柴油、潜航Retract停充并切电池；一般 movement 和导航共用限制 | 实际发令、移动上限、深度/侧别拒绝通过；不自动造桅杆或电耗尽停车规则 |
| 可选拖阵 | PDF48/5-9，§5.4.2.1；K1只给短/长类别 | `towed-array-optional-2026.json` 保原5/10/15+列；绕实际航向轴镜像是 RTS 几何表达 | 真部署、原生探测/报告、45–135°实际操纵、重获、稳定与后续kink，融合/FCS不恢复被抑制坐标 | 真实21分钟streaming及完整World续演通过；表外速度/未知阵长不插值，实验声纳不冒认真装备 |
| TL一次+15 | PDF74–75，§6.3.9/10 | `communications-2026.json` 一次15百分点机会 | `alerts` 消费真实同一个接收声纳试验，不叠加/重掷/追改已完成epoch | 原29全部通过；失败机会、晚到、重复、普通语音无TL均保留；实验1.25nm等距待裁未放开，阈值夹具改1.4nm |
| NRT/RT与AAW反应 | PDF71–75、84/8-1、99/8-19 | NRT+1/RT+2；本机ES+1；超出2..12末行截顶是明示RTS政策 | `cue_evidence/operations::reaction_with_cues` 消费原2D6并记修正/来源；共享被动ES不是主动AAW提示 | `communications_shared_consumers_resume` 11/11：真实scan、发送、原骰不重掷、表边界/封顶；发送源关雷达失cue，旧收件保留，开机新scan/新warning恢复 |
| 既有FCS与非TDL | PDF72、74–75 | RT同质量；NRT Good→Fair、Fair→Poor；两独立被动NA/主动Poor | 同一选定原生sensor形成quality/aim；同epoch自动warning不吃掉后来FCS；remote Err不否决本机Good | resume8、原29、shared11全过；原生Poor/Good由真实4kt转向/运动窗产生；NRT Poor再降和混合模式/家族仍pending，Air独立producer未闭 |
| 同批检测与ESM入口 | PDF45/5-6、74–75 | `Configuration.emissions` 明确source/sensor/role/LPI/range/height，不接受author emitting/success | `initialize_emissions` 原子核唯一unit及实际安装Radar；实际雷达开关/供电/损伤，每次原生ES重查；close在全部原报告/FCS后 | shared09非法load/后序坏行原子拒绝、shared10实际ES开关均过；修正仅用于声纳的active位误用，未改原位值。实验连续targeting模式不冒认真实F18参数或自动AI引导producer |
| 无人控制/交接 | PDF21–22，§2.5 | `unmanned-2026.json` 精确控制元件、HITL1/自主3、Tac交接/IT自主 | 物理控制与高级AI权分开；反应火力限制、丢链fallback、耗油、独立cause恢复 | 原29与resume8过；干扰/部分critical输入明确是裁判已裁合同，不称天然EW全因果 |
| UUV基础数据/无线电 | 同上，§2.5.2 | 基础调制解调单接触/位置/航令，不承载完整任务 | 真实运动到P/S、升桅、Tac建链；实际深度命令、原召回骰/信号 | 控制容量、模态/召回原骰、位置与完整状态区别通过；所有真实控制台/链路/卫星带宽fit未闭 |
| 隐蔽损失与遥测 | §2.5、§14.3/5；明示裁判真态/本方接收合同 | 持久隐蔽损失账、known roster、最后完整遥测 | 实际Mk48伤害→源沉没钟→真实World计分；owner伤情/死亡事件及ongoing零值kind键过滤，明确后收讣告只公开一次 | R05等实际通过，含完整World恢复；不注HP/死亡，不把显式失链触发称天然EW命中 |
| 扩展列/精确组件 | 英文 PDF129、134–137；中文扩展 PDF2–15 | `expanded-damage-2026.json` 13列每D100一行；英文“先选实物，再独立D10指挥仪”覆盖中文同D100细分 | 实物炮架/弹药/指挥仪/传感器/货物、CaS、源identity幂等与变源拒绝 | `expanded_damage_20261025` 20/20；列覆盖不是全部E00–E13结果/并发/真实fit完成；native primary与已裁source_critical分别标明 |
| 失电本地手动炮 | PDF70/6-2、134–135/14-8–9 | 精确local+manual+Gun/Ciws才属失电例外 | Full平台/每武器门保alive/massive/doomed/personnel/损管组硬门；实际manual surface gun与AssignTarget scan | `local_manual_power_resume` 先2真实红后4/4绿；真实工程电力骰/原生目视。实验30mm不冒认Ford手动实装，AAW directed local/NoLocal仍独立待源 |
| 可选飞机损伤消费者 | PDF137/14-11 §14.9；章4/9源选择器 | 实际损伤级/MR/载荷/重复伤组合，原打印格 | `aircraft_hit`→航空速度/爬降/节流、AAM/格斗、传感器/进攻/对抗/抛载 | expanded20+消费者9全过；真正格斗正例为J15炮2.5→F35C DP25源格；F18炮4.2/29DP、MR2.25垂直档保持拒绝。decoy损伤个例明确已裁输入 |
| 战后修理 | PDF136/14-10 §14.6 | `repairs-2026.json` 精确前向记录系统/层级、到期快照/原骰 | 6/12/24/48h、手动/自动同一结果、失败48h海上cap不被重开/再伤清掉 | `repairs_2026` 12/12；37%例18成功19失败、再伤/存取均验；不从旧aggregate重建critical，功能修复不补HP/弹药/货物/飞机 |
| 可用率/SAR合同 | PDF136–137 §14.7–9 | 可选开关、每日幂等、私有幸存者与本方别名 | 已有serviceability与明确sighting合同输入 | 相关expanded例通过；**原生视觉/IR发现幸存者仍无生产调用方**，战役50/50库存/停机个体原生转移未闭 |
| 核载体/引信 | PDF131–132/14-5–6 §14.1.8 | `nuclear-2026.json` 离散当量/半径、明确核变体 | 授权/释放快照、真实终端状态武装、miss延迟爆炸、blue-out；实际SAM核引信失败不落常规击杀 | `nuclear_batch_resume` 7/7；公开标注终端输入实验不称完整天然核FCS战斗；核ASW专门placement、未印当量不猜 |
| 核停机/飞行/mission abort | PDF131表注1/2与132正文 | 停机须真实同方facility stored_aircraft绑定 | 8psi地面/甲板与4psi飞行分开；2psi missionabort由实际航空/武器消费者执行，不给停机机体套舰D6×10%DP | `nuclear_parked_resume` 4/4；停机、飞行机/导弹反例通过；2psi具体weapon/sensor细伤、设施DP、EMP/沉降仍pending |
| 设施/岸基航空 | PDF116–118/11-1–3；表亲视117/118 | `ground-2026.json` 明确设施/资源/跑道/储机绑定 | 功能损伤、实际岸基起飞/跑道队列/间隔、修理资源与不可修rubble、存取/own投影 | `ground_2026` 8/8、storage1/1；GCI/补给无生产消费者，repair_rate_factor仅字段/校验，不能算完整机场港口 |
| Stand-off反潜 | PDF98–100/8-18–20；美2025 PDF82/E-1、中2025 PDF75/E-1 | `asw-weapons-2026.json` 明确载具/载荷/范围；无默认live fit | 原生passive分类Sub+独立active测距FCS→实际载体→一次就位D10→同ID鱼雷/同tick初攻、源潜射噪声6 | `asw_weapons_resume` 8/8，其中4纯计算、4原生/拒绝边界。最终水面用已有CS映射Ford+实验VLA，潜射用Virginia+实验fit；**不称真实Burke CS、Ford/Virginia VLA实装** |
| Projector/深弹 | 同上§8.5.3/4及E3/E4 | 源pattern阈值、Fair/Poor/海7/速度/深度/整pattern单骰/舍入 | 只签收 `projector_thresholds` 纯计算，未接物理发射 | ASW8中2项projector纯表测试通过；**8.5.3/4实际pattern/耗弹/弧/深度/FCS/重装/250yd链仍开放** |
| 公共合同/人工权/UI | 上述各机制来源；共享实现合同 | 完整typed shape、四个电池空struct变体、公开schema/双语帮助 | 共同来源/侧别/引用所有权、内部retain_manual、实际成功apply才抢权；modem/显式恢复AI共用门 | server5/MCP4新增合同包含在最终整库；HTTP/MCP/UI实际只签收下节列出的控制命令，不扩为所有发现动作或核/FCS实投 |

## 最终统一验证证据

| 检查 | 实际结果 | 最终文件 |
|---|---|---|
| Rust / Node | 119目标共1528/0/5；Node81/0。最终build仍有一条既有unused_mut警告，不声称零警告 | `final-workspace-1.log`、`final-node-1.log`、`final-build-1.log` |
| 冻结输入 | 330项before/after清单逐字节相同，SHA256 `cfd0640cc2f337b0dd92f76cf5d1947c5670cb72c38c5f773adcbcff8cbc1eab`；14原DB文件+2旧表共16项全部不变 | `final-candidate-1-inputs.json`、`after-acceptance-inputs.json`、`protected-inputs-final.json` |
| 实际HTTP | PID64740；7非法请求完整payload不变；11合法命令暂停仅排队、推进后实际生效；T202完整重演12batches/174events，`89ff6e355a6738d0` | `acceptance/runs/http-final-1/validation.json` |
| 独立进程 | 同SHA的PID51356，从T1/T7/T9含inbox检查点恢复，按明确调度续至T202，完整World/inbox及原命令重演相同 | `acceptance/runs/cross-final-1/validation.json` |
| 实际MCP stdio | PID65696；15非法均不排队；11操作组共12合法命令推进生效至T207，含battery raise显式automation-off+声明两条；其他合法apply才手控，末授权撤销、弹药不变 | `acceptance/mcp-runs/mcp-final-1/validation.json` |
| 历史兼容 | 六旧v4源版本明确拒读且当前完整payload不变；原v3 T299/7events `ab2ffcb9c57e6195`，续T329/15events `933df72aebd01cdd`；原Sol10800秒360批/7505events `22b2238739248f90`；旧原件SHA保持 | `acceptance/history-runs/history-final-1/validation.json` |
| 实际CUA界面 | 中/英1600×1000、1024×768；6合法请求、2拒绝；选择和当量草稿不写完整payload；观测页面错误0、弹药不变、末核授权false；T20818批/186events `7e2be22c5a4782a1` | `acceptance/ui/final-validation.json` 及其selection/yield/invalid-charge/snorkel子记录 |

所有表中最终文件以本报告证据根 `artifacts/rules-book-resume-20261025/` 为前缀。冻结server SHA256为 `c608303d35d4fa559c508504c8d568dbcd3b08030b6e4c4debdd93f1562c592c`，MCP为 `71c76d9711ea8bd7b46652c4779447efc4c2db3ee57ea9979e8055b2796d22d1`，与各实际进程回执一致。没有把准备脚本的static状态冒作运行结果。

HTTP实际命令为升通气管、开始/停止充电、AIP开、收桅、设当量/授权、无人自主/申请HITL、TDL自动开/手动警告。MCP实际覆盖电池升管/充停、设当量/授权/撤销、显式恢复AI、无人自主/实际180秒HITL交接、TDL自动开关；其验证范围明确排除核实投、FCS和完整手动警告。界面实际六请求为升管、撤销授权、设1kt、自动共享关、原生警告、自主模式，另拒绝active-AIP充电和不支持的SAM FCS。选择/草稿变化本身未发游戏命令。

CUA截图已经在根对话内实际展示，不声称存在本地截图文件；1024×768是受测视口，**没有实体平板验收**。没有新LLM比赛、真实DIS/AFSIM/STK连接或六项新接口完成声明。

## 失败与纠错证据保留

早期 `unified-batch-1.log`、`source-integration-2/3/4.log`、`local-manual-source-red.log`、`asw-native-5/6.log` 等保留真实失败和夹具诊断。parking字段缺失、私有API或事件字段误用是夹具/编译问题，不算规则红；声纳50%等距行仍正确source-pending。失电炮实际消费者、隐蔽损失零kind键、原雷达被误当sonar-only active位等实际失败修后保原断言。没有以削弱期望、捏造报告/骰、编造Burke CS、删除原ASW自然链或改冻结旧表取得绿。

## 仍须继续的机制

下表据本批独立 `coverage-audit/coverage-delta.md` 与最终代码复核更新。早期“本批未验”已由上面的最终证据替代，来源缺口和未接物理消费者不随整库绿自动升级。这是最小确认余项集合，非完整原子分母。

| 范围 | 状态 | 精确余项 |
|---|---|---|
| 章1–2 | execution_open | 多编队完整阶段/多速率、量纲/逐原子适用性四联 |
| 2.5 | source/execution_open | 全真实控制台/信道/LOS/SAT/调制解调/耐力，全部天然EW/critical失效组合 |
| 3.1 | execution_open | 碰撞/撞击实际几何、3D10×原始DP/方面角/舍入、搁浅/海图水深 |
| 3.2 | source/execution_open | CrashDive实际Annex许可、VeryDeep/低速、潜艇硬舵失深/工程crush/并发伤，Blow空化R-063 |
| 3.2.4–5 | source_blocked | old10明示政策仍需选；70–71%、full-speed分母/化学、分数/表外速度、一般AIP曲线/初值和>=20kt单位贡献 |
| 章4 | source/execution_open | NOE/TF/TA实际地形AGL/云、全部外载/岸勤；4.2 60%分母与4.7.5不混同；MR2.25/未印gun-DP格 |
| 章5/可选拖阵 | source/execution_open | 广域MAD、自然尾迹三越、浮标通道/真实深度设备、完整激光告警；拖阵表空隙/真K1长度fit |
| 6.3.9/10 | source/execution_open | 真实Annex L/HF-ES receiver、Air targeting producer、NRT Poor降级、混合非TDL复算 |
| 6.4 | source/execution_open | 真实跟踪/导弹/线导容量；Cbt Sys Gen不自动成为容量 |
| 章7 | source/execution_open | PRH Gen3、全线导双线反馈/垂直/多尾流圈、3+同型/异深互扰与未给声学优先级 |
| 章8 | source/execution_open | 舰炮/8.3对陆全几何/集中/组件分配，AA directed local模式，BMD/ASBM；R-059目标导弹MR适用仍未决，Annex ATA不是MR |
| 8.5.3/4 | execution_open | projector/深弹真实pattern、耗弹/弧/深度/FCS/重装/250yd链；纯阈值测试不算runtime |
| 章9 | execution_open | 全投放/扫射/真实载荷/出动→原生命中→设施完整因果 |
| 章10 | source/execution_open | 多对象/全线导反馈/垂直捕获/硬杀伤引信；未给交会杀伤和声学优先级不得填值 |
| 章11 | execution_open | GCI与补给helper无生产调用；repair_rate_factor只字段/校验；完整机场港口补给/维修 |
| 章12 | source/execution_open | 箔条走廊体积/风/事故、TALD全性能、真实EW装配及天然组合 |
| 章13 | source/execution_open | Type193M精确设备、echo→分类→hunt、完整布雷/漂移/扫雷/引信反应目标组合 |
| 14.1.8 | source/execution_open | 2psi设备细伤、未合格burst/设施核DP、EMP/沉降、核ASW专门就位效果 |
| 14.2–9 | source/execution_open | 所有结果/并发/真fit；cargo自爆本舰DP、crush、原始modifiedPh/停机个体原生转移/战役50-50库存 |
| 14.8 | execution_open | 幸存者视觉/IR原生producer；sighting API及显式测试不是自然发现 |
| 所有真实Annex | source/execution_open | 全平台/设备/年代/弹药/载荷；catalog/原页/候选不能代替运行fit |

用户后续优先序仍是：**全书规则完成后**，先无UI并行训练引擎、Lua TCP、蒙特卡洛分析、交互式独立仿真、剧本XML明文导出、DIS对接；之后才做综合近岸高端装备想定及期望收益/饱和/侦察反潜/航空迂回/EW护航共享FSM。本批不提前把这些后续交付记完成。根另维护plan、havedone、catalog；本文不覆盖旧账或旧manifest。

