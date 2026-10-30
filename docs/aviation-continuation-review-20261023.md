# 航空续接验收，2026-10-23

本次先完整阅读根协作记录和仓库规则 Skill，检索核心中文 RAG 的悬停、空空导弹与空战，再回看英文 2026 PDF33、102、105 及美国航空附件 PDF106/H4-2 原页图像。前一聊天仍运行的代理交接后才开始写入航空文件；不覆盖历史服务或旧规则。

| 规则 | Source / Data | 实际执行 | 来源回归 |
|---|---|---|---|
| §4.2/4.3 ASW 悬停 | PDF33/印刷4-1；既有推进/油耗来源表 | ASW 到站通过正式 Hover 油门保持位置，并按 FMP 时间耗油；出站或换任务恢复 Cruise。v4 不直接瞬移到30米。 | `aviation_2026::real_sim_asw_hover_is_stationary_and_burns_full_power_then_resumes_cruise`，包括真实60秒悬停、2.5海里预算、完整World保存续演、改任务恢复巡航。 |
| §9.5.1 持久失导 | PDF105/印刷9-4；`aam-2026.json`程序说明 | 非 TARH 包在其控制飞机规避时保存失导标志；来袭先消失也不会恢复制导；TARH 例外保留。 | `aam_2026::source_non_tarh_controller_evasion_is_permanent_after_threat_passes`，含中途World序列化。 |
| §9.4/9.5.1 缠斗速度与外场规避 | PDF102、105/印刷9-1、9-4 | 缠斗目标速度修正取来源巡航速度；缠斗内不启用仅限场外的来袭规避禁射。 | `aam_2026::source_dogfight_aam_uses_cruise_speed_and_does_not_trigger_outside_evasion`。 |
| §9.1/9.5.1 外挂投弃 | PDF102、105；附件H4-2来源挂载重量；`aviation-2026.json` RTS-04-26 | `Command::Aviation` 的 `jettison`动作扣实际弹库、显式外挂数量和当前载荷，MR/后续油耗立即消费新载荷；没有生成攻击弹体。 | `aviation_2026::real_sim_external_jettison_changes_maneuver_without_spawning_an_attack`。 |
| §4.7.5 实际投放减载 | 同上；源质量与想定显式装配分列 | 成功物理发射时调用 `aviation2026::ordnance_released`；未声明外部的武器不会自动减载或被当作外挂。 | `aviation_2026::actual_release_deducts_only_explicit_source_external_store_mass`。 |
| §11 岸基航空接入 | `ground2026`来源与想定声明，不采用舰载弹射时长 | Base 的起飞队列、预约库存、跑道适用性与回收转场时间接地面模块；舰载路径继续原规则版本。 | 联合 `ground_2026` 目标；完整地面覆盖由主代理另行对账。 |

前三个失败已由主代理先实际复现：悬停60秒发生约0.47海里漂移；短时威胁消失后原应失导导弹仍有0.5命中概率；缠斗AAM取600节实际速度而非485节来源巡航速度。修复前证据在 `artifacts/rules-all-20261023/master/continuation-red-hover.log` 与 `continuation-red-aam-source.log`。

原 AAM 双弹测试的第二次命令先遇到空弹库，其实符合校验顺序。现同时验证该挂架确实为空，再用另一尚有弹的挂架验证同机一分钟包限制。真实sim调用补全命令参数，保存续演的箔条检查改为比较两个World，不再使用自等断言。

双弹案例的指数也按来源重新分解：AIM-120D的ATA4.0减轻载J-15的MR4.5，加第6代AI雷达3.0，600节速度/Medium签名/第5代导引头对第2代诱饵各为0，得到2.5，原表双弹80%。旧断言4.0/91%并非该夹具的来源结果。F/A-18E在附件B-44为Small，不能按Medium测试跑道门：2000米跑道4次切断仍允许Small，8次为X；测试现分别验允许与禁止。

最终主代理统一执行 `--workspace --all-targets`：747通过、0失败、5忽略，其中航空20、空战8、AAM9、地面8全部通过。证据为 `artifacts/rules-all-20261023/master/continuation-workspace-final.log`。这些是测试执行数，包含共享工作区其他目标、源码检验和真实sim案例，不能计成已完成的原书规则条数。`continuation-domains-1.log`、`continuation-domains-2.log`、`continuation-domains-3.log`保留各轮失败与阶段结果，均不能替代最终全绿日志。尤其空战第8项航空鱼雷测试只调用辅助函数，生产武器循环尚未消费其门限/投放/互扰三个接口，故9.7仍不具备完整Execution，不能因该测试通过改变其未打通状态。

第四章原审计JSON保留23个章内工程组：9部分实现、8已验子集、4阻断、1仅结构化、1待办。新增窄机制不把这些组自动升级为完整规则；第9章单列文档同样保留AAM、多目标、反潜投放与可选惊袭的边界。

来源AAM事件的包ID已改为确定性不透明散列，失导事件只报告本方包数量，不将隐藏目标的真实实体ID拼进可见文本。权威World仍保存完整目标和裁决，供裁判重演。

外挂输入使用 `Loadout.external_weapon_rounds`（精确武器ID到数量）。当前只支持某武器全数外部的明示装配，初始声明须与来源平台的该型号总弹数一致，且总载荷覆盖已知外挂与油箱重量。混合内置/外置的同型武器、油箱投弃、未知挂载质量不猜填；部分耗尽的空中初态需后续逐挂架schema。普通命令不能声明或更改挂载质量。

本文件记录的是明确子集。航空全章、所有附件、NOE/TF/TA、高程、完整步进转向、外部60%音速门、水面飞机、空中加油全部机型和9.4.8夜间本机雷达/IRST追踪条件仍需继续。特别是非AAM的空对地受控武器也需消费永久失导状态，不能把当前AAM包修复写成所有制导武器均完成。全工作区、历史档和前端验收由主代理统一登记。

只读复核地面投影：观察入口只在本方单位迭代内调用，发射队列中的任务使用本方ContactId，炮击校射目标转换为本方ContactId，未直接暴露敌方实体ID。World保存队列、预约库存、每条作业通道和航空机体状态。另发现同跑道多架起飞在长期阻断后可能同秒恢复的节奏问题，独立真实sim用正式修理命令先复现T401实际放2架而应1架，红证据在 `artifacts/rules-all-20261023/master/continuation-red-runway-lanes.log`。主代理随后增加实际放飞通道时钟并保存；最终 `blocked_same_runway_launches_resume_with_actual_lane_spacing_and_save` 已在工作区日志通过，验证T401只放1架、T461第二架，以及两架之间存档续演完整World一致。

以上747批记录保留当时边界，后续新实验没有回写旧冻结程序。其后的9.7.1真实投放、9.6精确靶场子集、R-048夜间本机证据分别见`airborne-asw-surface-production-20261023.md`与`aam-night-onboard-production-20261023.md`，新批`fourth-core.log`的AAM12/Bomb7对应目标已通过。独立只读审计仍列明物性许可、加油alias、Side声明点与反潜挂重缺口，见`aviation-post-green-readonly-audit-20261023.md`；这些不因局部绿测试升级为全章完成。
