# 探测与电子战续作交接（2026-10-23）

后续执行包进度：以下51例/747次描述是已冻结上一包。新日志位于 `artifacts/rules-next-20261023/validation/`，EW13例已在 `second-core-run.log` 全绿，执行边界见 `ew-execution-20261023.md`。探测新增真实潜射 `WeaponsRelease` 显示名与 `ProjectileData.weapon` 稳定ID关联、歧义待裁阻断、NAV源加减速噪声与v4独立飞机variant读取，55例首轮53通过/2夹具失败，修正明确发射深度/航速及合法NAV最高速度后，`third-core.log` 55例全绿。同日志其他目标仍有失败，不能记为联合验收；新状态不追写为原747包已验证内容。后续本机证据与几何修复已在 `fifth-core.log` 探测61例全绿，仍独立于原747批次。

潜射身份修复先红证据是 `actual-launch-red.log` 的普通鱼雷真实launch：事件写显示名、旧消费函数按ID找表，导致应有 `(180,3)` 瞬态缺失。同日志另一例先断在测试读取错误的trace路径（实际为 `trace.inputs.target_modifier`），因此不计为第二个生产错误。生产launch现在在emit后由唯一hook从真实投射体取稳定ID，关联event_seq保存；扫描按来源表Noisy/Quiet/Loud持续180秒，不公开全图反探测。重复显示名而无映射的旧事件显式待裁并阻断该目标被动噪声裁决直至期限结束；成功映射、投射体先被移除、完整World恢复和歧义期限的案例已在上述53通过中验证。

导航联动来源为核心RAG加英文PDF24/印3-2、PDF55/印5-16视觉：超过静航50%加减速值采用空化被动修正+4。只读取由明确NAV来源fit的真实movement产生的marker，与常规速度/深度空化只加一次；不改变主动回波。`data/rules/detection-2026.json`增加来源瞬态索引、180秒及+4数据，参与新v4指纹。

本机传感器成功证据已在 `fourth-core.log` 的探测56例与AAM12例通过；后续发现真实 `world_solution` 仍从一方融合Contact借位置/运动，两条实际长时钟失败在 `fcs-own-motion-red.log`。当前已把逐接收器报告几何、同一角误差投影与180秒自身运动状态接入，`fifth-core.log` 探测61例全绿；详见 `fcs-own-reports-20261023.md`。这项解算限制不额外抬高夜战AAM的本机成功接收门槛。

本轮只对显式 `hv2026-rts-v4` 扩展探测执行。Legacy/v2/v3保持原入口。第5章与第12章均未达到整章完成门；附件全型号适配、无方位AIR告警、浮标/尾流完整生命周期等仍在各章台账中保留。

已实现并在最终全工作区日志 `artifacts/rules-all-20261023/master/continuation-workspace-final.log` 验证通过的新增路径，探测目标51例全部通过：

- §5.4.3 `DetectionAction::Dip` 通过既有 `Command::Detection` → `sim::tick` → `update_sensors_v4`。验证明确机体能力、悬停直升机/落水水上飞机及静止源安装舰艇；即时可用，同回合可收回。部署后的横移永久丢失缆绳，位置与丢失状态保存到完整World，保存恢复和同命令重演一致。
- §5.4.5 AIR使用实际ArrayJob深度。自身扫描、AIR与被动遮蔽共用 `deployed_sonar_depth`，收回/丢失/损坏/失电的阵列不会继续假发射。非直接传播AIR只能提供无方位告警，目前明确阻断公开接触，不捏造方位。
- §5.4.6.6 主动回波不再进入被动窄带舰级分类；必须有上一战术窗口保留的被动探测证据。收到主动鱼雷导引头的AIR可自动知道是鱼雷，仍不获得未核型号。

历史联合日志 `continuation-domains-3.log` 为50通过/1失败，唯一失败是本轮新增的Gen5 AIR方位精度回归。更早的 `continuation-red-detection.log` 是44通过/2失败，复现主动声纳错误获舰级与AIR未使用ArrayJob深度。修复后最终日志 `continuation-workspace-final.log` 中探测51通过/0失败，保留这些红测记录说明修复过程。

第51例已在最终绿测通过：Gen5 AIR在12海里内能测距，仍保留±3度方位精度；公开估计点用报告方位与已测距离组合，不能直接取真坐标。新Detection事件类别仅来自实际分类证据，测距本身不能公开宿主真类别。该例同时核对方位误差、保留距离、事件unknown及Observation.category=null。

变更文件：`crates/harpoon-core/src/rules/detection2026.rs`、`src/sensors.rs`的v4分支、`tests/detection_2026.rs`、`data/rules/detection-2026.json`及探测台账。源表补§5.4.3即时部署/零水平速度；规则指纹由现有v4总表计算纳入。新增 `Configuration.dipping_platforms` 为明确机体来源能力；测试中安装在作者夹具上的声纳不代表真实福特号存在此装备。

电子战：第12章PDF119–121全章三页视觉复核，结构表 `data/rules/ew-2026.json`，19项Source/Data/Execution/Tests对账在 `ew-all-mechanisms-20261023.md/.json`，证据 `artifacts/rules-all-20261023/ew/manifest.json`。**此新表仅来源结构转录，没有接入新自动裁决。** 现有 `electronic_warfare` 只保留另行核验的12.5噪声干扰子集；箔条走廊、远程箔条、浮动诱饵、水声干扰、机动潜艇诱饵、TALD等均按未完成列示。

主代理需在根open_questions保留EW-Q01（箔条走廊停留超过2战术回合后的10%发动机事故检定频率不明确）、EW-Q02（机动诱饵仅给最大速度寿命，无其他速度耗能公式）。TALD来源例已放大复核：11000米/250节/68海里及7000米/400节/25海里，不是1100米，也没有据两点插值滑翔包线。

最终全工作区all-targets合计747次通过、0失败、5忽略，其中探测51例全绿；次数含工作区重复目标及既有测试，不是规则数或整章完成率。HTTP、UI、跨进程存读档/历史档保持由主代理独立记录；本交接不把现有单元/完整World案例冒称真实HTTP或UI已验。

原51例后的适配缺口续接：`Receiver::Sonar`仍是单一频带，多频被动声纳选择最佳频带未实现；主动直接路径错误依赖目标被动noise字段已在新EW13例中先红后修并验绿，不追记成原51例成果。完整浮标无线监听/频道/寿命、真实阵列国别深度、单机仅方位长期定位和明确TDL/交叉定位继续保留为未完成。
