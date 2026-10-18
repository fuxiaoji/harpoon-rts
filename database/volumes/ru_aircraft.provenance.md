# ru_aircraft.json — 出处与估算记录

数据书：`8-4.Russias_Aircraft_1_Dec_25.pdf`（Russia's Aircraft, 1 Dec 25 Digital Update，2021 June printing + Dec 2025 更新）。
抽取方式：`swift /tmp/ruair/extract.swift <pdf>`（PDFKit，逐页 `PDFPage.string`）；同时用
`sqlite3 鱼叉/resources/rag/harpoon_rules.sqlite3 "select page,text from chunks where document_id='source_abc1d7408e61' ..."`
交叉核对。卷内印刷页码为 `B-1`…`B-58`，对应 PDF 第 5…62 页（PDF 页 = B 页 + 4）；附录页码为
`H2-3`、`J3-2`、`K-1` 等，对应 PDF 第 70…89 页。

平台条数 35（全部 `kind=Aircraft`），武器 27，传感器 28。

## 1. 平台条目出处

| id | 卷内条目 | 印刷页 | PDF 页 | 备注 |
| --- | --- | --- | --- | --- |
| ru_mig29k | MiG-29K [Fulcrum D] Fighter | B-29 | 33 | DP 33；N-010 Zhuk；1998? |
| ru_mig29kr | MiG-29KR [Fulcrum D] Fighter | B-29–B-30 | 33–34 | DP 29；N-041 Zhuk-M1；2015 |
| ru_su27sm | Su-27SM [Flanker B Mod 1] Fighter | B-41 | 45 | DP 35；N-001V；Dec 04 |
| ru_su30sm | Su-30SM [Flanker H] Fighter | B-42 | 46 | DP 37；N-011MR Bars-R；2015 |
| ru_su33 | Su-33 [Flanker D] Fighter | B-42–B-43 | 46–47 | DP 35；N-001；Aug 98 |
| ru_su35s | Su-35S [Flanker E Variant 2] Fighter | B-43–B-44 | 47–48 | DP 36；N-035 Irbis；Dec 15 |
| ru_mig31bm | MiG-31BM [Foxhound C] Fighter | B-31 | 35 | DP 40；RP-31AM Zaslon-AM；2009 |
| ru_su57 | Su-57 [Felon] Fighter | B-44 | 48 | DP 37；N-036 Byelka；2025 |
| ru_su24m | Su-24M [Fencer D] Attack | B-37–B-38 | 41–42 | DP 34；Orion-A；June 1983 |
| ru_su24mr | Su-24MR [Fencer E] Recon | B-38 | 42 | DP 34；Relyef + 相机，无搜索雷达 |
| ru_su25sm | Su-25SM [Frogfoot] Attack | B-39 | 43 | DP 28；1st Gen RWR + 激光测距 |
| ru_su34 | Su-34 [Fullback] Attack | B-43 | 47 | DP 44；V-004；March 2014 |
| ru_yak38 | Yak-38/38M [Forger A] Attack | B-57 | 61 | DP 20；Gen 0 RWR；STO 数据 |
| ru_yak41m | Yak-41M [Freestyle] Fighter | B-58 | 62 | DP 23；N-002 Zhuk；1995（取消） |
| ru_tu22m3 | Tu-22M-3 [Backfire C] Bomber | B-50–B-51 | 54–55 | DP 49；PNA Rubin；1983 |
| ru_tu95ms | Tu-95MS [Bear H] Bomber | B-52 | 56 | DP 91；Obzor；Dec 83 |
| ru_tu160 | Tu-160 [Blackjack A] Bomber | B-54 | 58 | DP 85；Obzor-K；May 1987 |
| ru_tu142 | Tu-142 [Bear F Mod 0-2] ASW Patrol | B-53 | 57 | DP 83；Berkut；Dec 72 |
| ru_tu142mz | Tu-142MZ [Bear F Mod 5] ASW Patrol | B-54 | 58 | DP 83；Korshun；1993 |
| ru_il38 | Il-38 [May] ASW Patrol | B-7 | 11 | DP 65；Berkut；Aug 68 |
| ru_be12 | Be-12 [Mail] ASW Patrol | B-5 | 9 | DP 39；Initsiativa-2B；Apr 65 |
| ru_a40 | A-40 [Mermaid] ASW Patrol | B-1 | 5 | DP 50；Sokol（用 Novella 2 数据）；1995 |
| ru_a50 | A-50/50M/50U [Mainstay] AEW | B-1 | 5 | DP 84；Shmel/Shmel-M；1992/2012 |
| ru_a100 | A-100 Premier AEW | B-1 | 5 | DP 84；Premier；计划中 |
| ru_il76md90a | Il-76MD-90A [Candid] Transport | B-8 | 12 | DP 90；2019 |
| ru_il78m90a | Il-78M-90A Tanker | B-9 | 13 | DP 90；2021? |
| ru_an124 | An-124 Ruslan [Condor] Transport | B-4 | 8 | DP 112；Feb 87 |
| ru_ka27 | Ka-27 [Helix A] ASW Patrol | B-11 | 15 | DP 18；Osminog + VGS-3；Apr 81 |
| ru_ka29 | Ka-29 [Helix B] Transport/Assault | B-11 | 15 | DP 19；Aug 87 |
| ru_ka31 | Ka-31 [Helix E] AEW | B-12 | 16 | DP 17；E-801 Oko；Aug 2015 |
| ru_ka52 | Ka-52 Alligator [Hokum B] Attack | B-12 | 16 | DP 22；FH-01 Myech；2012 |
| ru_mi24v | Mi-24V [Hind E] Attack | B-17 | 21 | DP 22；Gen 0 RWR；Mar 76 |
| ru_mi8mt | Mi-8MT [Hip H] Transport | B-14–B-15 | 18–19 | DP 19；1977 |
| ru_mi14pl | Mi-14PL [Haze A] ASW | B-16 | 20 | DP 20；VGS-2 + MAD；1976 |
| ru_mi26 | Mi-26 [Halo] Transport | B-17 | 21 | DP 30；May 85 |

## 2. 武器出处

| id | 卷内名称 | 印刷页 | PDF 页 | 采信数值 |
| --- | --- | --- | --- | --- |
| ru_r60m | R-60M [AA-8 Aphid] | H4-2 | 78 | 4.3 nmi / 1721 kt / ATA 1.5 |
| ru_r73 | R-73 [AA-11 Archer] | H4-2 | 78 | 8.1 nmi / 1434 kt / ATA 3.0 |
| ru_r74m2 | R-74M2 | H4-2 | 78 | 14.5 nmi / 1434 kt / ATA 3.5 |
| ru_r27r | R-27R [AA-10 Alamo A] | H4-1 | 77 | 32.4 nmi / 2008 kt / ATA 2.0 |
| ru_r27er | R-27ER [AA-10 Alamo C] | H4-1 | 77 | 39.9 nmi / 2295 kt / ATA 2.5 |
| ru_r77 | R-77 [AA-12 Adder] | H4-2 | 78 | 36.7 nmi / 1721 kt / ATA 3.0 |
| ru_r77_1 | R-77-1 [AA-12 Adder] | H4-2 | 78 | 44.6 nmi / 1721 kt / ATA 3.5 |
| ru_r33 | R-33 [AA-9 Amos] | H4-1 | 77 | 46.2 nmi / 2008 kt / ATA 2.5 |
| ru_r37m | R-37M [AA-13 Axehead] | H4-2 | 78 | 139 nmi / 3442 kt / ATA 3.5 |
| ru_igla_v | 9M39 Igla-V [SA-18 Grouse] | H4-1 | 77 | 2.0 nmi / 1107 kt / ATA 3.0 |
| ru_kh31 | Kh-31AM [AS-17 Krypton] | H2-3 | 72 | 5.4–86.4 nmi / 1690 kt / 35+D6 |
| ru_kh35 | Kh-35U Uran [AS-20 Kayak] | H2-3 | 72 | 3.8–140.4 nmi / 560 kt / 35+D6/2 |
| ru_kh22 | Kh-22M Burya [AS-4 Kitchen] | H2-2 | 71 | 70–189 nmi / 2003 kt / 74+D6 |
| ru_kh101 | Kh-101 [AS-23a Kodiak] | H2-4 | 73 | 15–2160 nmi / 400 kt / 49+D6 |
| ru_apr2 | APR-2 Yastreb | F-1 | 66 | 0.8 nmi / 62 kt / 58 vs sub |
| ru_apr3 | APR-3 Orel | F-1 | 66 | 1.1 nmi / 65 kt / 57 vs sub |
| ru_umgt1 | UMGT-1 Orlan | F-1 | 66 | 4.3 nmi / 41 kt / 50 vs sub |
| ru_at2 | AT-2M | F-1 | 66 | 1.9 nmi / 40 kt / 62 vs sub |
| ru_fab250 | FAB-250M54 | H1-1 | 68 | 39 DP |
| ru_fab500 | FAB-500M54 | H1-1 | 68 | 49 DP |
| ru_kab500 | KAB-500Kr | H2-1 | 70 | 41 DP，1.6–4.9 nmi，528 kt |
| ru_plab250 | PLAB-250-120 Lastochka | E-1 | 65 | 9 DP（major） |
| ru_s8 | S-8 80mm rocket | H1-1 | 68 | 13 DP，salvo 4（双发齐射规则） |
| ru_gun_gsh301 | GSh-30-1 30mm | H3-1 | 76 | 3 DP，ATA 2.5 |
| ru_gun_gsh302 | GSh-30-2 30mm | H3-1 | 76 | 3 DP，ATA 4.4 |
| ru_gun_gsh623 | GSh-6-23 23mm | H3-1 | 76 | 3 DP，ATA 3.9 |
| ru_gun_gsh23 | GSh-23 23mm | H3-1 | 76 | 3 DP，ATA 3.7 |

## 3. 传感器出处

空基雷达取 Annex J3 的列（Range 列为 nmi）。约定：
- 空搜/多模式（LAS）：`air_range = Med`（中型目标），`surface_range = Small`，
  `missile_range = VSmall`；
- 纯对海搜索（SS）：`surface_range = Med`，`air_range = Small`，
  `missile_range = VSmall`。
雷达行内若同时列出 AS 与 SS 两行，按上述功能取对应行。
`radar_class` 取功能：LD/SD、AI、AS、3D 记为 `LAS`；纯粹 SS 记为 `SS`。

| id | 系统 | 印刷页 | PDF 页 | 原始行（Lge/Med/Small/VSmall） |
| --- | --- | --- | --- | --- |
| ru_radar_n001 | N-001 Meych | J3-1 | 83 | 75/75/57/23 |
| ru_radar_n001v | N-001V/VM | J3-1 | 83 | 135/114/81/33 |
| ru_radar_n002 | N-002 Zhuk | J3-1 | 83 | 91/64/45/18 |
| ru_radar_n007am | RP-31AM Zaslon-AM | J3-4 | 86 | 184/129/92/37 |
| ru_radar_n010 | N-010 Zhuk | J3-2 | 84 | 49/34/24/10 |
| ru_radar_n011mr | N-011MR Bars-R | J3-2 | 84 | 135/111/79/32 |
| ru_radar_n035 | N-035 Irbis | J3-2 | 84 | 215/151/108/43 |
| ru_radar_n036 | N-036 Byelka | J3-2 | 84 | 194/136/97/39 |
| ru_radar_n041 | N-041 Zhuk-M1 | J3-2 | 84 | 148/104/74/30 |
| ru_radar_orion_a | Orion-A | J3-3 | 85 | 81/71/40/23 |
| ru_radar_v004 | V-004 | J3-4 | 86 | 101/71/51/20 |
| ru_radar_pna | PNA Rubin | J3-3 | 85 | 270/188/108/60 |
| ru_radar_obzor | Obzor | J3-3 | 85 | 200/177/101/57 |
| ru_radar_obzor_k | Obzor-K | J3-3 | 85 | 200/200/164/92 |
| ru_radar_shmel | Shmel | J3-4 | 86 | 249/175/125/50 |
| ru_radar_premier | Premier | J3-3 | 85 | 350/350/323/129 |
| ru_radar_berkut | Berkut | J3-1 | 83 | 125/89/51/28 |
| ru_radar_korshun | Korshun | J3-1 | 83 | 150/106/61/34 |
| ru_radar_initsiativa_2b | Initsiativa-2B | J3-1 | 83 | 136/76/44/24 |
| ru_radar_initsiativa_2m | Initsiativa-2M | J3-4 | 86 | 116/65/37/21 |
| ru_radar_novella_2 | Novella 2 | J3-3 | 85 | SS 170/95/54/31; AS 101/71/51/20 |
| ru_radar_osminog | Osminog | J3-3 | 85 | 100/90/51/29 |
| ru_radar_e801 | E-801 Oko | J3-1 | 83 | 171/120/86/34 |
| ru_radar_fh01 | FH-01 Myech-1U | J3-1 | 83 | AI 17/12/8.6/3.4; SS 24/14/9.6/4.8 |
| ru_sonar_vgs3 | VGS-3 Ros-V | K-1 | 88 | active 2.5 / passive 0.5 |
| ru_sonar_vgs2 | VGS-2 Oka | K-1 | 88 | active 1.7 / passive 0.4 |
| ru_mad | MAD | J3/A-40 等条目 | 5, 11, 20, 57, 58 | 只给“MAD”，无距离 |
| ru_sonobuoy_p | Sonobuoy field (passive) | K-1/K-2 | 88–89 | RGB-16 被动 3.4 nmi class |

## 4. estimated（数据书未直接给出、按同量级推算/引擎抽象）

- **航速/巡航**：`max_speed_kts` 取数据书该机最高档（喷气机取 Reheat 的 High 值；无 Reheat 的
  运输机/巡逻机/直升机取 Full Mil）；`cruise_speed_kts` 取表内 Cruise 值。均为原值，非估算。
- **endurance_s**：按 `Cruise Range (nmi) / Cruise (kt) × 3600` 计算（由数据书两项原值导出），
  未计备份油/爬升；属推算值。
- **turn_rate_deg_s**：数据书无此列。按类取值：战斗机 12、MiG-31 8、Su-57 14、
  攻击机 8–9、轰炸机 4–6、运输/预警 3–4、直升机 20–30。全部 estimated。
- **accel_kts_per_s**：数据书无此列。喷气战斗机 5、攻击机 3–5、轰炸机 1–2、
  运输/巡逻 1–2、直升机 0.8–1.2。全部 estimated。
- **vp_value**：数据书只有 DP。按本库飞机通用比例 `round(DP × 0.4)` 推算（既有
  fighter 30→12、bomber 45→16 的比例），全部 estimated。
- **chaff**：数据书只给 `Counterm` 代次。按代次映射为干扰弹基数：
  4th Gen 20 / 3rd Gen J&D 16 / 3rd Gen D 12 / 2nd Gen J&D 10 / 2nd Gen D 8 /
  1st Gen J&D 6 / 1st Gen D 4 / Gen 0 2 / 无 0。全部 estimated。
- **AAM 的 pk 与 damage**：Annex H4 只给 ATA rating、射程、速度、弹重，未给 DP。
  `pk = 0.30 + 0.08 × ATA`（再按轻型弹微调，如 R-60M 0.42、Igla 0.40）；
  `damage` 按弹重档：轻型 8–16、中型 R-27/R-77 22–25、重型 R-33/R-37M 30–35。
  全部 estimated。
- **炸弹/火箭 pk、reload**：Annex H1/H2 只给 DP。沿用引擎 §9.6 的标定
  `Bomb pk 0.30–0.35 / reload 180 s`、`Rocket pk 0.25 / salvo 4 / reload 180 s`；
  KAB-500Kr 作为制导炸弹取 pk 0.55。estimated。
- **机炮 range/pk/reload**：Annex H3 只给 DP 与 ATA。射程 1.5 nmi、pk 0.20–0.25、
  salvo 3、reload 4 s 为引擎标定。estimated。
- **PLAB-250-120 的 `damage`**：Annex E 给 9 major / 4 minor，引擎只用一个值，取 9。
- **rcs**：数据书 `Size/Signature` 的第二列为 signature，直接映射
  Large/Medium/Small/VerySmall；Su-57 的 `Stealthy` 引擎无此枚举，映射为 `VerySmall`
  （数据书正文注明仅前向 315°–045° 隐身）。Su-34/Su-35S 的 `Medium/Small` 取 Small。
- **Ka-27 / Mi-14PL 同时挂 dipping sonar 与 MAD**：数据书说明两者不能同任务兼得；
  JSON 中同时给两个传感器（引擎无任务挂载切换），属引擎抽象，非数据书原值。
- **ru_sonobuoy_p**：数据书用“sonobuoy processor + RGB 系列”描述，引擎没有声呐浮标
  布放实体；本卷用一个 `SonarPassive sub_range 3.4` 的抽象传感器代表浮标场
  （RGB-16 被动 3.4 nmi 量级）。estimated / 引擎抽象。
- **ru_mad**：数据书只写 MAD，无作用距离；`sub_range_nm 0.5`、`alt_m 100` 为估算。
- **雷达距离映射**：`air_range=Med`、`surface_range=Small`、`missile_range=VSmall` 是本卷
  的映射约定（见上），不是数据书直接列出的“air/surface/missile”分类。
- **Su-24MR / Su-25SM / Mi-24V / Ka-29 / Yak-38**：数据书无搜索雷达，只给 RWR/激光；
  JSON 用 `esm_std + visual` 代表其 RWR/目视能力（`esm_std` 是通用件，非新定义）。
- **Tu-95MS / Tu-160 的 Kh-101 载弹**：数据书给 8 或 12 枚；由于引擎无核/常区分，
  只录常规型 Kh-101（Slcm），未录 Kh-55SM/Kh-102 核弹型。

## 5. 已知偏差 / 未录入

- 任务提示中的 `ru_kh31` “speed 1000 kt” 与卷内 Annex H2 的 Kh-31AM 1690 kt 不一致，
  按卷取 1690 kt。
- `ru_r27er`/`ru_r73`/`ru_r77` 三个 id 也出现在 `ru_navy.json`（同一数据书 Annex H4）。
  合并时按文件名顺序 `ru_navy.json` 在后、会覆盖本卷同名定义；两者名称/射程/速度一致，
  仅 pk/damage 的估算口径略有差异（本卷 pk 由 ATA 公式得出，damage 按弹重档）。
  `ru_igla_v` 刻意改名为空中发射型，避免被 `ru_navy.json` 的舰载 `ru_igla`（Sam）覆盖。
- Su-57 的巡航速度在卷内本身标注为 `e530`（估算值），JSON 取 530 kt。
- 本卷只录 35 类（满足 ≥25），未覆盖卷内全部 228 个条目（早期 MiG-15/17/19/21/23/25/27、
  Su-7/9/11/15/17/22、Tu-16/Tu-95 早期型、Yak-25/28、无人机等未录）。
- 数据书的核炸弹、核深弹、反辐射导弹（Kh-31P/Kh-58）、空射诱饵、侦察吊舱、电子战吊舱
  未建模（引擎无对应 WeaponKind/传感器语义）。
- 直升机的反坦克导弹（9M114/9M120/9M127）未录入：引擎没有 ATGM 语义，
  用 `ru_s8` 火箭与机炮近似。
- 歼击机的 IRST/OLS、数据链、RWR 代次未逐一建模；统一用 `esm_std` 代表 RWR/ESM。
- 巡航导弹的终末段弹道统一写成 `SeaSkimmer`（Kh-22/Kh-32 实际为高空巡航+俯冲）；
  Kh-101 为低空巡航。引擎只有这一种 ASCM 制导枚举。
- 米格-29K 条目的 N-010 雷达在 J3-2 表内；Kh-31A/P 的 “P” 反辐射型未单独定义，
  本卷 `ru_kh31` 取 Kh-31AM 反舰型数值。
- 所有 `max_speed` 取无外挂干净构型；数据书注明挂 Kh-22 时 Tu-22M3 最大 895 kt，
  本 JSON 未按挂载降低最大速度（引擎无按挂载限速）。
- 引擎 `crates/harpoon-core/src/sensors.rs:42` 的 `is_radar_sensor` 以 **id 前缀 `radar`**
  判定一个平台是否为辐射源。本卷新雷达按命名规则用 `ru_radar_*`，因此 ESM 不会把这些
  俄机视为雷达辐射源。正确修法是在共享代码中改为 `SensorKind::Radar` 判定，超出本卷
  权限，未改动；在此记录该引擎侧限制。
