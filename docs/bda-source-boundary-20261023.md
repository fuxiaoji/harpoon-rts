# R-049 未观察战损的v4投影边界

权威依据为English Jan2026 PDF75/印刷6-7的Kill Chain/Assess说明：评估攻击是否命中及可能损伤，需要回到平台传感器取得更多资料，效果往往难以确定；PDF69/6-1列出Find、Fix、Track、Target、Engage与Assess分别的环节。先按Skill检索三本中文核心资料的战损/评估/裁判，命中`core_rules_cn:p075:c02`，再视觉核验英文75页，证据为`artifacts/rules-next-20261023/bda-source/english-p075.png`。本项目`agent.md`要求裁判保管隐藏信息、玩家与智能体只看己方observation；`world.rs::SideState.bda`和`observe.rs`亦明示该字段只来自实际观察。

旧`damage::bda_to_shooter`却只检查接触未过期且quality>0.25，就累加真实命中数、真实DP、`hp_left/hp_max`，并给攻击方复制含精确damage与hp_left的Impact或BaseDamaged。被动声纳已定位、型号已知或接触quality很高，均不等于观察到了后续爆炸或精确耐久。这是投影边界错误，不以新BDA骰子或估算HP公式替代。

本修正仅针对显式v4：缺少已实现且有来源的观察→评估程序时，阻断这条自动敌方精确BDA投影。受击方继续看到自身精确伤害与OwnHP，裁判仍持有完整实际结果和`R-049.unverified_enemy_bda_withheld`的Referee-only策略记录；攻击方的接触簿与quality不自动转成伤害观察。具体策略、来源和限制独立存于`data/rules/bda-2026.json`，由`damage::bda_tables()`读取，只纳入v4指纹。旧`damage-runtime-v2.json`、`operations-v3.json`不改，Legacy/v2/v3继续使用历史行为，不能重解释旧录制。

先用真实损伤入口`apply_hit_from`→source damage queue/finish→`observe`复现，而不是直接测试私有投影函数。受控夹具为水下被动观察者持有已知舰艇接触，裁判已经判定一个有来源的Mk82 35DP物理命中；此夹具不宣称潜艇能够投放Mk82，也不替代武器发射合法性验收。`bda-source-red.log`记录v4三红、v2/v3一绿：原攻击方可直接得到0.9653465346534653、两次命中后0.9306930693069307的精确耐久比例。新增四项同时覆盖任意quality或visual标签不能代替BDA、受击方精确数值保留、历史v2/v3兼容及完整World保存续演。

根代理统一运行的`artifacts/rules-next-20261023/validation/workspace.log`已验证`bda_source_2026`四项全部通过，既有BDA兼容测试亦通过；AAM12、Bomb7继续通过。`damage::bda_tables()`已由根代理接入仅v4的规则指纹。该轮总计819通过、1失败、5忽略，唯一失败是独立鱼雷新range案例，故本记录只确认R-049子包已验，不能称整工作区通过。

边界仍未闭合：本包没有实现视觉/声学等有来源的定性或定量损伤估计；不把其他路径的击沉确认、自动战果回执、已有手工植入的BDA记录或全部旧事件生产者算作已审计完成。新v4指纹拒绝旧表解释，是存档版本隔离的一部分。
