# 第12章电子战逐机制录入台账（2026-10-23）

**全章尚未实现。** 英文2026-01-01整章三页PDF119–121已来源视觉复核与结构化转录 `data/rules/ew-2026.json`。续包现已把明确源安装的部署对象接入显式v4命令、库存、寿命、真实扫描与完整World；首编译EW13例为11通过/2失败，两项修复后在 `artifacts/rules-next-20261023/validation/second-core-run.log` 本目标13例全绿；同日志其他目标有失败，不记为全工作区验收。既有噪声干扰仍按 `electronic-warfare.json` 的另行核验子集执行。执行边界见 `ew-execution-20261023.md`。

先检索中文核心RAG“噪声干扰”“水声对抗措施”，命中主规则PDF119–120及玩家表PDF10，再回看英文原页。来源文件SHA256为 `8196ffc715bbcccd862d8597c607de7816dac5baee31fa659f89dfaf4323700c`，渲染及核对清单在根目录 `artifacts/rules-all-20261023/ew/`。原件未改。

下表分母按章内机制而非代码函数数目划分；每行的Source/Data/Execution/Tests独立记录。现有代码并不证明完整附件型号适配。

| ID | 来源 | 机制 | Source | Data | Execution / Tests |
|---|---|---|---|---|---|
| EW-01 | 12.1 / PDF119 | 机载雷达防御ECM与红外对抗按制导类型适用 | verified_source | structured | 既有导弹路径有局部防御；全部载荷/世代/重量与独立防攻组合未验 |
| EW-02 | 12.1 / PDF119 | 进攻噪声干扰与防御ECM并存 | verified_source | structured | 既有噪声干扰与导弹ECM分离，完整组合源案例未验 |
| EW-03 | 12.2 / PDF119 | 箔条走廊容量、0.25海里宽、飞行消耗、所在高度遮雷达 | verified_source | structured | not_implemented；没有以自卫箔条替代专用走廊 |
| EW-04 | 12.2 / PDF119 | 箔条每3回合降一高度带、每10分钟随风移动、Low覆盖VLow | verified_source | structured | not_implemented；缺局部体积与风场生命周期 |
| EW-05 | 12.2 / PDF119 | 普通飞机穿云超过2回合10%发动机事故、施放机例外 | verified_source | structured / trial cadence TBD | blocked；原文未明多次检定频率，不能每秒重掷 |
| EW-06 | 12.3 / PDF119 | 远程箔条5海里、15分钟、部署时50%小/中型回波 | verified_source | structured | tested_subset；明确源安装/库存、5海里门、唯一回波骰、15分钟和真雷达假回波已接；6.3.2未完成 |
| EW-07 | 12.3 / PDF119 | 已锁定导弹不被远程箔条转移，未锁定/主动指向可攻击 | verified_source | structured | not_implemented；缺有身份的诱饵实体与导引状态链 |
| EW-08 | 12.4 / PDF119 | 浮动反舰诱饵1分钟部署、最多3小时、可预沉 | verified_source | structured | tested_subset；部署、准确寿命、真实回波与主动沉没已接；安装数量必须明确来源 |
| EW-09 | 12.4 / PDF119 | 浮动诱饵海况>4即沉、半风速漂移、拖速最多10节 | verified_source | structured | tested_subset；明确风场下漂移及海况淹没已接；拖曳缆绳/10节门仍未完成 |
| EW-10 | 12.5 / PDF119–120 | 队外/护航/点干扰、点干扰ES/RWR先探测、同雷达不叠加 | verified_source | structured | executable_subset；electronic_warfare::configure/range_modifier_for，只有已核两型飞机 |
| EW-11 | 12.5 / PDF120 | 六代主瓣/旁瓣全表、扣抗杂波、旁瓣净效应减半下取整 | verified_source | structured | existing_tested_subset；既有Su-24MP例与两部雷达/旁瓣测试，尚非全组合/全平台验收 |
| EW-12 | 12.5 / PDF119 | 噪声干扰作为辐射雷达被ES接收并识别为噪声 | verified_source | structured | partial；旧扫描有辐射门，v4仍需明确噪声发射源绑定/可见信息，不能自动公开干扰机身份 |
| EW-13 | 12.6.1 / PDF120 | 鱼雷ACM与声纳干扰作用域分离、内置/小外置2回合、大外置最多7回合 | verified_source | structured | tested_subset；真实世代/库存/寿命与作用域已接，active_acms由10.2.5消费；大外置无准确来源寿命即阻断 |
| EW-14 | 12.6.1 / PDF120 | 声纳干扰10度锥、Loud噪声，拖曳诱饵无限寿命但成功诱骗后被毁 | verified_source | structured | tested_subset；真实Loud扫描/干扰锥、拖曳无限寿命和消耗接口已接；移动外置干扰器缺航速资料仍阻断 |
| EW-15 | 12.6.2 / PDF121 | 4代机动潜艇诱饵寿命/最大速度/MAD/主动回波全表、3/4代2次变航变速 | verified_source | structured | tested_subset；4代最大速度/寿命及实际运动/代际回波、3/4代两次转向已接；低速能耗与变速、低代被动接触切主动后立即识破仍未完成 |
| EW-16 | 12.6.2 / PDF120–121 | 诱饵继承潜艇声纹、鱼雷脱靶再攻时可获取诱饵、影响火控 | verified_source | structured / references | executable_subset；声纹继承、被动分类和10.2.5接口已接；实际机动诱饵次窗分类、完整再获取/火控尚未验 |
| EW-17 | 12.7 / PDF121 | TALD预设航迹、250–500节、挂载/重量/三联架、射程原例 | verified_source | structured | not_implemented；只保存原书离散例，不外推滑翔包线 |
| EW-18 | 12.7 / PDF121 | RF/箔条TALD及动力诱饵、40次箔条/1海里线、各航点变航变高度 | verified_source | structured | not_implemented；缺完整型号附件/载荷与飞行动力学 |
| EW-19 | 12-3 sidebar / PDF121 | Suter攻击由想定设计具体规则 | verified_source | scenario_specific | not_enabled；无书中通用数值，不虚构自动网络攻击效果 |

需要独立裁定：EW-Q01，箔条走廊发动机事故原文仅说明停留超过两战术回合后有10%机会，未明持续停留时检定次数；保留 `failure_trial_cadence: null`。EW-Q02，机动诱饵寿命标为“最大速度下小时数”，没有其他速度的耗能公式；不按速度比例延长寿命。TALD原例为11000米/250节/68海里及7000米/400节/25海里，未将原书例子误读成1100米，也未从两点插值整个滑翔包线。

本轮探测执行补丁在 `detection-all-mechanisms-20261023.md` 单独验收；本台账不借用探测测试数给本章报完成。新EW生命周期对象独立保存在裁判Runtime，仅扫描期间投射目标，不成为战斗舰潜/本方单位/击杀分。初次EW日志为 `artifacts/rules-next-20261023/validation/first-core.log`；本目标13例已在 `second-core-run.log` 全绿；拖曳销毁仅接口验证，尚无真实武器成功诱骗调用。旧747次批次独立冻结，旧局及运行服务保持其既有语义。
