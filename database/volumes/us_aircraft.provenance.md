# us_aircraft.json — 出处与估算记录

数据书：**4-4.Americas_Aircraft__1_Dec_25.pdf**（America's Aircraft, U.S. Military
Aviation 1955–Present, 1 Dec 25 Digital Update）。

抽取方式：本机没有 `pdftotext`/`/tmp/pdftext`，因此逐页读取
`鱼叉/resources/rag/harpoon_rules.sqlite3` 的 `chunks` 表
（`document_id = source_8dc2d5008f81`，`page` 为 PDF 页码，1–126 页），
按 `page` 拼接文本。下表页码同时给出 **PDF 页**和书内印刷页（B-x / H-x / F-x / J3-x）。
书内印刷页来自每页第 2 行的页眉。

规模：34 个平台、12 个武器、2 个传感器；全部平台 `kind = "Aircraft"`。
通用传感器/武器 id（`radar_air_*`、`esm_std`、`visual`、`sonar_dipping`、
`aam_ir`）直接复用内置定义，未在本卷重复定义。

## 平台条目

| id | 条目 | PDF 页 | 印刷页 | 转录/说明 |
| --- | --- | --- | --- | --- |
| us_f18c | F/A-18C/D Hornet | 48 | B-43 | Damage Value 27；High Reheat 1030、Cruise 490；Cruise Range 765；APG-65/2nd Gen RWR；载弹 2×AIM-9、2×AIM-120、2×AGM-84（反舰构型） |
| us_f18e | F/A-18E/F Super Hornet | 49 | B-44 | Damage Value 29；High Reheat 920、Cruise 460；Cruise Range 895（单座 E）；APG-73/APG-79；载弹 2×AIM-9、2×AIM-120、4×GBU-12 |
| us_f18e_b | F/A-18E/F Super Hornet Block III | 49–50 | B-44–45 | Damage Value 29；High Reheat 920、Cruise 460；RCS 前向 VSmall；载弹 2×AIM-9X、2×AIM-120、2×AIM-174B（2024 构型） |
| us_ea18g | EA-18G Growler | 35 | B-30 | Damage Value 30；High Reheat 920、Cruise 460；Cruise Range 765；APG-79/3rd Gen ES-ELS；仅保留 2×AIM-120（HARM 未建模，见“未建模条目”） |
| us_f35a | F-35A Lightning II | 50 | B-45 | Damage Value 24；High Reheat 920、Cruise 580；Cruise Range 970；Medium/VSmall；载弹 2×AIM-9X、2×AIM-120、2×GBU-31 |
| us_f35b | F-35B Lightning II | 51 | B-46 | Damage Value 24；High Reheat 920、Cruise 580；Cruise Range 870；载弹 2×AIM-9X、2×AIM-120、2×GBU-31 |
| us_f35c | F-35C Lightning II | 51 | B-46 | Damage Value 25；High Reheat 920、Cruise 580；Cruise Range 1020；载弹 2×AIM-9X、2×AIM-120、2×GBU-31 |
| us_e2d | E-2D Advanced Hawkeye | 33 | B-28 | Damage Value 38；Full Mil 325、Cruise 255；Cruise Range 1400；APY-9 |
| us_p8a | P-8A Poseidon | 76–77 | B-71–72 | Damage Value 45；Full Mil 490、Cruise 440；Cruise Range 4605；APY-10、126 sonobuoys；载弹 5×Mk 54（内部）、4×AGM-84D（外部） |
| us_p3c | P-3C Orion (Update III) | 76 | B-71 | Damage Value 62；Full Mil 405、Cruise 330；Cruise Range 4105；APS-115、MAD、84 sonobuoys；载弹 7×Mk 54、4×AGM-84D |
| us_mh60r | MH-60R Seahawk | 69–70 | B-64–65 | Damage Value 18；Full Mil 140、Cruise 100；Cruise Range 265；APS-147/153、AQS-22F dipping sonar、25 sonobuoys；载弹 2×Mk 54（ASW 构型；Hellfire 未建模） |
| us_mh60s | MH-60S Seahawk | 69–70 | B-64–65 | Damage Value 18；Full Mil 150（带武器 135）、Cruise 110；Cruise Range 225；仅 NVG/视觉；载弹 2×19 Hydra 70 |
| us_sh60b | SH-60B Seahawk | 84–85 | B-79–80 | Damage Value 18；Full Mil 145、Cruise 125；Cruise Range 300；APS-124、MAD、25 sonobuoys；载弹 3×Mk 46 |
| us_mv22b | MV-22B Osprey | 72 | B-67 | Damage Value 35；Full Mil 280、Cruise 240；Cruise Range 620 |
| us_cmv22b | CMV-22B Osprey | 72 | B-67 | Damage Value 35；Full Mil 280、Cruise 240；书内 **estimated** cruise range 1100 nmi、fuel 5680 kg；载弹无 |
| us_ch53e | CH-53E Super Stallion | 31–32 | B-26–27 | Damage Value 34；Full Mil 150、Cruise 135；Cruise Range 425 |
| us_ch53k | CH-53K King Stallion | 32 | B-27 | Damage Value 28；Full Mil 170、Cruise 145；Cruise Range 320 |
| us_ah1z | AH-1Z Viper | 17 | B-12 | Damage Value 17；Full Mil 158、Cruise 140；Cruise Range 260；载弹 2×AIM-9M（映射 `aam_ir`）、4×19 Hydra 70；Hellfire 未建模 |
| us_uh1y | UH-1Y Venom | 87 | B-82 | Damage Value 17；Full Mil 165、Cruise 135；Cruise Range 275；载弹 2×19 Hydra 70 |
| us_ea6b | EA-6B Prowler ICAP II/III | 35 | B-30 | Damage Value 30；Full Mil 470、Cruise 410；Cruise Range 1095；APS-130；HARM/ALQ 吊舱未建模，故无 weapons |
| us_s3b | S-3B Viking | 82–83 | B-77–78 | Damage Value 32；Full Mil 450、Cruise 350；Cruise Range 2300；APS-137、MAD、60 sonobuoys；载弹 2×Mk 46、1×AGM-84、2×Mk 82 |
| us_mq4c | MQ-4C Triton | 70–71 | B-65–66 | Damage Value 18；Full Mil 345、Cruise 310；Cruise Range 9750；ZPY-3、4th Gen FLIR/EO、4th Gen ES |
| us_mq25a | MQ-25A Stingray | 72 | B-67 | Damage Value **e17**；speed **e450**、Cruise 400；Cruise Range 2550；书内 ceiling **e15240 m**；仅 tanker/FLIR/ES |
| us_e3 | E-3 Sentry | 33 | B-28 | Damage Value 60；Full Mil 460、Cruise 430；Cruise Range 4235；APY-1/2 |
| us_c130j | C-130J Hercules | 28 | B-23 | Damage Value 65；Full Mil 355、Cruise 320；Cruise Range 2435；APN-241 |
| us_kc130j | KC-130J Super Hercules | 64 | B-59 | Damage Value 65；Full Mil 355、Cruise 320；Cruise Range 3485；2nd Gen RWR；Harvest Hawk 武器未建模 |
| us_c17a | C-17A Globemaster III | 26 | B-21 | Damage Value 100；Full Mil 500、Cruise 440；Cruise Range 3995；APS-133 |
| us_f22a | F-22A Raptor | 50 | B-45 | Damage Value 33；High Reheat 1290、Cruise 670；Cruise Range 905；APG-77；载弹 2×AIM-9X、2×AIM-120、2×GBU-32 |
| us_f15e | F-15E Strike Eagle | 44 | B-39 | Damage Value 30；High Reheat 1175、Cruise 495；Cruise Range 1430；APG-70；载弹 4×AIM-9、4×AIM-120、8×GBU-12 |
| us_f16cj | F-16CJ Fighting Falcon Block 50/52 | 47 | B-42 | Damage Value 20；High Reheat 1175、Cruise 490；Cruise Range 770；APG-68(V)5；载弹 2×AIM-9X、2×AIM-120、4×GBU-12；HARM 未建模 |
| us_a10c | A-10C Thunderbolt II | 14 | B-9 | Damage Value 37；Full Mil 350、Cruise 340；Cruise Range 865；2nd Gen RWR；载弹 2×AIM-9M（`aam_ir`）、4×GBU-12、2×7 Hydra 70 |
| us_b1b | B-1B Lancer | 19 | B-14 | Damage Value 76；High Reheat 720、Cruise 460；Cruise Range 5505；APQ-164；载弹 24×AGM-158C（反舰构型） |
| us_b2a | B-2A Spirit | 20 | B-15 | Damage Value 71；Full Mil 545、Cruise 485；Cruise Range 7480；APQ-181；载弹 16×GBU-31 |
| us_b52h | B-52H Stratofortress (1993) | 24 | B-19 | Damage Value 105；Full Mil 526、Cruise 442；Cruise Range 7175；APQ-166；载弹 8×AGM-84（反舰构型） |

## 武器条目

| id | 条目 | PDF 页 | 印刷页 | 转录值 | 估算/尺度说明 |
| --- | --- | --- | --- | --- | --- |
| us_aim9x | AIM-9X-2 Sidewinder | 106 | H4-2 | range 12 nmi、speed 1434 kts、IR | pk 0.5 / damage 20 按任务书“参照 aam_ir”的引擎尺度；书内给 ATA，不给 pk |
| us_aim120 | AIM-120 AMRAAM | 106 | H4-2 | speed 2295 kts；书内 AIM-120D range 60/10 nmi、AIM-120C 44/10 | **range 30 按任务书指定**（非书内 60/44），已在 notes 标注；pk 0.6 / damage 20 为引擎尺度 |
| us_aim174b | AIM-174B | 106 | H4-2 | range 140/10 nmi、speed 2006 kts | pk 0.6 / damage 25 为引擎尺度估算 |
| us_agm84 | AGM-84D Harpoon IC | 99 | H2-1 | range 80 nmi、speed 561 kts、damage 40+D6/2 → 40 | pk 0.55 为引擎尺度估算；targets surface only |
| us_agm158c | AGM-158C LRASM | 101 | H2-2 | range 300+ nmi、speed 540 kts、damage 46+D6 | pk 0.6 估算；targets surface only |
| us_lgby | GBU-12B Paveway II | 101 | H2-2 | range 2.5–6.5 nmi（取 6.5）、speed 528 kts、damage 35 | pk 0.7 估算；Bomb 用 `BallisticGun`，引擎将 Bomb/Rocket 归入 instant gun_salvo |
| us_gbu31 | GBU-31 JDAM (Mk 84) | 102 | H2-3 | range 13 nmi、speed 528 kts、damage 59 | pk 0.75 估算；Bomb/`BallisticGun` |
| us_gbu32 | GBU-32 JDAM | 102 | H2-3 | range 13 nmi、speed 528 kts、damage 46 | pk 0.75 估算；Bomb/`BallisticGun` |
| us_mk82 | Mk 82 500 lb bomb | 98 | H1-3 | damage 35 | range 0.8 nmi / pk 0.35 复用引擎 `bomb_mk82` 的无制导校准（estimated） |
| us_hydra70 | Hydra 70 / M151 warhead | 96、98 | H1-1、H1-3 | M151 warhead damage 9 | range 3 / pk 0.25 / salvo 4 复用引擎 `rocket_57` 校准（estimated） |
| us_mk46 | Mk 46 Mod 5A(SW) | 93 | F-1 | range 8.0 nmi @ 30 kts（also 6.0 @ 45）、damage 52 | enable 0.8 nmi / pk 0.55 估算 |
| us_mk54 | Mk 54 | 93 | F-1 | range 8.0 nmi @ 30 kts（also 6.0 @ 45）、damage 52 | enable 0.8 nmi / pk 0.6 估算 |

## 传感器

| id | 条目 | 出处 | 说明 |
| --- | --- | --- | --- |
| us_sonobuoy | Sonobuoy Field | PDF 69–70/B-64–65（MH-60R 25 枚）、76/B-71（P-3C 84 枚、P-8A 126 枚） | `SonarPassive`；sub_range 8 nmi 为 **estimated**（书内只给阵位数/型号，不给探测距离） |
| us_mad | Magnetic Anomaly Detector | PDF 76/B-71（P-3C）、84/B-79（SH-60B） | `Magnetic`；sub_range 2 nmi 为 **estimated**（书内只列 MAD，不给距离） |

复用内置通用传感器（未在本卷定义）：`radar_air_med`、`radar_air_long`、
`radar_air_xlong`、`radar_surf_med`、`radar_surf_sm`、`radar_nav`、`esm_std`、
`visual`、`sonar_dipping`。语义映射：

- `radar_air_med` ← APG-65/73/79/81/68(V)5/70/77（J3 PDF 113–114 页，Lge 160–180 nmi）。
- `radar_air_long` ← APY-10（J3 PDF 117 页，Lge 200 nmi）、APS-115（Lge 150 nmi）。
- `radar_air_xlong` ← APY-9（J3 PDF 117 页，Lge 525 nmi）、APY-1/2（Lge 350 nmi）。
- `radar_surf_med` ← APS-137/147/153、APY-10 SS 模式、APQ-164/181/166 等。
- `radar_surf_sm` ← APS-124、APN-241。
- `esm_std` ← 书内 ES/ELS/RWR；`visual` ← 目视/FLIR/NVG；`sonar_dipping` ← AQS-22F。

内置通用传感器自身的 `alt_m`（radar_air_med 25 m 等）是引擎既有的舰载/机载
共用抽象；本卷按任务书要求复用，未另造机载高度版本。

## estimated 值清单（不在书内逐项印出的字段）

- **endurance_s（全部平台）**：书内给 Cruise Range（nmi）与 Cruise/Full Mil 速度，
  不印“留空小时”。本卷用 `Cruise Range ÷ cruise_speed_kts × 3600` 推导，
  已逐条四舍五入到秒。该值与“航程全程按巡航速度”的假设绑定。
- **turn_rate_deg_s（全部平台）**：书内只有 Maneuver Rating（如 4.5/1.5），
  没有 deg/s。按机型（战斗机 14–20、攻击机 10、直升機 20–30、运输/预警 4–5、
  轰炸机 3–5）取引擎尺度估算。
- **vp_value（全部平台）**：书内无 VP；按角色/体量给引擎尺度值（战斗机 14–22、
  直升机 7–10、巡逻机 20–22、预警 24–25、轰炸机 28–32、运输机 14–20）。
- **chaff/decoys（全部平台）**：书内只给 J&D 代数（如 3rd Gen J&D），不给弹数；
  本卷按代数/体量给引擎尺度的挂载数。无 Counterm 的条目填 0。
- **rcs**：`Stealthy` → `VerySmall`（引擎枚举无 Stealthy）；书内“front-arc
  VSmall”简化为整机 `VerySmall`（F/A-18E/F Block III、EA-18G、F-16CJ、F-35 等）。
- **visual_factor/noise_db**：未设置，使用 `PlatformClass` 默认值（1.0/50）。
- **武器 pk / enable_range / reload_s**：书内给 ATA/伤害/射程/速度，不给引擎的
  `pk`、`enable_range_nm`、`reload_s`。除任务书指定的 AIM-9X/AIM-120 尺度外，
  其余按既有 `aam_ir`/`ascm_std`/`lwt`/`bomb_mk82`/`rocket_57` 校准取值。
- **us_aim120 range 30**：任务书明确指定（pk 0.6、range 30）；书内 H4-2 的
  AIM-120D 为 60/10 nmi、AIM-120C 为 44/10 nmi。此处遵从任务书，**不是**书内值。
- **CMV-22B 1100 nmi**：书内 p.72/B-67 明标 estimated（“Estimated fuel 5680 kg,
  cruise range 1100 nmi”）。
- **MQ-25A**：书内 hp `e17`、max speed `e450`、ceiling `e15240 m`、fuel `e8000 kg`
  均带 `e`（estimated），本卷照录并保留 “est.” 标记。
- **us_sonobuoy 8 nmi / us_mad 2 nmi**：estimated（见上表）。
- **载弹 `mounts × ammo`**：`crates/harpoon-core/src/scenario.rs:129` 在生成
  `WeaponMount` 时执行 `ammo: w.ammo * w.mounts as u16`。因此本卷按
  “mounts = 典型挂点数、ammo = 每挂点弹数”填写，使**实际总弹数 = 任务书典型
  挂载数**。例如 us_f18c 的 `us_aim120 {mounts:2, ammo:1}` = 2 枚。

## 未建模条目（明确省略，不是数据缺失）

- **AGM-88 HARM / AARGM**：引擎没有反辐射制导类；若强行归入 `Ascm` 会与
  “SeaSkimmer 反舰”语义冲突。EA-18G、EA-6B、F-16CJ 因此未带 HARM。
- **AGM-114 Hellfire / AGM-176 Griffin / JAGM**：引擎没有空对地导弹（direct-attack）
  制导类，只有反舰 `Ascm`；为避免语义错误，AH-1Z、MH-60R、MH-60S、KC-130J
  的 Hellfire 条目未录入，保留火箭巢/鱼雷。
- **JASSM / JSM / SLAM / JSOW / SDB / 核炸弹 / 水雷 / 深弹 / 吊舱**：未录入；
  本卷只覆盖可映射到 `Aam`/`Ascm`/`Bomb`/`Rocket`/`Lwt` 的典型反舰/对地/反潜挂载。
- **电子战吊舱（ALQ-99/249 等）**：引擎无干扰吊舱实体；EA-18G/EA-6B 只保留
  自卫 AAM（EA-18G）或无武器（EA-6B）。
- **AIM-9M/AIM-7/AIM-54**：本卷仅在 AH-1Z/A-10C 用通用 `aam_ir` 代表 AIM-9M；
  AIM-7/AIM-54 未建模（老旧型号，未列入 34 类核心现代机队）。

## 数据书未覆盖 / 无法核对的项

- 书内没有逐平台的“留空小时”“turn rate”“VP”“chaff 弹数”，这些均为
  引擎尺度估算（见上）。
- 书内 J3/H2/H4/F 表格给的是逐目标尺寸/逐载荷的细节；本卷按任务书要求映射到
  引擎通用传感器与武器类别，未逐项建立 34 套专用雷达/挂载。
- `us_aim120` 的 30 nmi 是任务书指定值，与书内 60/44 nmi 不一致；如果后续要
  严格按书，可改为 60（AIM-120D）或 44（AIM-120C-7）。

## 2026-10-22 调整：us_f35c AIM-120 载弹量

`us_f35c` 的 AIM-120 由每挂点 1 枚改为每挂点 2 枚（总 4 枚），对应 F-35
标准的 4 枚内埋 BVR 弹空优构型；其余字段不变。这是虚构 2026 年载弹假设，
非数据书原文（原书仅列 2×AIM-120）。同步应用于
`database/consolidated.json`。改动动机：近岸航母编队战（csg_littoral）
平衡校准，见该剧本与 `us_aircraft_mission_fits.provenance.md` 同日记录。
