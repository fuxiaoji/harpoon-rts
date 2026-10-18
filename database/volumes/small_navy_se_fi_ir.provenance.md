# small_navy_se_fi_ir — 转录出处表

三卷合录：瑞典、芬兰、伊朗。平台 30 类、武器 36 项、传感器 47 项。

## 数据书与页码约定

| 标签 | 文件名 | 实际刊期 | PDF 页数 | 文本抽取 |
| --- | --- | --- | --- | --- |
| SE | 27.（数据库）瑞典海军2025（NSITREP_63_13_Nov_25）.pdf | Naval SITREP #63, **October 2022** | 32 | `/tmp/pdftext/pdftext`（PDFKit）→ `/tmp/se/pNNN.txt` |
| FI | 28.（数据库）芬兰海军2023NSITREP_64_5_Aug_23.pdf | Naval SITREP #64, **April 2023** | 32 | `/tmp/pdftext/pdftext` → `/tmp/fi/pNNN.txt` |
| IR | 29.（数据库）伊朗海军2025（Naval_SITREP_65_(October_2023)）.pdf | Naval SITREP #65, **October 2023** | 32 | `/tmp/pdftext/pdftext` → `/tmp/ir/pNNN.txt` |

- 页码均为 **PDF 文件页码**；版心印刷页码 = PDF 页码 − 1（例如 PDF p.3 的版心为 Page 2）。
- SE 文件名含 “2025/13 Nov 25”，IR 文件名含 “2025”，但 PDF 封面与版权页显示的实际刊期分别为 **#63 October 2022** 与 **#65 October 2023**；本表按 PDF 实际刊期记录。
- 抽取命令（三份均执行成功，各返回 `pages=32`）：
  `/tmp/pdftext/pdftext "<pdf>" /tmp/{se|fi|ir}`。逐页文本保存为 `p001.txt` … `p032.txt`。

## 平台条目（30）

| id | 名称 | 卷 | PDF 页 | 取数说明 |
| --- | --- | --- | --- | --- |
| se_ffg_visby | Visby-class Corvette (FFL) | SE | 3 | hp 57 = 末档 Dam Pts；40 kts = Surf Speed 首值；RBS 15 为 vice ROV 的可选舷侧 2×4 构型 |
| se_ss_a19 | Gotland (A19) SSP | SE | 3 | hp 58（MLU 档 59，未取）；Subm Speed 首值 20；Max Depth Int IV；Tp 62/Tp 45 为现役弹 |
| se_ss_a17s | Södermanland (A17S) SSP | SE | 3 | hp 56 = Dam Pts (17S) 末值；Sub Spd (17S) 首值 19；Östergötland 2021 年除役，未单列 |
| se_pb_stockholm | Stockholm-class PB (Spica III, 2015 fit) | SE | 4 | hp 39；32 kts；2015 年改装后 400mm TT 撤除、RBS 15 减至 4 枚 |
| se_ff_gtb | Göteborg-class FFL (Gävle MLU) | SE | 4 | hp 38；32 kts；Gävle/Sundsvall 2017–22 MLU 构型（Mk3 57mm、Sea Giraffe AMD、ST2400） |
| se_mhc_landsort | Koster/Landsort-class MHC | SE | 5 | hp 35；15 kts；Koster MLU 后撤 ASW-600、保 40mm Sea Trinity |
| se_helo_hkp15b | A.109M Hkp 15B ASW/Patrol (ceiling 5970 m) | SE | 7 | Damage Value 12；167/154 kts；265 nmi；16 枚 SSQ-906 声呐浮标 |
| se_helo_hkp14f | NH90 Hkp 14F SAR/ASW (ceiling 3300 m) | SE | 8 | Damage Value 21；160/140 kts；345 nmi；FLASH-S 吊放声呐、50 枚 SSQ-906H、2 枚 Tp 47（2025） |
| se_air_jas39c | JAS 39C/D Gripen Fighter (ceiling 15240 m) | SE | 7 | Damage Value 19；1033 kts = High/Reheat；450 kts = Low/Cruise；620 nmi |
| se_air_s100b | S 100B Argus AEW (ceiling 7620 m) | SE | 9 | Damage Value 25；285 kts；985 nmi；PS-890 Erieye |
| fi_ffg_pohjanmaa | Pohjanmaa-class Corvette (FFL, 2027) | FI | 3 | hp 221；26 kts；2027 年计划成军（0+0+4）；32 ESSM、8 Gabriel V、Tp 47 |
| fi_pgg_hamina | Hamina-class PGG (post-MLU) | FI | 3 | hp 26；32 kts；按 2018–22 MLU 后构型：Gabriel V、Mk4 40mm、Trackfire、Umkhonto、Tp 45 |
| fi_pgg_rauma | Rauma-class PGG | FI | 3 | hp 23；30 kts；2010–13 后构型：RBS 15 Mk3、ST2400、MASS、Sako/Mistral 已撤 |
| fi_mm_hameenmaa | Hämeenmaa-class Minelayer (MLU) | FI | 4 | hp 64；20 kts；2006–08 MLU 后：57mm、Umkhonto、TRS-3D、直升机甲板 |
| fi_mhc_katanpaa | Katanpää-class MHS | FI | 4 | hp 29；13 kts；40mm、HMS-12M/Klein 5500、MSP-500 |
| fi_air_f18c | F-18C/D Hornet Fighter (ceiling 15240 m) | FI | 7 | Damage Value 27；1032 kts = High/Reheat；490 kts = Low/Cruise；765 nmi；2 AIM-9 + 6 AMRAAM |
| fi_helo_nh90 | NH90 Transport Helicopter (ceiling 3300 m) | FI | 8 | Damage Value 18；157/140 kts；370 nmi；无武装运输型 |
| ir_kilo | Project 877EKM Kilo SS | IR | 21 | hp 89；Subm Speed 首值 17；Max Depth Int IV；6 TEST-71ME + 12 53-65KE |
| ir_ff_moudge | Moudge/Jamaran-class FFL | IR | 22 | hp 108；30 kts；Noor（4）、Mehrab（4）、Fajr-27、Fath-40、GAM-B01、ET-52 |
| ir_ss_besat | Besat-class SS (2025) | IR | 21 | hp 51；Subm 15；Max Depth Int III；6+ YT-534 或 Jask-2（本录按鱼雷构型） |
| ir_ss_fateh | Fateh-class SSM | IR | 21 | hp 30；Subm 14；Max Depth Int III；4 YT-534 或 Jask-2 |
| ir_ss_ghadir | Ghadir-class SSM | IR | 21 | hp 11；Subm 8；Max Depth Int II(e)；2 YT-534 或 Jask-2 |
| ir_ff_saam | Alvand/Saam-class FF | IR | 21-22 | hp 84；39 kts；1996 年后 C802/Noor 构型（Sea Killer/Sea Cat 已撤） |
| ir_ptg_sina | Sina-1-class PTG | IR | 23 | hp 34；36 kts；4 Noor、Fajr-27、Fath-40 |
| ir_wptg_zolfaghar | Zolfaghar (Peykaap III) WPTG | IR | 26 | hp 4.4；52 kts；2 Nasr、2 Doshka |
| ir_ptg_c14 | C-14 Catamaran PTG | IR | 22-23 | hp 4.5；50 kts；4 Nasr（TL-10A/C704 后替换）；前炮按书内 “可能为 Doshka” 取 Doshka |
| ir_wptg_houdong | Type 021 Houdong WPTG | IR | 27 | hp 27；35 kts；2019 年 Qader 替换 Noor |
| ir_wffl_soleimani | Shahid Soleimani-class WFFL | IR | 25 | hp 30；32 kts；16 Mehrab、6 Abu-Mahdi、4 Kamand |
| ir_lst_hengam | Hengam-class LST | IR | 23 | hp 118；14 kts；4 ZU-23、4 40mm、2 Doshka；可载 300 t 燃油/600 t 货物 |
| ir_esb_makran | Makran (ex-Al Buhaira) ESB | IR | 24 | hp 515；14 kts；4 ZU-23、2 Doshka；6–7 直升机甲板 |

## 武器条目（36）

| id | 名称 | 卷 | PDF 页 | 取数说明 |
| --- | --- | --- | --- | --- |
| se_57mk3 | Bofors Mk3 57mm/70 | SE | 10 (Annex C) | 3.5 kyd 极限 → 1.73 nmi；Damage 10 为极限档；AA 0.69 |
| se_gun40 | Bofors 40mm/70 SAK 315/350 | SE | 10 (Annex C) | 2.5 kyd → 1.23 nmi；Damage 4 |
| se_mg50 | M2 .50 cal Machine Gun | SE | 10 (Annex C) | 1.0 kyd → 0.49 nmi；Damage 0.5–0.8，取 1 |
| se_elma | Elma ASW 600 Rocket Launcher | SE | 10 (Annex E3) | 0.1–0.3 kyd；9×100mm；Damage 9；Contact 0.08 |
| se_rbs15 | RBS 15 Mk2 Anti-Ship Missile | SE | 10 (Annex D1) | 54 nmi；594 kts；42+D6/2；VLow cruise |
| se_rbs15f | RBS 15F Air-Launched Anti-Ship Missile | SE | 11 (Annex H2) | 37.8 nmi 巡航 / 80 nmi 直接；594 kts；39 damage |
| se_tp62 | Tp 62 Heavy Torpedo | SE | 10 (Annex F) | 13.8 nmi @55 kts；145/102 damage；Dual-Wire/3 |
| se_tp45 | Tp 45 400mm Torpedo | SE | 10 (Annex F) | 5 nmi @35 kts（11 @25、16 @11）；50/50 damage |
| se_tp47 | Tp 47 400mm Torpedo | SE | 10 (Annex F) | 10.8 nmi @40 kts（20 @10）；64/64 damage |
| se_amraam | AIM-120B AMRAAM | SE | 11 (Annex H4) | 40/10 nmi；2295 kts；damage 20 为标准化估算（弹头 DP 未印） |
| se_bk27 | Mauser BK27 27mm | SE | 11 (Annex H3) | ATA 2.0；Damage Points 2.9，取 3 |
| fi_mg127 | NSV 12.7mm Machine Gun | FI | 9 (Annex C) | 1.0 kyd；Damage 0.6–0.9，取 1 |
| fi_umkhonto | Umkhonto IR Block III SAM | FI | 9 (Annex D) | 10.8 nmi；1323 kts；damage 20 为标准化估算 |
| fi_essm | ESSM Block 2 SAM | FI | 3 | 仅舰上装载表（32 发）；弹道数据不在 FI 附录，按美制 ESSM 量为 27 nmi/2250 kts/25（估算，见 us_navy.json 对照） |
| fi_gabriel | Gabriel V Anti-Ship Missile | FI | 9 (Annex D) | 108+ nmi；429 kts；41+D6/2 |
| fi_amraam | AIM-120C-7 AMRAAM | FI | 7 | F-18 挂载表；44/10 nmi；2295 kts；damage 20 标准化估算 |
| fi_vulcan | M61 Vulcan 20mm | FI | 7 | F-18 Off Guns；射程/伤害为估算（书未给弹道表） |
| ir_nasr | Nasr-1 Anti-Ship Missile | IR | 28 (Annex D) | 4.3–18.9 nmi；561 kts；33+D6/2 |
| ir_noor | Noor Anti-Ship Missile | IR | 28 (Annex D) | 8.1–64.8 nmi；594 kts；36+D6/2 |
| ir_qader | Qader Anti-Ship Missile | IR | 28 (Annex D) | 5.0–162 nmi；594 kts；39+D6/2 |
| ir_mehrab | Mehrab SAM | IR | 28 (Annex D) | 空/面 1.0–17.5 nmi；1240 kts；4 damage；SARH/2 |
| ir_jask2 | Jask-2 Submarine-Launched Anti-Ship Missile | IR | 28 (Annex D) | 4.3–18.9 nmi；561 kts；33+D6/2 |
| ir_abu_mahdi | Abu-Mahdi Land-Attack Cruise Missile | IR | 28 (Annex D) | 5–540 nmi；561 kts；14 damage；I&Sat/TARH/4 |
| ir_yt534 | YT-534-UW1 Valfajr Torpedo | IR | 28 (Annex F) | 8.1 nmi @35 kts；79 damage；Pass/Wake/3 |
| ir_test71 | TEST-71ME Torpedo | IR | 28 (Annex F) | 8.2 nmi @40 kts（4.5 @31）；89 vs sub；Wire-G/2 |
| ir_53_65 | 53-65KE Wake-Homing Torpedo | IR | 28 (Annex F) | 10.4 nmi @45 kts；150 damage；Pass/Wake/2 |
| ir_et52 | ET-52 Torpedo | IR | 28 (Annex F) | 3.3 nmi @30 kts；47 damage；Act/Pass/2 |
| ir_fajr27 | Fajr-27 76mm/62 | IR | 28 (Annex C) | 14 kyd → 6.9 nmi；Damage 11–17，取 12 |
| ir_gun40 | 40mm/70 Gun (Fath-40 / Type 564) | IR | 28 (Annex C) | 2.5 kyd → 1.23 nmi；Damage 3–5，取 4 |
| ir_gamb01 | GAM-B01 20mm/85 | IR | 28 (Annex C) | 1.4 kyd → 0.69 nmi；Damage 1.8–3，取 2 |
| ir_mk8 | Mk8 4.5in/55 | IR | 28 (Annex C) | 24 kyd → 11.8 nmi；Damage 14–21，取 17 |
| ir_gdmc | GDM-C 35mm/90 | IR | 28 (Annex C) | 1.9 kyd → 0.94 nmi；Damage 3–4，取 3 |
| ir_kamand | Kamand 30mm | IR | 28 (Annex C) | 1.9 kyd → 0.94 nmi；Damage 3–5，取 3 |
| ir_type69 | Type 69 30mm/65 | IR | 28 (Annex C) | 1.9 kyd → 0.94 nmi；Damage 2–4，取 2 |
| ir_zu23 | ZU-23 23mm/81 | IR | 28 (Annex C) | 1.4 kyd → 0.69 nmi；Damage 1.9–3，取 2 |
| ir_doshka | Doshka 12.7mm | IR | 28 (Annex C) | 1.0 kyd → 0.49 nmi；Damage 0.5–0.8，取 1 |

## 传感器条目（47）

| id | 名称 | 卷 | PDF 页 | 取数说明 |
| --- | --- | --- | --- | --- |
| se_giraffe_amb | Sea Giraffe AMB 3D Radar | SE | 12 (Annex J1) | 3D/LAS 95/67/48/19/6；SS/GFC 40/38/25/13/6 |
| se_giraffe50 | Sea Giraffe 50HC Radar | SE | 12 (Annex J1) | AS 51/36/25/10/3；SS 30/28/16/9/5 |
| se_scanter2001 | Scanter 2001 Radar | SE | 12 (Annex J1) | SS 48/35/20/11/6；air 20 为估算 |
| se_9gr600 | 9GR600 (9GA209) Radar | SE | 12 (Annex J1) | LAS 24/20/14/6/1.7；SS 36/35/20/11/6 |
| se_subfar | SUBFAR 100 Periscope Radar | SE | 12 (Annex J1) | LAS 15/11/8/3/0.9；SS 27/27/16/9/5 |
| se_sonar092 | Sonar 092/SUBAC Submarine Sonar | SE | 13 (Annex K1) | 被动 3.0/3.4；主动 2.6/3.2；取被动 3.4 |
| se_sonar082 | Sonar 082/SUBAC Submarine Sonar | SE | 13 (Annex K1) | 被动 2.6/3.4；主动 2.6/3.2；取被动 3.4 |
| se_cvds26 | CVDS-26 Variable Depth Sonar | SE | 13 (Annex K1) | 主动 1.7 |
| se_hydro_tas | Hydrosciences Towed Array | SE | 13 (Annex K1) | 被动 4.5 |
| se_ss304 | SS304 Spira Hull Sonar | SE | 13 (Annex K1) | 主动 1.3 |
| se_st2400 | ST2400 Variable Depth Sonar | SE | 13 (Annex K1) | 主动 1.9 |
| se_hms12m | HMS-12M Minehunting Sonar | SE | 5 | Landsort MLU 提及；距离 1.0 为估算 |
| se_ps890 | PS-890 Erieye AEW Radar | SE | 12 (Annex J3) | 3D 242/242/200/80；SS 190/106/60/34/19 |
| se_ps05 | PS-05/A Mk2 Radar | SE | 12 (Annex J3) | LD/SD 80/80/58/23/7；SS 80/80/46/26/14 |
| se_irst | IRST / EO Director | SE | 3 | Visby 3rd Gen IRST；距离为估算 |
| se_flir | 3rd Gen FLIR | SE | 7-8 | Hkp 15B/Hkp 14F；距离为估算 |
| se_aps143b | APS-143B Ocean Eye Radar | SE | 12 (Annex J3) | SS 178/100/57/32/18 |
| se_flashs | FLASH-S Dipping Sonar | SE | 13 (Annex K2) | 主动 3.0、被动 1.0 |
| se_sonobuoy906 | SSQ-906 Sonobuoy | SE | 13 (Annex K2) | 被动 1.4–1.5 |
| se_sonobuoy906h | SSQ-906H Sonobuoy | SE | 13 (Annex K2) | 被动 1.5 |
| se_primus701 | Primus 701A Weather/Patrol Radar | SE | 12 (Annex J3) | SS 200/118/67/38/21；本录取 Small 列 67，air 0 |
| fi_trs3d | TRS-3D/16 3D Radar | FI | 9 (Annex J) | 3D 68/47/34/14/4；SS 50/32/18/10/6 |
| fi_sonac_dts | Patria Sonac DTS Dual-Tow Sonar | FI | 3 | 舰上装备；距离 3.0 为估算 |
| fi_ss2030 | SS2030 Hull Sonar | FI | 3 | 舰上装备；距离按 SE Annex K1 同型 1.9 |
| fi_st240 | ST240 Toadfish VDS | FI | 3 | 舰上装备；距离按 SE Annex K1 ST2400 1.9 |
| fi_sonac_pta | SONAC/PTA Towed Passive Array | FI | 3 | 舰上装备；距离 2.5 为估算 |
| fi_eoms | EOMS EO/IR Director | FI | 3 | 距离为估算 |
| fi_msp500 | MSP-500 EO/IR Director | FI | 4 | 距离为估算 |
| fi_hms12m | HMS-12M Minehunting Sonar | FI | 4 | 距离 1.0 为估算 |
| fi_klein5500 | Klein 5500 Towed Sidescan Sonar | FI | 4 | 距离 0.5 为估算 |
| fi_sd9500 | SD 9500 Dipping Sonar | FI | 3 | 距离 2.0 为估算 |
| fi_giraffe4a | Sea Giraffe 4A Radar | FI | 9 (Annex J) | 3D 151/151/108/43/13 |
| fi_giraffe1x | Sea Giraffe 1X Radar | FI | 9 (Annex J) | 3D 32/23/16/6/1.9 |
| fi_apg73 | APG-73 Radar | FI | 7 | F-18 装备；距离 80/60 为估算 |
| fi_primus701 | Primus 701A Weather/Patrol Radar | FI | 8 | NH90 装备；距离按 SE Annex J3 Small 列 67 |
| fi_flir | 3rd Gen FLIR | FI | 8 | 距离为估算 |
| ir_asr | Asr 3D PESA Radar | IR | 22 | Moudge 装备；IR 卷无雷达附录，距离全部估算 |
| ir_aws1 | AWS-1 Air/Surface Radar | IR | 21-22 | Saam/Moudge 装备；距离估算 |
| ir_mvr88 | MVR-88 Navigation Radar | IR | 22 | 距离估算 |
| ir_ess1 | ESS-1 Hull Sonar | IR | 22 | 距离估算 |
| ir_mrk50 | MRK-50 Kaskad Periscope Radar | IR | 21 | Kilo 装备；距离估算 |
| ir_mgk400 | MGK-400 Rubikon Sonar | IR | 21 | Kilo 装备；距离估算 |
| ir_mgk519 | MGK-519 Arfa Sonar | IR | 21 | Kilo 装备；距离估算 |
| ir_fateh_sonar | Fateh/Besat Submarine Sonar | IR | 21 | 距离估算 |
| ir_hafez | Hafez Air/Surface Radar | IR | 25 | 距离估算 |
| ir_type756 | Type 756 Navigation Radar | IR | 23 | 距离估算 |
| ir_sr47a | SR-47A Navigation Radar | IR | 27 | 距离估算 |

## estimated 值清单（按字段）

### 平台

- **`cruise_speed_kts`（全部平台）**：数据书只印 Surf/Subm Speed 档位，不印巡航速度。按同级舰/机常用巡航值取 50%–70% 最大速；全部为估算。
- **`turn_rate_deg_s`、`noise_db`、`vp_value`（全部平台）**：数据书不提供引擎字段，均为按吨位、推进、静音特征归一化的估算。
- **`rcs`**：按数据书的 Size Class / Signature 映射；潜艇一律 VerySmall，Visby/A19/A17S 等 VSmall/Stealthy 取 VerySmall。
- **`visual_factor`**：潜艇 0.15；水面舰/飞机 1.0（估算）。
- **`chaff` / `decoys`**：仅在有 MASS / 3rd Gen D 记载时设置。Visby、Göteborg、Stockholm、Hamina、Rauma、Hämeenmaa 20/20（Hamina 10/10），Moudge、Saam、Soleimani 10/10；数量为估算，记载仅说明具备该能力。
- **`endurance_s`（飞机）**：按印出的 Cruise Range ÷ Cruise Speed × 3600 计算（派生值）：
  - se_helo_hkp15b 265/154 → 6195 s
  - se_helo_hkp14f 345/140 → 8871 s
  - se_air_jas39c 620/450 → 4960 s
  - se_air_s100b 985/160 → 22162 s
  - fi_air_f18c 765/490 → 5620 s
  - fi_helo_nh90 370/140 → 9514 s
- **`max_speed_kts`（飞机）**：取表内 High/Reheat 或最大 Full Mil（直升机取 Full Mil）；`cruise_speed_kts` 取 Low/Cruise。
- **`max_speed_kts`（舰/艇）**：取 Surf Speed 或 Subm Speed 档位首值（无损）。潜艇：A19 20、A17S 19、Kilo 17、Besat 15、Fateh 14、Ghadir 8。
- **`max_depth_m`**：数据书只给 Int 等级；Int IV→300 m、Int III→200 m、Int II→150 m（按既有卷同等级换算，非原书数值）。
- **`hp`**：取 Dam Pts 末值；A19 取 58（未取 MLU 59）；A17S 取 17S 档 56。

### 武器

- **fi_essm**：FI 卷只在 Pohjanmaa 舰上列 “32 RIM-116 ESSM Blk 2”，无弹道表。采用 27 nmi / 2250 kts / 25 damage（美制 ESSM 标准量，估算）。
- **fi_amraam**、**se_amraam**、**fi_umkhonto**：射程/速度按书中挂载表或附录；**damage 20 为标准化估算**（书中未印战斗部 DP）。
- **fi_vulcan**：FI 卷只给 “20mm Vulcan (3.7)” 的 ATA；射程 1.0 nmi、damage 3 为估算。
- **se_bk27**：书中 ATA 2.0、Damage Points 2.9；damage 取 3。
- **se_mg50 / fi_mg127 / ir_doshka**：小口径机枪，伤害按书中 0.5–0.9 取整为 1（估算）。
- 其余舰载/潜射武器：射程、速度、damage、salvo、reload 均按 SE/FI/IR 的 Annex C/D/F 表；未印的 `min_range_nm`、`reload_s`、`salvo` 按同型武器常用值补齐（估算）。

### 传感器

- **伊朗全部传感器**：IR 卷没有雷达/声呐附录（只有 Annex C/D/F）；所有 `air/surface/sub/missile_range_nm` 均为估算，名称/型号来自舰艇条目。
- **估算距离的瑞典/芬兰传感器**：`se_scanter2001` 的 air 20、`se_irst`、`se_flir`、`se_hms12m`、`fi_sonac_dts`、`fi_sonac_pta`、`fi_eoms`、`fi_msp500`、`fi_hms12m`、`fi_klein5500`、`fi_sd9500`、`fi_apg73`、`fi_flir`。
- **fi_ss2030 / fi_st240**：距离取自 SE Annex K1 的同型 SS2030 / ST2400（跨卷同型对照，非 FI 卷原表）。
- **se_primus701 / fi_primus701**：SE Annex J3 只有 SS 行 200/118/67/38/21（Large/Med/Small/VSmall/Stealthy）；本录取 Small 列 67，air 0（该雷达为 SS/气象功能）。这是目标尺寸档的选择，不是新增数据。
- **se_giraffe_amb 用于 Göteborg 的 “Sea Giraffe AMD”**：AMD 无单独附录行，按 AMB 近似。
- **se_9gr600**：按 Annex J1 的 9GA209（LAS + SS）行，用于 Koster MLU。

### 构型与合并说明

- **se_ss_a17s**：A17 级原有 Västergötland、Hälsingland、Södermanland、Östergötland；前两艘 2005 年售新加坡，Östergötland 2021 年除役，仅 Södermanland 在役。按 “同型多舷合并” 记为 A17S 一类，名称用 Södermanland，Östergötland 不另列。
- **se_ffg_visby**：RBS 15 为 “vice ROV” 的可选舷侧 2×4 构型；本录按打击构型录入。MLU 提到的 RBS 15 Mk3 尚未确认，按 Mk2（54 nmi）录入。
- **se_ff_gtb**：Göteborg、Kalmar 已除役，Gävle、Sundsvall 在 2017–22 MLU 后仍在役；按 Gävle 构型合并。
- **se_pb_stockholm**：2000–02 改装为 Malmö 构型（57mm Mk3、撤 40mm/Elma、400mm TT）；2015 年撤 400mm TT、RBS 15 减至 4 枚。
- **fi_pgg_hamina**：按 2018–22 MLU 后全部四艘统一构型；2023–25 的 Tp 47 换装尚未完成，按 Tp 45 录入。
- **fi_ffg_pohjanmaa**：刊期为 2023，Pohjanmaa 为 0+0+4 计划舰（2027 成军），按计划构型录入。原书舰上条目写作 “RIM-116 ESSM Blk 2”（型号混写），本录按 ESSM 处理。
- **ir_mehrab**：原书列在 Annex D（面导弹）但为 Sayyad-2 的舰载防空型，本录按 SAM 处理，同时保留书中的对海射程/伤害。
- **ir_ff_moudge**：Moudge/Jamaran 同级；Noor/Qader 取 Noor，Sahand 的 Kamand 30mm 替代 Fath-40 作为变体合并，未单列。
- **ir_kilo**：Tareq/Nuh/Yunes；2015–19 起部分艇可发射 Qader，本录仍按 TEST-71ME + 53-65KE 基线。
- **ir_ff_saam**：Saam/Zaal/Rastam/Faramarz → Alvand/Alborz/Sabalan/Sahand；按 1996 年后 C802/Noor 构型，Sea Killer/Sea Cat 已撤。
- **ir_ss_besat / ir_ss_fateh**：均可 “鱼雷或 Jask-2”，本录取鱼雷构型；Jask-2 能力在 provenance 说明，不在 weapons 列表重复计入。
- **ir_ptg_c14**：TL-10A 后期换 C704/Nasr，按 Nasr 录入。
- **ir_wptg_houdong**：2019 年 Qader 替换 Noor，按 Qader 录入。
- **ir_wffl_soleimani**：16 Mehrab、6 Abu-Mahdi 为 VLS 单元数；4×Kamand 30mm 与 1×Type 69 30mm 为书内估计枪型。

## 未收录的源内条目

为控制规模，以下同一批数据书中可读到的条目未录入（不是读不到）：Nahang SSM、Ex-US PGM-71、Ex-US PF-103、French Combattante IIB、Fouque LSL、Arya Sahand LSM、Karbala LCU、Delvar AE、Khalij-e Fars AG、Hendijan AG、Bandar Abbas AOR、PRC Type 082、Ex-US Cape MSC、Seraj-1/Cougar/Taregh/Ashura/Kuch/Tir/Zoljanah/New Interceptor/Zulfaghar AD/Bavar 等小型艇；瑞典 Tapper、Dalarö、Styrsö、Carlskrona、SAM 01、662/601/CB90/Trossö/Orion/Belos；芬兰 Isku、Pansio、Kuha、Kiiski、Utö、Uisko、Meriusko、Vietivisko、Kampela、Halli、Hylje、Valas、Turva、Tursas、Merikarhu、TDX/SAH-2000 及 AB.412、AS 332、AW119、C-295、Do 228、Hawk、Learjet、MD 500、PC-12、Ranger。上述条目均已在对应 PDF 页读到，但本卷按任务规模目标（≥15 类）选择性录入 30 类。
