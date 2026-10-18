# 规则合规审计报告 rules-audit-004（第四轮）

- 审计对象：四域规则录入（commit `237b7ec`：rules/{aaw,asw,aircombat,damage_ext}.rs + damage.rs + weapons.rs + db_data.rs）、strike.rs 与大剧本（commit `0fe5d2d`：rules/strike.rs、scenarios/pacific_storm.json、examples/pacific_smoke.rs）、web/tutorial.js。
- 审计日期：2026-10-18
- 产出分工：覆盖率更新见 `rules-coverage-matrix.md` **v2 节**；本轮抽查结论与 P0 判定在本文件。
- 限制：同前几轮，规则引文经页级索引核对（aaw/asw/aircombat/damage_ext 各模块自引的 PDF 页码与第一/三轮核对记录一致）。

---

## 1. P0 状态判定（任务要求由本审计裁定）

| 原 P0 | 机制 | 判定 | 依据 |
|---|---|---|---|
| §6.3.1 火控解质量门 | 发射资格 | **关闭**（此前 commit `d35daf5` 已落） | weapons.rs:452-454：手操 Fire 要求接触质量 ≥0.3；自由开火路径质量 >0.25（weapons.rs:243）。Good/Fair/Poor 三档压成两档阈值，属已声明简化（adapt#2 一族），不再构成 P0。 |
| §8.1.5 三秒规则 | PD 带拦截碎片 | **关闭** | rules/aaw.rs:68-73（90%→10% 随拦截距离在 0-4nm PD 带内插值，出带为 0）；aaw.rs:85-155：碎片威胁对象正确解析为"被拦截导弹的目标舰"且距离取导弹-舰几何（fragment_target，含同 tick 独立随机流）。插值替代官方"弹速×口径"二维表为**模块内声明改编**（aaw.rs:23-28：1.8nm≈54% 对齐印刷表 50% 格）。 |
| §14.1.4 空爆伤害 | 空爆关键 | **关闭** | aaw.rs:157-238：D20 按空爆列掷一层"半强度"关键——不扣 DP、不殉爆（§14.1.4 两禁则都落实）；火灾/进水为计时器由 damage::update_fires 消耗。 |

**结论：P0 未关闭清单 = 空（3/3 关闭）。**

残留（降级为 P2 备忘，不构成 P0）：
1. 空爆不查装甲——aaw.rs:33-35 自declared"known gap"（§14.1.4 原文任何装甲均可挡碎片；Entity 尚无装甲字段）。
2. 空爆 D20 分布为官方空爆列的近似：引擎 1-2 武器/3 传感器/4-5 动力/6 CIC/7 声纳/8-12 货舱/13-14 进水/15-18 火灾/19 桥楼/20 舵机；印刷空爆列（印刷14-2/PDF p128）实为 1-3 武器、4-5+12 动力、6+11 CIC、7+10 传感器、8-9 货舱、13-14 进水、15 武器/飞行甲板、16-18 火灾、19-20 飞行甲板/舰桥-空情室。严重度量级相当，但模块注释自称"printed column"与原文逐行不完全一致——**注释措辞建议改为"近似"**（文档级更正，不影响裁决）。
3. aaw.rs:18-21 自行纠正了任务书把指挥仪通道写作"§8.4"的笔误（实为 §6.1/§8.1.1，印刷6-1 SAM Salvo Summary）——更正正确。

## 2. 四域新模块合规抽查

| 模块 | 覆盖规则 | 实现与声明核查 |
|---|---|---|
| rules/aaw.rs (641L) | §8.1.3/§8.1.5/§14.1.4 + 指挥仪通道 | 见上。AA 炮 75/55/25/15% 射程带 × 交叉目标表方位乘数（1.0/0.8/0.6，带界随 AAW 射程带 4/15/30nm 切换）为模块内声明的强度表改编；官方 AA Strength→D100 换算表（印刷8-9/PDF p89）未逐值转录（改编声明在位）。指挥仪通道：`directors` 字段（db_data.rs:180,186 = 4/2）×salvo 封锁在途弹数（weapons.rs:145-152；0=不限），测试覆盖。 |
| rules/asw.rs (1040L) | §5.4.2/5.4.4/5.4.6/5.4.7.1/5.7.1/3.2.4 | 跃变层：跨层 ×0.5（官方 -5→×0.30 的声明简化）+ >200m 海底 ×0.8（自估）；拖曳阵：>25kt 收/ <20kt 持续 30s 布（对官方 21 分钟/15 节的 RTS 自动化改编，声明在位），未布放 ×0.5；声纳浮标：5nm/8h/悬停布放（范围值为工作单固定，声明"Annex K2 值"待录入）；MAD：SensorKind::Magnetic（sensors.rs:127，<60m 截止+×0.5）；电池：恒定放电/通气管充电+低电半速（对 §3.2.4 放电率表的声明简化）。各改编均模块内声明。 |
| rules/aircombat.rs (910L) | §9.1/9.4/9.4.x/9.5.1 | 合并 6nm/15 秒一回合/7 回合上限/油量脱离；机动 0/1/2 为官方双机动等级的声明抽象；航炮基础 Pk 0.12 与 34% hp 伤害为对 Aircraft Gun Attack Table 的**自估**（模块内标注 estimated）；violent 机动 =AAM 生存 ×0.75（aam_evasion_multiplier 挂入 weapons）+受炮击 ×1.2。 |
| rules/damage_ext.rs (441L) + damage.rs | §14.1.6/14.1.7/14.4/14.6 + 扩展表 3-2 | 装甲：cvn 0.85/ddg 0.90/小艇 1.0 类别表（对"按命中部位装甲"的声明改编）；鱼雷 ×(2-装甲) 近似 §14.1.7（0.90 板→1.10 增伤）；损管：火 60/进水 120 进度点+80%/秒推进→`fire_out`/`flooding_contained`；系统修理：末次受击 300s 静默后每 60s 恢复 0.2（进水中不修）；桥楼/舵机/CIC 关键行进入双列表（damage.rs:135-175，比例引自 3-2 扩展表份额）。 |
| rules/strike.rs (243L) | §11.1/11.5 | runway_ops/radar_ops 随 hull 比例派生、甲板周期 ×(2-runway_ops)、0.5 阈值一次性 `runway_degraded`；为 RTS 状态机改编（无逐弹弹坑累积），声明在位。 |

**未发现新的违规**。所有新机制的自估/简化均在模块头声明且与官方结构对齐；唯一的跨文件问题是文档同步（见 §4）。

## 3. 交付物抽查

### 3.1 examples/pacific_smoke.rs —— **抽查通过（双盲不作弊）**
- 所有依赖敌情的触发器均经本方接触簿：`contact_named(world, Side::X, …)` / `red_cvn_contact` / `nearest_red_ship_contact`（pacific_smoke.rs:752-800）统一走 `world.sides[side].contacts` 且过滤 `!stale && quality > 0.3`；部分触发要求 `class_name` 匹配（即已识别，q≥0.7 才产生）——比引擎门槛更严。
- 本方单位状态读取（`world.entity(own_id)` 的位置/武备/甲板）属自视合法范围；`world.entity` 全量遍历仅用于摘要 digest 与裁判断言，不参与任何一方决策。
- 时间表驱动的 doctrine 指令（波次/脉冲 weapons-free）由剧本作者（兼任双方）排定，文件头已声明该特权，不构成泄密。
- 双跑 digest/事件计数一致性断言在位（pacific_smoke.rs:114-119）。

### 3.2 web/tutorial.js —— **抽查通过（不教违规、不泄真值）**
- 数据面只取 `/api/state?side=Blue&max_events=50`（observe.rs 的单方投影），步骤判定只读 `st.units/st.contacts/st.events`——无裁判端点、无真值坐标。
- 教学内容与规则一致：等待雷达地平（"约 12 海里"）出现 CONTACT 并明示"双盲：你看到的是估计位置"；ASSIGN TARGET 展示火控解 Q 值；FIRE 步骤尊重 11nm 舰炮射程与"火控解质量越高命中越稳"（与 weapons.rs:452-454 的质量门一致）；防御步骤教 INCOMING→DEPLOY CHAFF+武器自由自动拦截（与箔条语义一致）。无任何教唆越权（如直瞄真身、绕过质量门）的步骤。

### 3.3 scenarios/pacific_storm.json —— **抽查通过（数值在声明范围内）**
- 全部 13 平台均用现有 db_data 类，无臆造实体；速度不超巡航/最大（CVN 18≤32、AEW 340=cruise、MGB 24≤36、SSK 3-4 节浅伏）。
- 两艘 SSK 深度 45m/180m：均在 max_depth 250 内，且分别处于 150m 跃变层两侧——正好驱动 asw.rs 跨层传播与电池分支（180m>20m 持续放电）。
- AEW 8500m 与 radar_air_xlong 的 alt_m 一致；海况/能见度 14nm/昼间在 weather 语义内；8 小时时长与 100×100nm 箱体无规则冲突。

## 4. 文档同步问题（须后续更正，本审计不代改）

1. **adaptations.md 已过期**：Open Items 第 1 条（三秒规则）与第 6 条（拖曳阵）仍写"未建模"，但 `237b7ec` 已实现；同批的四域改编（空爆插值、AA 炮带、交叉表乘数、层 ×0.5、拖曳自动化、浮标 5nm、电池恒定率、狗斗 15s/自估 Pk、装甲类别表、损管实时化、跑道状态机）均只声明在各模块头，未按 adaptations.md 自己的序言（"凡未在此声明的偏离，按违规处理"）镜像为编号行。建议下一 commit 把 Open Items 1/6 移除、新增约 10 行编号声明。
2. aaw.rs:157-159 空爆列注释措辞（"the printed column"→"approximation of"），见 §1 残留 2。

**总评**：四域录入质量高于前几轮平均水平（模块自带规则出处页码与改编声明、测试与事件可审计）；P0 全部关闭；三个交付物抽查全部通过。矩阵 v2 总账与逐行更新见 rules-coverage-matrix.md。
