# 本方鱼雷状态与物理管共享：来源实施账

仅适用显式 `hv2026-rts-v4`。上一包823/0/5及其快照保持冻结，本文件不将新行为虚记为上一包验收。本包全工作区56目标877通过、0失败、5忽略；本领域40项鱼雷测试及4项旧计数回归全绿（`artifacts/rules-following-20261023/validation/workspace.log`）。源码、数据与测试继续冻结，独立服务/HTTP/跨进程、旧局和来源快照由根代理收尾。全章、全书和全部线导反馈未完成。

来源流程已执行中文核心RAG“线导 数据”“鱼雷 管 装填”，随后直接视觉核对英文2026-01-01 PDF79–80/7-4–7-5与PDF110/10-1。新的检索与图片在 `artifacts/rules-following-20261023/torpedo-status/`。未改任何原始PDF。

| 规则 | Source | Data | Execution | Tests |
|---|---|---|---|---|
| R-TORP-OWN-STATUS | §7.5.4只给予线导命令；DualWire另可接收状态和目标位置。该段未规定状态报告误差、保鲜、时间及完整内容。隐蔽裁判真态不等于发射方报告 | torpedo-2026.json/own_status与独立LaunchRecord；缺报告的物理存活/在航数为unknown | observe排除v4真实Torpedo UnitObs；独立发射声明不随弹体秘密退役消失；ContactObs.own_in_flight为可空值；没有合格回报时为null；common weapons::launch即登记声明；线导回执独立于秘密cleanup，接触关联限原始ContactId | 首轮5真红及追加通用launch/秘密通道/回执/接触关联真红均已记录并修复；最终workspace鱼雷40/0、旧计数4/0通过 |
| R-TORP-OWN-TUBE-LOG | §10.1：只能发射已装武器；玩家记录初装；一管180秒重装、540秒换弹，按明确平台fit限定并行 | own_status的180/540与源Platform.tubes/reserves/reloads | UnitObs.torpedoes给真实本方初装/空管/储备/装填截止钟；源fit缺失给verified=false、null库存，不造零管 | 真实Fire后管空→真实Reload→保存恢复→截止装好，先红后绿；命令生效当tick为完整180秒，原179夹具错误已纠正，38/0通过 |
| R-TORP-SHARED-PHYSICAL-TUBE | §10.1物理装管限制与第13章潜艇布雷共用管；矿投放间隔由mines2026来源规则供给 | 显式Platform.tube_ids映射物理ID→源管索引；矿总量=管内+储备+已预留装填 | tube_ready、mine_inventory、occupy_mine_tubes统一同一库存和时钟；全部ID/载荷/线/忙碌/并行校验后原子扣现存储备；无储备不自动生雷 | 三项直接事务/360秒同钟/无储备通过；mines_dynamic的真实潜艇同管投放也通过。矿模块该目标仍有其他3项失败待其修复，不把这一条通过当作完整布雷全部验收 |

观察schema：旧规则与没有未确认鱼雷的接触仍输出整数 `own_in_flight`；旧JSON缺字段仍默认已知0。v4一旦有该接触的已发射、但无资格回报的鱼雷，值为JSON null。`own_torpedo_releases`另给已知发射声明并标明物理存活和在航数量未知，不能将声明数当作活弹数。此记录随秘密退役保留，也不通过弹体是否还在World恢复武器ID。旧profile不启用新管视图或原始鱼雷过滤。

关联只使用发射时保存的本方ContactId。真实传感器更新在旧航迹stale后会创建新ID（sensors.rs的fresh-match与next_contact逻辑），因此新C109不能凭裁判`about`与旧C99同一实体继承C99的发射承诺。平台自己的历史声明仍保存C99。旧v4存档如果只有Flight、没有原始声明，只能在本方平台账显示contact:null；不从Flight当前target反查现有接触冒充原始发射记录。第40项以真实Fire后更换本方接触号留存红证，按ContactId修复后已经全工作区40/0验收。

本方管视图只包含本方源fit、装管/储备、实际服务工作及本方发射声明。旧聚合mount ammo是另一种库存口径，绝不据其数值猜初装管数或声称“现可发射数”。无精确物理ID绑定的平台不能调用共享布雷接口；不能将两模块各自第一个管默认当作同一根管。

尚未实现与仍需明确的边界：

- DualWire允许接收状态不等于本包已实现完整反馈。当前精度、报告时钟及失锁保鲜尚未明确，故精确弹体/目标坐标和自动寻的phase均不公开；不存在用配置DualWire直接序列化Flight的捷径。
- 共同管库存的总数与矿模块carried stock必须在来源初始化中核对；任何缺映射、错装或不一致均阻断。VLS完整源初装仍另列未完成，不把鱼雷管API冒充VLS。
- 已发射但未回报的声明保留为历史未知，不假称其始终active。公共最大运行时钟推理与状态确认策略尚未补，不依据秘密fuel/alive删除声明。
- 引导通道准入旧筛选依赖真实alive的反事实已实际复现红，现改为本方未切导线占用；秘密退役不会悄悄空出通道，本条已转绿。
- 第39项真实WireOrder反事实已在`mine-wire-boundary-red.log`实际复现红（其余38绿），随后窄修：己方已获许可的线导订单独立保存/同一回执；仅在物理弹体存在时应用航向和深度，不能据secret cleanup返回不同错误。此为RTS回执语义适配：接受订单不等于报告鱼雷存活。原来源导线连接资格/断线概率未改；已在following-core-third转绿，其他命令许可路径不因此自动算全部完成。
- Wake仍按独立草案列待办：缺明确离轴/精确型号参数、尾流宽度/寿命/开局历史绑定，以及VSmall穿越和多拖曳器优先级。穿尾流/75%闭合/两项规避/穿越舰独立抗骗/Gen3–4拖曳双骰与实际consume_towed、500yd返回均不能虚记为本包完成。

上一包R049已经修复 `bda_to_shooter` 自动精确HP/DP/hits投影，并通过4项来源及旧BDA回归。剩余完整观察BDA、击沉确认和其他事件仍需逐路径审计；本包只新增鱼雷单位/在航数量/本方物理管边界。
