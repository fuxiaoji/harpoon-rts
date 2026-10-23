# cn_navy.json — 出处与估算说明

数据书：`6-3.（数据库）2025版Chinas_Navy_1_Dec_25.pdf`
（China's Navy, 1 Oct 2021 printing, 1 Dec 25 Digital Update，共 105 个 PDF 页）
抽取方式：`鱼叉/resources/rag/harpoon_rules.sqlite3` 中 `document_id = source_b2e04b4bc501`
的逐页 chunk 文本（`chunks.page` + `chunks.text`）。

页码约定（本文件全部使用印刷页码，不用 PDF 页码）：

- Annex A（舰船）：印刷页为 `A-n`，对应 PDF 第 `n+7` 页。例：`Type 055` 在 A-8 = PDF 15。
- Annex B（飞机）：印刷页为 `B-n`，对应 PDF 第 `n+40` 页。例：`J-15` 在 B-11 = PDF 51。
- 武器/传感器附件：直接引用印刷页 `C-1`、`D-1`、`E-1`、`F-1/F-2`、`H-3/H-4/H-5`、`J-1/J-3`、`K-1/K-2`。

## 平台条目（43 类）

| id | 条目 | 卷页 | 备注 |
|---|---|---|---|
| cn_001 | Ex-Russian Project 1143.5/Type 001 [Kuznetsov] CV | A-3 | Liaoning/16；DP 1010 |
| cn_002 | Type 002 [Kuznetsov Mod] CV | A-3 | Shandong/17；DP 1023 |
| cn_003 | Type 003 CV | A-3 | Fujian/18；DP 1278 |
| cn_094 | Type 094/094A [Jin] SSBN | A-3 | 094/094A 合并；DP 217；SLBM 未建模 |
| cn_093a | Type 093A/09IIIA [Shang] SSN | A-4 | 与 093 Mod 合并；DP 145 |
| cn_093 | Type 093/09III [Shang] SSN | A-5 | DP 145 |
| cn_039a | Type 039A/039B/039C [Yuan] SSP | A-5 | 本卷 DP 100（先导层 107 为 Type 041 行，已按本卷修正） |
| cn_039g | Type 039G1 [Song Mod] SS | A-5 | DP 74 |
| cn_kilo | Pr. 636/636M [Improved Kilo] SS | A-6 | DP 91 |
| cn_055 | Type 055 [Renhai] CG | A-8 | DP 283；112 单元 VLS |
| cn_052d | Type 052D [Luyang III] DDG | A-8/A-9 | DP 204；Batch 3 为 210、LJQ-520，未单列 |
| cn_052c | Type 052C [Luyang II] DDG | A-9 | DP 201；HQ-9A 48 发 |
| cn_052b | Type 052B [Luyang I] DDG | A-9 | DP 198 |
| cn_052 | Type 052 [Luhu] DD | A-10 | DP 155；2011-12 改装后状态 |
| cn_051c | Type 051C [Luzhou] DDG | A-9 | DP 216 |
| cn_051b | Type 051B [Luhai] (2016) DDG | A-10 | DP 199；2016 改装后状态 |
| cn_956e | Pr. 956E [Sovremenny III] DDG | A-8 | DP 223；按 2019/2021 改装后（HQ-16B/YJ-12A）建模 |
| cn_956em | Pr. 956EM [Sovremenny II] DDG | A-9 | DP 223 |
| cn_054a | Type 054A [Jiangkai II] FFG | A-12 | DP 140；按前 16 艘的配置建模 |
| cn_054 | Type 054 [Jiangkai I] FFG | A-13 | DP 138 |
| cn_053h3 | Type 053H3 [Jiangwei II] FF | A-13 | DP 102 |
| cn_053h1g | Type 053H1G [Jianghu I Upgrade] (2007) FF | A-13 | DP 102；2007 升级型 |
| cn_056a | Type 056A [Jiangdao] FFL | A-16 | DP 71 |
| cn_056 | Type 056 [Jiangdao] FFL | A-16 | DP 71；2021 年整级转海警，仍保留条目 |
| cn_022 | Type 22 [Houbei] PGG | A-18 | DP 16；两座四联 YJ-83 |
| cn_071 | Type 071 [Yuzhao] LPD | A-22 | DP 358 |
| cn_075 | Type 075 [Yushen] LHD | A-22 | DP 548 |
| cn_072a | Type 072A/072B [Yuting II] LST | A-23 | DP 122 |
| cn_903a | Type 903A [Fuchi Mod] AOR | A-28 | DP 226 |
| cn_815a | Type 815A [Dongdiao II] AGM/AGI | A-27 | DP 141 |
| cn_j15 | J-15 Feisha Fighter | B-11 | Damage Value 35；ceil 17000 m |
| cn_j15d | J-15D EW Fighter | B-11（J-15 Remarks） | 双座电子战型；Damage Value 按 J-15 取 35（estimated） |
| cn_j11b | J-11B [Flanker L] Fighter | B-10/B-11 | Damage Value 35；ceil 18500 m |
| cn_jh7a | JH-7A Feibao [Flounder] Attack | B-12 | Damage Value 30；ceil 15500 m |
| cn_h6j | H-6J Naval Bomber | B-5 | H-6J/K/N 条目；Damage Value 47；H-6J 反舰型 |
| cn_kj600 | KJ-600 AEW | B-14 | Damage Value 42；估计数据；ceil 15000 m |
| cn_z18f | Z-18F Haiying ASW Helicopter | B-26 | Damage Value 13；ceil 3150 m |
| cn_z18j | Z-18J Haibianfu AEW Helicopter | B-26 | 由 Z-18F/J 条目 Remarks；Damage Value 13 |
| cn_z9c | Z-9C Haitun ASW Helicopter | B-25 | Damage Value 12；ceil 4300 m |
| cn_z9d | Z-9D Haitun Attack Helicopter | B-25 | Damage Value 12；Z-9C/D 条目 |
| cn_z20f | Z-20F ASW Helicopter | B-26 | Damage Value 17；ceil 6000 m |
| cn_ka28 | Ka-28 [Helix A] ASW Helicopter | B-13 | Damage Value 18；ceil 5000 m |
| cn_y8q | Y-8Q (KQ-200) Maritime Patrol Aircraft | B-22 | 条目名 Y-8FQ [Maid]；Damage Value 66；ceil 10400 m |

## 武器条目（41 个）

| id | 系统 | 卷页 | 采用值（卷） |
|---|---|---|---|
| cn_pj45a | PJ-45A 130mm/70 | C-1 | 15.7 nmi（31.8 kyds）、短程伤害 25 |
| cn_ak130 | AK-130 130mm/70 | C-2 | 12.4 nmi（25.2 kyds）、短程伤害 26 |
| cn_pj87 | PJ-87 100mm/55 | C-1 | 9.4 nmi（19.0 kyds）、短程伤害 20 |
| cn_pj33b | PJ-33B 100mm/56 | C-1 | 12.2 nmi（24.6 kyds）、短程伤害 19 |
| cn_pj26 | PJ-26 76mm/59 | C-1 | 7.4 nmi（15.0 kyds）、短程伤害 16 |
| cn_pj76 | PJ-76 37mm/67 | C-1 | 1.09 nmi（2.2 kyds）、伤害 4 |
| cn_pj13 | PJ-13 30mm | C-1 | 0.94 nmi（1.9 kyds）、伤害 5 |
| cn_hhq9 | HQ-9B | D-1 | 162 nmi、3438 kts、伤害 37、6@2 |
| cn_hhq9a | HQ-9A | D-1 | 80 nmi、3438 kts、伤害 37、6@2 |
| cn_hhq16 | HQ-16B | D-1 | 37.8 nmi、2008 kts、伤害 29、1@2 |
| cn_hq10 | HQ-10 | D-1 | 5.0 nmi、1319 kts、伤害 15、3@2 |
| cn_hq7 | HQ-7A | D-1 | 8.1 nmi、1750 kts、伤害 17、1@2 |
| cn_shtil1 | Shtil-1 (9M317) | D-1 | 22 nmi、2008 kts、伤害 27、2@2 |
| cn_48n6 | 48N6 (SA-N-20) | D-1 | 81 nmi、3690 kts、伤害 39、6@2 |
| cn_yj83 | YJ-83 | D-1 | 97 nmi、594 kts、伤害 38、穿甲 9 |
| cn_yj83k | YJ-83K | H-4 | 135 nmi、594 kts、伤害 38 |
| cn_yj12 | YJ-12A | D-1 | 216 nmi、1719 kts、伤害 48 |
| cn_yj18 | YJ-18 | D-1 | 165.5 nmi、巡航 528 kts、末段 1980 kts（Remarks G）、伤害 33 |
| cn_yj62 | YJ-62 | D-1 | 189 nmi、528 kts、伤害 44 |
| cn_yj82 | YJ-82 | D-1 | 16.2 nmi、594 kts、伤害 37 |
| cn_3m54e | 3M54E Klub | D-1 | 118 nmi、528 kts、伤害 42 |
| cn_yj9 | YJ-9 | H-4 | 8.1 nmi、561 kts、伤害 20 |
| cn_yj91 | YJ-91 | H-4 | 64.8 nmi、1940 kts、伤害 D6+2 |
| cn_moskit_mve | Moskit-MVE (3M80MVE) | D-1 | 130 nmi、1650 kts、伤害 52 |
| cn_dh10 | DH-10 | D-1 | 1080 nmi、528 kts、伤害 52 |
| cn_akd88 | AKD-88 | H-3 | 124 nmi、594 kts、伤害 36 |
| cn_yu6 | Yu-6 | F-1 | 16.4 nmi @50 kts（24.6 nmi @36 kts）、雷头 141 舰/100 潜 |
| cn_yu10 | Yu-10 | F-1 | 21.8 nmi @60 kts（27.4 nmi @40 kts）、141/100 |
| cn_yu9 | Yu-9 | F-1 | 9.7 nmi @42 kts（15.7 nmi @25 kts）、128/91 |
| cn_yu7 | Yu-7 | F-1 | 5.1 nmi、43 kts、雷头 49 |
| cn_53_65k | 53-65K | F-1 | 10.4 nmi、45 kts、雷头 150 |
| cn_te2_01 | TE-2-01 | F-2 | 16.2 nmi @45 kts（27 nmi @35 kts）、150/106 |
| cn_apr3e | APR-3E Orel | F-1 | 1.1 nmi、65 kts、雷头 57 |
| cn_yu8 | Yu-8 | E-1（Annex E4） | 16.2 nmi、660 kts、载荷 Yu-7C |
| cn_whh003 | WHH-003 | E-1（Annex E3） | 1.7 nmi、接触命中 0.03/0.11/0.18、伤害 22/7/3 |
| cn_whh004 | WHH-004 | E-1（Annex E3） | 1.6 nmi、0.03/0.19/0.39、伤害 30/11/5 |
| cn_rbu1000 | RBU-1000 Smerch-3 | E-1（Annex E3） | 0.5 nmi、接触命中 0.11、伤害 11/6/3 |
| cn_pl8 | PL-8 | H-5 | 8.1 nmi、1721 kts、ATA 3.0 |
| cn_pl10 | PL-10 | H-5 | 10.8 nmi、2295 kts、ATA 3.5 |
| cn_pl12 | PL-12 | H-5 | 37.8 nmi（对 ATA 9.5 为 37.8）、2295 kts |
| cn_pl15 | PL-15 | H-5 | 108 nmi（对 ATA 9.5 列 108）、2295 kts |

## 传感器条目（31 个）

| id | 系统 | 卷页 | 采用值（卷） |
|---|---|---|---|
| cn_ljg346b | LJG-346B AESA 3D | J-1 | 250/247/177/71/21（L/M/S/VSmall/Stealthy） |
| cn_ljg346a | LJG-346A | J-1 | 150/150/110/44/13 |
| cn_ljg346 | LJG-346 | J-1 | 125/125/90/36/11 |
| cn_ljq382 | LJQ-382 3D | J-1 | 162/162/124/49/15；SS 27 |
| cn_ljq517b | LJQ-517A/B | J-1 | AS 189/132/95/38/11 |
| cn_ljq517h1 | LJQ-517H-1/H2 | J-1 | AS 135/95/68/27/8.1 |
| cn_ljq364 | LJQ-364 | J-1 | LAS 54/54/54/22/6.5；SS 35 |
| cn_ljq366 | LJQ-366 [Band Stand] | J-1 | SS-T 120/120/72/40/22 |
| cn_ljq361a | LJQ-361A | J-1 | LAS 35/35/35/20/6.0；SS 35 |
| cn_ljq362 | LJQ-362 Mod | J-1 | LAS 35/35/27/11/3.2；SS-T 65 |
| cn_ljq360 | LJQ-360 | J-1 | LAS 81/81/73/29/8.7；SS 57 |
| cn_aesa_xband | AESA X-Band LAS | J-1 | LAS 75/75/63/25/7.6；SS 40 |
| cn_aesa_targeting | AESA Targeting Complex | J-1 | SS-T 135/135/99/56/31 |
| cn_klj4 | KLJ-4 (Type 1474) | J-3 | LD/SD 81/81/64/26/8；SS 162 |
| cn_klc7 | KLC-7 Silk Road Eye | J-3 | 342/240/171/68/21 |
| cn_kq200 | KQ-200 | J-3 | SS 210/118/67/38/21 |
| cn_z18j_radar | Z-18J radar | J-3 | AS 214/150/107/43/13 |
| cn_z18f_radar | Z-18F radar | J-3 | SS 120/67/38/22/12 |
| cn_z20f_radar | Z-20F radar | J-3 | SS 120/67/38/22/12 |
| cn_sjd11 | SJD-11 | K-1 | 主动 4.3 / 被动 1.5 |
| cn_sjd9 | SJD-9 | K-1 | 主动 3.6 / 被动 1.4 |
| cn_sjd5a | SJD-5A | K-1 | 主动 1.8 / 被动 0.4 |
| cn_sjg206 | SJG-206 | K-1 | 被动 6.0 |
| cn_sjg311 | SJG-311 | K-1 | 主/被动 7.0 |
| cn_sqz265a | SQZ-265A bow | K-1 | 主/被动 2.5 |
| cn_sqz265a_tow | SQZ-265A towed | K-1 | 被动 4.0 |
| cn_sqz262 | SQZ-262C | K-1 | 主动 2.1 / 被动 2.0 |
| cn_sqg204a | SQG-204A/204B | K-1 | 被动 2.5 |
| cn_sqg207 | SQG-207 | K-1 | 被动 3.0 |
| cn_sqg2b | SQG-2/2B | K-1 | 被动 1.9 |
| cn_mgk335 | MGK-335EM | K-2 | 主动 4.3 / 被动 1.5 |

## 估算与适配清单（不是卷面数值）

以下字段卷不直接印出，均按既有引擎标定规则换算，属 **estimated**：

1. **noise_db（全部平台）**：卷只印声学签名级（Loud/Noisy/Quiet/VQuiet）。
   按 `db_fleets.rs` 的既有标定（Loud 66、Noisy 56-60、Quiet 40-46、VQuiet 28-34）按吨位插值。
   例：055 Quiet→46，052D Noisy→58，039A VQuiet→28，Kilo VQuiet→30。
2. **vp_value（全部平台）**：按 hp、吨位、任务价值缩放；卷不印。
3. **turn_rate_deg_s（全部平台）**：按吨位/舰型估计；卷不印。
4. **chaff / decoys（全部平台）**：按卷内对抗措施配置量级估计；卷不印具体发数。
5. **舰船 cruise_speed_kts**：按最大航速的 55-70% 取整（estimated）；
   潜艇取静音巡逻航速 5-6 kts（estimated）。
6. **飞机 endurance_s**：由卷内 Cruise Range ÷ Cruise Speed × 3600 推得
   （例如 J-15 1135/485×3600=8425 s），不是卷面直接印出的续航秒数。
7. **飞机 ceiling**：写入 `name`（如 `(ceil 17000 m)`）；卷印的是米制升限。
8. **飞机 vp_value、turn_rate_deg_s、chaff/decoys**：估计。
9. **飞机 hp**：取卷内 `Damage Value`（J-15D 无独立行，按 J-15 的 35 估计）。
10. **潜艇 max_depth_m**：
    - 039A/039B/039C = 250（卷 `Btry Rtng: 250 (new)`）
    - 039G1 = 165（卷 `Btry Rtng: 165 (new)`）
    - Kilo 636/636M = 200（卷 `Btry Rtng: 200 (new)`）
    - 093/093A、094 = 300（卷只写 `Max Depth: Int V`，无米数；estimated）
11. **武器 pk / salvo / reload_s / min_range_nm**：卷不印 Harpoon 引擎所需的
    单发命中率、齐射数与再装填秒数；按同类内置武器量级估计。damage 采用卷内
    伤害基数（如 `38 + D6/2` 取 38，D6 项未逐发模拟）。空空导弹 H-5 只给
    ATA 等级，未给伤害点数；`cn_pl8`/`cn_pl10`/`cn_pl12`/`cn_pl15` 的
    damage 20/20/25/28 为按内置 `aam_ir` 量级估计。`cn_yj91` 的 D6+2
    取 10。
12. **重型鱼雷单值伤害**：引擎 `damage` 只有一个值，而卷面分对舰/对潜两列。
    `cn_yu6`=120（卷 141/100）、`cn_yu10`=120（141/100）、`cn_yu9`=110（128/91）、
    `cn_te2_01`=128（150/106），均为取中间值的 estimated 适配。
13. **cn_yu8 / cn_whh003 / cn_whh004**：反潜火箭类，`pk` 取卷内接触命中概率量级；
    `salvo` 按发射管数取 1-2。
14. **cn_yj18 speed=1980 kts**：卷 D-1 Remarks G 明确“最后 10.8 nmi 为
    Mach 3.0 冲刺（1980 kts）”；任务提示中的“speed 900”与卷面不一致，
    本转录以卷为准。引擎只有一个 speed 字段，未模拟 528 kts 巡航段。
15. **cn_hhq9 / cn_hhq9a**：分别为 HQ-9B（162 nmi）与 HQ-9A（80 nmi）；
    052C 用 HQ-9A，052D/055 用 HQ-9B。
16. **传感器 air/surface/missile 列**：J-1/J-3 给出 L/M/S/VSmall/Stealthy 多列。
    本卷取“Large 目标”列为 air_range，SS 行（若存在）为 surface_range，
    “VSmall”列作 missile_range。没有 SS 行的雷达（LJQ-517B、KLC-7、Z-18J）
    其 surface_range 为估计值；`cn_ljq366` 取 SS-T 行的 Large 列。
    声呐取 K-1/K-2 的主动/被动 nm 值，未建模频率、会聚区等。
17. **deck.capacity / cycle_s**：capacity 取卷内 Air Group 总数或机库/甲板
    可容纳量；cycle_s 为引擎甲板周期估计（航母 90-120 s、两栖舰 120-240 s、
    驱逐舰/护卫舰 180 s），卷不印。
18. **052D 合并**：Batch 1/2 与 Batch 3 合并为 `cn_052d`；Batch 3 的 hp 210、
    LJQ-520、Z-20 未单列（`cn_ljq520` 未定义，Batch 3 仍用 `cn_ljq517b`）。
19. **054A 合并**：前 16 艘（SJG-206、PJ-12）与后续批次（SJG-311、PJ-11）
    合并；JSON 按前 16 艘配置，`cn_sjg206`/`ciws_30mm` 表示。054A+ 的
    SJG-311 与 PJ-11 未单列。
20. **052B**：广州/168 在 2019 年改装为 HQ-16B VLS，未单列；`cn_052b`
    保持卷内主配置 Shtil-1。
21. **956E**：按 2019/2021 改装后的 Sovremenny III 建模（HQ-16B、YJ-12A、
    HQ-10）；原 956E 配置见 A-10。
22. **056**：卷注 2021 年整级转海警；仍按卷内 FFL 条目保留。
23. **Kilo 载荷**：卷面 18 枚（12×53-65K + 6×TE-2-01）。JSON 同时给出
    `cn_3m54e` 4 发作为 636M 的 Klub 替代载荷；两者不是同时携带，
    ammo 相加会超过 18。
24. **未建模项**：
    - 094 的 JL-2/JL-2A SLBM：引擎无 SLBM 武器类型，未虚构条目。
    - 071 的 120mm 多管火箭（仅对地）：无对应卷面弹道数据，未建模。
    - 075/071 的 LCAC、726A 气垫艇、LCVP：引擎无登陆艇类型，未建模。
    - 舰载直升机的吊放声呐深度、声呐浮标数量：引擎只建模 `dipping_sonar` 布尔值。
    - 反潜导弹 Yu-8 的“火箭助飞 + 鱼雷”两段弹道：用 `AswRocket` 单段近似。
    - 956EM 的 Kortik 弹炮合一系统：只建模 30mm 炮（`ciws_30mm`），
      9M311 导弹（4.3 nmi）未单列。
    - 反舰导弹的航路规划、末段机动、红外/被动雷达复合制导：引擎只有
      `SeaSkimmer` 单一制导类型。
25. **舰载机映射**：903A 的 Z-8 用 `cn_z18f` 代表（同量级大型直升机）；
    052D/052C/052B/051B/054A/056 等的 Z-9C 用 `cn_z9c`；956E/EM 的 Ka-28
    用 `cn_ka28`；052D Batch 3 的 Z-20 未单列。
26. **航空联队数量**：001 = 24 J-15 + 6 Z-18F + 4 Z-18J + 2 Z-9S；
    002 = 24 J-15 + 3 J-15D + 6 Z-18F + 4 Z-18J + 2 Z-9S；
    003 = 48 J-15T + 4 KJ-600 + 10 Z-18F（Z-18A/Z-8JH/Z-9S 未单列）。
    Z-9S 是 SAR 型，JSON 用 `cn_z9c` 占位（未单列 SAR 类）。
27. **hp 的 D 值**：全部取卷内 `Dam Pts` 的 100% 行；未应用卷首的
    -10%/-15% 中国造舰伤害修正（该修正属于裁决规则，不进入平台 hp 基数）。
28. **VLS 内部分配**：052D（64 单元）、055（112 单元）、054A（32 单元）、
    051B（32 单元）的 JSON 装载是代表性分配，各项相加等于卷面单元数；
    卷只给“可装 HQ-9B/YJ-18/DH-10/Yu-8”的混合清单，没有逐单元数量。
29. **J-15 载弹**：JSON 采用反舰挂载 2×YJ-83K + 6×PL-12 + 4×PL-8；
    卷内另有 8×PL-12 + 4×PL-8B 的制空挂载与 4×250/500 kg 炸弹挂载，未同时列出。
30. **KJ-600**：卷内该条目标注 “Estimated stats”，全部性能（速度、航程、
    雷达）均为卷面估计值；`cn_klc7` 的 surface_range 为 60 nmi（estimated）。
31. **J-15D / Z-18J / Z-20F**：卷内为改型说明或估计条目；J-15D 的
    Damage Value 35 按 J-15 取，Z-18J 的 hp 13 按 Z-18F 取。
32. **ESM 发射源适配**：引擎 `sensors.rs` 的 `is_radar_sensor` 只识别
    id 以 `radar` 开头的传感器；为使 `cn_*` 雷达平台仍能被 ESM 发现，
    所有带 `cn_` 雷达的平台都同时保留一个通用 `radar_nav` /
    `radar_surf_*` 导航/搜索雷达。`cn_*` 雷达负责测距，通用雷达负责
    满足引擎的发射源判定。这是引擎适配，不是卷面数据。
33. **Y-8Q 的 MAD 与声呐浮标**：卷内 K2 附件有声呐浮标数据，但引擎只有
    `Magnetic` 传感器类型且本卷未给 MAD 距离；为避免虚构数值，未添加
    MAD 条目，Y-8Q 只装 `cn_yu7` 鱼雷与 KQ-200 雷达/ESM。
34. **签名映射**：卷内 `Signature` 首词映射到引擎的 4 档 `RcsClass`。
    `VSmall`→`VerySmall`；022 的 `Stealthy` 也映射为 `VerySmall`
    （引擎无 Stealthy 档，这是最近档，属适配而非卷面 4 档原值）。
35. **max_speed_kts**：全部取卷内 `Surf Speed`（水面舰）或 `Subm Speed`（潜艇）
    的第一个（无损）值；没有取 0 节沉没值。`cruise_speed_kts` 则是估计值。

## 2026-10-23 新增：cn_yj12_air（鹰击-12 空射型）与 cn_j15_yj12（J-15 防区外打击构型）

`cn_yj12_air` 完整复制本卷已核实的 `cn_yj12`（射程 216nm、1.7Ma 末端）
数值行，仅去掉对陆能力（空射变体无对陆任务），用于 J-15 空射。
`cn_j15_yj12` 克隆 `cn_j15`，弹药改为 2×PL-12 + 2×PL-8 自卫 +
1×`cn_yj12_air`。J-15 挂载 YJ-12 为 China's Navy 2025 导弹附录
（H-3/H-4）记载的空射构型家族；具体挂点数为估算。动机：近岸航母
编队战平衡——Blue 侧对等获得防区外打击（对位 Red 的 F-35C/LRASM
构型），射程 216nm 覆盖 HQ-9B（162nm）包线。数据库平台 410 → 411、
武器 +1。
