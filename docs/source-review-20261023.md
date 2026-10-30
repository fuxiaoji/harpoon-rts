# 火控、防空与对陆武器续接复核

仅 `hv2026-rts-v4`。本轮接管已有模块后，先独立写出来源失败案例，再修执行链；完整章节仍未完成。联合`continuation-domains-3.log`已实测crossfix/director两例通过、被动测距误用于主动/目视例按预期失败（随后修正kind资格），`land_weapons_2026`三例全部通过。最终资格修复及全工作区复核由联合验收记录确认，不能用本文件或函数存在代替测试通过。

| 规则 | 来源与复核 | 数据/执行改变 | 验证入口与范围 |
|---|---|---|---|
| 6.3.3 被动RF交叉定位 | 中文核心RAG“火控”；英文2026 PDF73 / 6-5原页视觉，表PDF71 / 6-3 | 原逻辑错误地先查Passive RF表。现三代ES cross-fix均查Active RF，再按1/2代降一级、3/4代不变、5/6代升一级 | `source_review_20261023::crossfix_uses_active_rf_table_before_es_generation_quality_adjustment`；原失败已记录。只验解算API，尚无来源TDL多平台传输实装，不能标整项执行完成 |
| 6.3.4 被动测距加成资格 | 英文2026 PDF73 / 6-5的被动测距阵正文、PDF72 / 6-4修正栏 | 原逻辑允许wavefront/vertical flank的+4/+3同时加到主动声纳或目视解；现明确要求PassiveAcoustic | `source_review_20261023::acoustic_passive_ranging_cannot_be_added_to_active_or_visual_solution`，修复前联合日志已实际失败；不把fixture调用算成真实装配阵列已完整接入 |
| 6.2.2–6.2.3 / 8.1.3 Mk45 AA指挥仪 | 英文2026 PDF69–70 / 6-1..2；US Navy2025 PDF34 / A31备注Mk45使用SPY-1防空、PDF75 / C1的N；新独立PDF34渲染在根artifacts/rules-all-20261023/source-review/us-p034.png | 明确SPY-1D→Burke主空情雷达稳定ID绑定、指挥仪弧；发射前检查实际存在、失能、雷达开机。No Local Control在失去指挥仪后禁射，不沿用完整源AA强度 | `source_review_20261023::no_local_mk45_cannot_keep_directed_aa_solution_after_director_is_removed`；原失败已记录。只验此已核单炮位，未补复杂共享指挥仪/EO/局部控制程序 |
| 11.2 LACM/弹道导弹命中表 | 中文核心RAG“巡航导弹”；英文2026 PDF116 / 11-1正文，PDF118 / 11-3表视觉 | 实际sea-skimmer对Base终端调用guidance×size表；来源缺失不落回def.pk；原D100进入aaw.surface.land_outcomes与存档 | `land_weapons_2026`；T2精度在测试中明确为scenario assumption，并非真实Tomahawk型号判定 |
| 11.1 / 11.1.2 / 14.1.6 地面DP、装甲与比率关键 | 英文2026 PDF116 / 11-1、PDF127 / 14-1、PDF130 / 14-4；共用已核损伤比率表 | 支持已核HE DP、轨迹对应垂直/水平装甲、严格大于才穿透、不穿透半DP。穿透且无附加关键的支持子集按本tick总DP共同计算普通比率；建筑F/G不套小艇不累积HP规则。已可由Tomahawk51DP直接摧毁20DP源雷达，目标保持建筑残骸实体 | `land_weapons_2026`实际`advance_projectiles`，包含def.pk=0仍使用源20%命中格、100种子命中/未中、功能失能、保存恢复、待裁不移动不重掷。直接摧毁目标不需要追加关键判断 |
| 11 未明分支 | 11.1.2与14.1.5跨章适用关系尚需明确 | 未核导弹附加关键/自动火灾如何适用存活建筑、未穿透存活建筑关键保护、核伤/airburst/聚能装药、无DP结构适用等，保存完整弹体与已有骰结果待裁；无假HP或def.pk回退 | 具体待裁理由写入pending_adjudications；重进/读档不重新滚骰 |
| 11.4 / 11.6 / 11.7 NGFS | 原书要求8.3源炮表/测距惩罚/倍伤/反向组件分配 | 当前v4禁用旧range-fraction对Base炮击，并在发射/扣库前返回来源未装配原因。root已有spotting/reverse API不等于真实炮击完成 | 本轮不标完成；后续必须接实际8.3解算和组件分配，不放开旧近似 |

附加核查：Ford Phalanx三物理炮位每位7次，数据库fit中7是每炮位数，`scenario.rs`加载时乘3得到总弹库21；并非7次全舰，也不应把fit改为21再乘3。ESSM两架每架8枚同理，总16。

仍须保留在完整分母中的缺口：6.2复杂director分配/局部控制；6.3真实多平台TDL、cross-fix几何、BOL和placement在实际反舰/鱼雷链的消费；7章完整制导/随机锁定；8.3全部海面炮击；8.4反舰表目前仅结构化，旧Ship终端仍未消费；8.5深弹/投射武器；10章鱼雷仍有旧常量锥角、探测距离、诱饵概率和def.pk，不能算全水下规则录入。

待并入总歧义账：`R-LAND-EXTRA-SCOPE`，确认§14.1.5附加airframe/自动Fire对存活陆上结构适用方式及§11.1.2失能关系；`R-LAND-NONPEN-CRITICAL`，确认非穿透陆上结构可适用的功能关键保护。两项均已保守阻断相应裁决。
