# commonwealth_navies.json — 数据出处与估算说明

## 数据书

- 主卷：`17-1.Commonwealth_Navies_1_Dec_25.pdf`（112 页；本文页码写作「PDF p.X / 印刷页 A-Y」）。
  PDF 页与印刷页的对应关系由卷内页眉核对：PDF p.33 = A-29，PDF p.48 = A-44，PDF p.62 = A-58。
- 飞机补充：`16-1.Commonwealth_Aircraft_1_Dec_25.pdf`（99 页；「PDF p.X / 印刷页 B-Y」）。
  17-1 的 Annex R 只给航母航空联队编成，不给飞机性能；为满足本卷「含舰载航空联队固定翼/直升机类」的要求，
  飞机条目取 16-1 的 Annex B 数据，出处逐条列出。
- 抽取方式：`sqlite3 -readonly 鱼叉/resources/rag/harpoon_rules.sqlite3` 按 `chunks.document_id`、`page`、
  `chunk_id` 顺序拼接逐页文本；缺页尾时用 `/tmp/pdftext/extract`（PDFKit `PDFPage.string`）复核
  （例如 River Batch 2 的 DP 位于 PDF p.56 页首、Canterbury 的 DP 位于 PDF p.33 页首）。

## 转录规则

- `hp`：舰船/潜艇取该级 `Dam Pts` 行的最后一个数值（100% 行）；飞机取 16-1 的 `Damage Value`。
- `max_speed_kts`：取无损状态第一档 `Surf Speed` / `Subm Speed`；飞机取 `Full Mil`/`Reheat` 档。
- `rcs`：取 `Signature:` 第一个词。`Large→Large`、`Medium→Medium`、`Small→Small`、
  `VSmall→VerySmall`、`Stealthy→VerySmall`。飞机按 16-1 `Size/Signature` 的第二项。
- `noise_db`：数据书只给声学代次/等级（Loud/Noisy/Quiet/VQuiet/EQuiet），不给 dB；按引擎既有标尺换算
  （Loud=62；Noisy 按舰体大小取 54/56/58；Quiet 取 42/44/46；VQuiet=30；EQuiet=22），属于 **estimated**。
- `cruise_speed_kts`、`turn_rate_deg_s`、`accel_kts_per_s`、`visual_factor`、`vp_value`、`chaff`、`decoys`：
  数据书不印这些字段，均按同级舰/角色推算，属于 **estimated**。
- 传感器：优先复用既有通用 id（`radar_air_*`、`radar_surf_*`、`radar_nav`、`esm_std`、`visual`、
  `sonar_p_hull`、`sonar_p_tow`、`sonar_a_hull`、`sonar_dipping`）。具体型号（如 Sampson、Artisan、
  SPY-1D、CEAFAR、Sonar 2087）在表中注明，映射到量级最接近的通用 id；不新增同义传感器 id。
- `weapons` 的 `ammo` 是每座弹药数，场景加载器会乘 `mounts`（`scenario.rs:128-129`）。因此
  「2 座 × 4 发」写成 `mounts:2, ammo:4`，总弹数 8。VLS 按导弹数而非物理发射单元数计。
- 鱼雷 `ammo` 为该座的总装载量（含再装填），不是发射管数；例如 Type 23 的 24 枚 Sting Ray 写成
  `2×12`，Type 42 的 6 枚写成 `2×3`。

## 舰船/潜艇条目

| id | 类名 | 数据书出处（PDF / 印刷页） | 备注 |
|---|---|---|---|
| uk_qe | Queen Elizabeth (ii) CV | 17-1 PDF p.33 / A-29 | 1052 DP、27 kt；3×Phalanx；航空联队 40（最多 36 F-35B） |
| uk_illustrious | Illustrious (Invincible class) CVH | 17-1 PDF p.36 / A-32 | 563 DP、28 kt；22 Sea Dart、3×Goalkeeper |
| uk_ocean | Ocean LPH | 17-1 PDF p.62 / A-58 | 504 DP、19 kt |
| uk_albion | Albion (ii) LPD | 17-1 PDF p.61 / A-57 | 382 DP、18 kt |
| uk_bay | Bay class LSD (Largs Bay) | 17-1 PDF p.62 / A-58 | 293 DP、18 kt；Cardigan Bay/Lyme Bay 的 Phalanx/DS30 加装 |
| uk_vanguard | Vanguard SSBN | 17-1 PDF p.37 / A-33 | 271 DP、25 kt；SLBM 不建模（引擎无 SLBM 类型） |
| uk_astute | Astute SSN | 17-1 PDF p.37 / A-33 | 162 DP、29 kt；24 Spearfish + 14 Tomahawk |
| uk_trafalgar | Trafalgar SSN | 17-1 PDF p.37-38 / A-33-34 | 130 DP、29 kt（泵喷）；12 鱼雷 + 12 Tomahawk（2009 构型） |
| uk_resolution | Resolution SSBN | 17-1 PDF p.37 / A-33 | 176 DP、23 kt；Polaris 不建模 |
| uk_swiftsure | Swiftsure (ii) SSN | 17-1 PDF p.38 / A-34 | 123 DP、30 kt（泵喷）；16 鱼雷 + 8 Tomahawk |
| uk_upholder | Upholder SS | 17-1 PDF p.39-40 / A-35-36 | 78 DP、20 kt；12 鱼雷 + 4 Harpoon |
| uk_type45 | Type 45 (Daring) DDG | 17-1 PDF p.43 / A-39 | 246 DP、31 kt；16 Aster 15 + 32 Aster 30、8 Harpoon、2×Phalanx |
| uk_type42_3 | Type 42/3 (Manchester) DDG | 17-1 PDF p.44 / A-40 | 212 DP、31 kt；22 Sea Dart |
| uk_type42_12 | Type 42/1 & 42/2 DDG | 17-1 PDF p.45 / A-41 | 198 DP、29 kt；22 Sea Dart |
| uk_type82 | Type 82 (Bristol) DDG | 17-1 PDF p.44-45 / A-40-41 | 270 DP、30 kt；38 Sea Dart、23 Ikara |
| uk_type31 | Type 31 (Inspiration) FF | 17-1 PDF p.47-48 / A-43-44 | 231 DP、26 kt；12 Sea Ceptor、57mm、2×40mm；SSM 仅预留 |
| uk_type26 | Type 26 (City) FFG | 17-1 PDF p.48 / A-44 | 263 DP、28 kt；48 Sea Ceptor；Mk41 24 单元载荷未确认，未装 |
| uk_type23 | Type 23 (Duke) FF | 17-1 PDF p.48-49 / A-44-45 | 196 DP、30 kt；LIFEX 构型：32 Sea Ceptor、4 Harpoon、24 鱼雷 |
| uk_type22_3 | Type 22/3 (Cornwall) FF | 17-1 PDF p.49 / A-45 | 225 DP、30 kt；8 Harpoon、12 Sea Wolf |
| uk_type22_2 | Type 22/2 (Boxer) FF | 17-1 PDF p.49-50 / A-45-46 | 224 DP、30 kt；4 Exocet、12 Sea Wolf |
| uk_type21 | Type 21 (Amazon) FF | 17-1 PDF p.50-51 / A-46-47 | 154 DP、30 kt；4 Exocet、4 Sea Cat |
| uk_leander_3 | Leander/3 (Broad-Beamed) FF | 17-1 PDF p.52 / A-48 | 157 DP、28 kt；4 Sea Cat、Limbo |
| uk_leander_2 | Leander/2 (Exocet) FF | 17-1 PDF p.50 / A-46 | 169 DP、28 kt；4 Exocet、12 Sea Cat |
| uk_county_2 | County (Batch 2) DDG | 17-1 PDF p.45 / A-41 | 264 DP、30 kt；24 Seaslug Mk2、8 Sea Cat |
| uk_county_1 | County (Batch 1) DDG | 17-1 PDF p.46 / A-42 | 258 DP、30 kt；24 Seaslug Mk1（15 nm）、8 Sea Cat |
| uk_river_b2 | River (ii) Batch 2 PB | 17-1 PDF p.55-56 / A-51-52 | 103 DP（p.56 页首）、24 kt；1×DS30M |
| uk_hunt | Hunt MHS | 17-1 PDF p.64 / A-60 | 56 DP、17 kt；DS30、猎雷声呐 |
| uk_sandown | Sandown MHS | 17-1 PDF p.64 / A-60 | 45 DP、13 kt；DS30、猎雷声呐 |
| uk_fort_victoria | Fort Victoria AOR | 17-1 PDF p.70-71 / A-66-67 | 363 DP、20 kt；2×Phalanx（后期） |
| uk_tide_later | Tide (Later) AO | 17-1 PDF p.69 / A-65 | 346 DP、26 kt；3×40mm |
| uk_wave_knight | Wave Knight AO | 17-1 PDF p.69 / A-65 | 320 DP、19 kt；2×DS30 |
| uk_endurance_2 | Endurance (ii) PS | 17-1 PDF p.58 / A-54 | 200 DP、12 kt；2×Lynx |
| au_melbourne | Melbourne CVL | 17-1 PDF p.8 / A-4 | 406 DP、24 kt；1971 年攻击联队 14 A-4G + 4 S-2E + 6 Wessex |
| au_collins | Collins SS | 17-1 PDF p.8-9 / A-4-5 | 96 DP、21 kt；18 Mk48 + 4 Harpoon |
| au_hobart | Hobart DDG (AWD) | 17-1 PDF p.9 / A-5 | 262 DP、28 kt；32 SM2MR + 64 ESSM、8 Harpoon |
| au_anzac_asmd | Anzac (ASMD) FF | 17-1 PDF p.12 / A-8 | 189 DP、27 kt；32 ESSM、8 Harpoon |
| au_anzac | Anzac (Aust svc) FF | 17-1 PDF p.12 / A-8 | 189 DP、27 kt；8 RIM-7P、无 Harpoon（基线构型） |
| au_perry | O.H. Perry (FFG UP) FFG | 17-1 PDF p.11 / A-7 | 166 DP、29 kt；32 SM1MR + 32 ESSM、8 Harpoon |
| au_canberra | Canberra LHD | 17-1 PDF p.15 / A-11 | 418 DP、20 kt；10 MRH90（甲板 18 个位） |
| au_tobruk_2 | Tobruk (ii) LST | 17-1 PDF p.15-16 / A-11-12 | 100 DP、18 kt |
| au_arafura | Arafura PB | 17-1 PDF p.14 / A-10 | 119 DP、22 kt |
| au_huon | Huon MHC | 17-1 PDF p.16 / A-12 | 56 DP、14 kt |
| au_largs_bay | Largs Bay (Choules) LSD | 17-1 PDF p.16 / A-12 | 293 DP、18 kt |
| au_supply | Supply AOR (ii) | 17-1 PDF p.19 / A-15 | 230 DP、20 kt |
| ca_bonaventure | Ex-UK Majestic CVL (Bonaventure) | 17-1 PDF p.20 / A-16；Annex R PDF p.93 / R-1 | 406 DP、24 kt；8 Tracker + 13 Sea King（Annex R） |
| ca_victoria | Victoria (ex-Upholder) SS | 17-1 PDF p.20-21 / A-16-17 | 78 DP、20 kt；16 Mk48 |
| ca_iroquois_trump | Iroquois (TRUMP) DDG | 17-1 PDF p.21-22 / A-17-18 | 210 DP、29 kt；29 SM2MR、76mm |
| ca_halifax | Halifax FF | 17-1 PDF p.23 / A-19 | 226 DP、29 kt；16 Sea Sparrow、8 Harpoon |
| ca_halifax_felex | Halifax FELEX FF | 17-1 PDF p.22-23 / A-18-19 | 226 DP、29 kt；16 ESSM、8 Harpoon |
| ca_kingston | Kingston MM | 17-1 PDF p.26 / A-22 | 64 DP、15 kt |
| ca_harry_dewolf | Harry DeWolf AOPS | 17-1 PDF p.27 / A-23 | 247 DP、17 kt |
| ca_protecteur_2 | Protecteur (ii) AOR | 17-1 PDF p.27 / A-23 | 236 DP、20 kt |
| ca_asterix | Astérix AOR | 17-1 PDF p.27-28 / A-23-24 | 186 DP、20 kt |
| nz_anzac_fsu | Anzac (FSU) FF | 17-1 PDF p.28-29 / A-24-25 | 185 DP、28 kt；20 Sea Ceptor |
| nz_anzac | Anzac (NZ svc) FF | 17-1 PDF p.29 / A-25 | 185 DP、28 kt；8 RIM-7P |
| nz_aotearoa | Aotearoa AOR | 17-1 PDF p.32 / A-28 | 486 DP、20 kt |
| nz_canterbury | Canterbury (ii) AX | 17-1 PDF p.32-33 / A-28-29 | 219 DP、19 kt |
| nz_endeavour_3 | Endeavour (iii) AOR | 17-1 PDF p.32 / A-28 | 174 DP、14 kt |
| nz_otago | Otago (Protector) PS | 17-1 PDF p.31 / A-27 | 105 DP、22 kt |

## 飞机条目（16-1）

| id | 机型 | 数据书出处（PDF / 印刷页） | 备注 |
|---|---|---|---|
| cw_f35b | Lightning (F-35B) | 16-1 PDF p.47 / B-42 | Damage 24；920 kt（high/reheat）；升限 15240 m 写入 name |
| cw_merlin_hm2 | Merlin HM.2 | 16-1 PDF p.49-50 / B-44-45 | Damage 23；典型 2 条鱼雷 |
| cw_wildcat_hma2 | Wildcat HMA.2 | 16-1 PDF p.70 / B-65 | Damage 16；ASW 构型 2 条鱼雷；Martlet 未装 |
| cw_lynx_has3 | Lynx HAS.3 | 16-1 PDF p.49 / B-44 | Damage 15；ASW 构型 2 条鱼雷；Sea Skua 未装 |
| cw_sea_harrier_fa2 | Sea Harrier FA.2 | 16-1 PDF p.55 / B-50 | Damage 19；2 AMRAAM + 2 Sea Eagle |
| cw_sea_king_has6 | Sea King HAS.6 | 16-1 PDF p.57 / B-52 | Damage 17；4 条鱼雷 |
| cw_sea_king_asac7 | Sea King ASaC.7 | 16-1 PDF p.56 / B-51 | Damage 17；AEW，无武器 |
| cw_s2e_tracker | S-2E Tracker | 16-1 PDF p.16 / B-11 | Damage 29；2 鱼雷 + 4 深弹 |
| cw_a4g_skyhawk | A-4G Skyhawk | 16-1 PDF p.6 / B-1 | Damage 17；4 AIM-9 + 6 Mk82 |
| cw_wessex_mk31 | Wessex Mk31A/B | 16-1 PDF p.18 / B-13 | Damage 13；2 条鱼雷 |
| cw_ch148_cyclone | CH-148 Cyclone | 16-1 PDF p.24 / B-19 | Damage 19；2 条鱼雷 |
| cw_sh2g | SH-2G Super Seasprite | 16-1 PDF p.31 / B-26 | Damage 15；2 条鱼雷（Penguin 仅 SH-2G(I)，未装） |
| cw_ch124_sea_king | CH-124 Sea King | 16-1 PDF p.23 / B-18 | Damage 17；4 条鱼雷 |
| cw_mrh90 | MRH90 Taipan | 16-1 PDF p.14 / B-9 | Damage 18；运输型，无武器 |

## 武器条目

远程/防空导弹数值取 17-1 Annex D1（PDF p.76-77 / D-1-D-2）；炮取 Annex C（PDF p.73-74 / C-1-C-2）；
Ikara 取 Annex E4（PDF p.80 / E-1）；飞机挂载取 16-1 Annex H1/H2。

| id | 名称 | 类型 | 数据书出处 | 关键值（数据书） |
|---|---|---|---|---|
| cw_aster15 | Aster 15 | Sam | 17-1 p.76 / D-1 | 18.6 nm、1725 kt、21+D6/3 |
| cw_aster30 | Aster 30 Blk 0 | Sam | 17-1 p.76 / D-1 | 74.5 nm、2590 kt、25+D6/3 |
| cw_seaceptor | Sea Ceptor (GWS35) | Sam | 17-1 p.76 / D-1 | 13.5 nm、1978.5 kt、16+D6/3 |
| cw_seawolf | Sea Wolf (GWS25/26) | Sam | 17-1 p.76 / D-1 | 4.0 nm、1320 kt、17+D6/3 |
| cw_seadart | Sea Dart Mod 2/3 | Sam | 17-1 p.76 / D-1 | 80 nm、1650 kt、24+D6/3 |
| cw_seaslug | Seaslug Mk2 | Sam | 17-1 p.76 / D-1 | 25 nm、1188 kt、32+D6/2 |
| cw_seaslug_mk1 | Seaslug Mk1 | Sam | 17-1 p.76 / D-1 | 15 nm、1188 kt |
| cw_seacat | Sea Cat | Sam | 17-1 p.76 / D-1 | 2.5 nm、530 kt、18+D6/3 |
| cw_essm | ESSM RIM-162 Blk I | Sam | 17-1 p.77 / D-2 | 18 nm、2250 kt、25+D6/2 |
| cw_sm1mr | SM1MR RIM-66 Blk VI | Sam | 17-1 p.76 / D-1 | 25 nm、1650 kt、25+D6/2 |
| cw_sm2mr | SM2MR RIM-66 Blk IIIA | Sam | 17-1 p.77 / D-2 | 90 nm、2006 kt、27+D6/2 |
| cw_seasparrow | RIM-7M/P Sea Sparrow | Sam | 17-1 p.76 / D-1 | 12 nm、1650 kt、23+D6/3 |
| cw_exocet | MM38 Exocet | Ascm | 17-1 p.76 / D-1 | 23 nm、603 kt、36+D6/2 |
| cw_nsm | NSM | Ascm | 17-1 p.76 / D-1 | 108 nm、627 kt、34+D6/2 |
| cw_ikara | Ikara GWS40 | AswRocket | 17-1 p.80 / E-1 | 1-12 nm、495 kt |
| cw_4_5in | Mk8/Mk6 4.5in | Gun | 17-1 p.74 / C-2 | 短程 21 伤、12 nm 级 |
| cw_5in_mk45 | Mk45 Mod 4 5in/62 | Gun | 17-1 p.74 / C-2 | 短程 24 伤、13 nm 级 |
| cw_76mm | 76mm/62 | Gun | 17-1 p.73-74 / C-1-C-2 | 18 伤、10 nm |
| cw_57mm | Bofors 57mm/70 Mk3 | Gun | 17-1 p.73 / C-1 | 15 伤、9.3 nm（3P） |
| cw_40mm | Bofors 40mm/60-70 | Gun | 17-1 p.73 / C-1 | 5 伤、5.4 nm |
| gun_30mm | （复用内置）30mm DS30/GCM | Gun | 17-1 p.73 / C-1 | DS30 3-4 伤；CIWS 用 `ciws_30mm` |
| cw_25mm | 25mm Typhoon/DS25M | Gun | 17-1 p.73 / C-1 | 2 伤、1.6 nm |
| cw_20mm | 20mm | Gun | 17-1 p.73-74 / C-1-C-2 | 2 伤、1.4 nm |
| cw_50cal | 12.7mm/M2/Mini-Typhoon | Gun | 17-1 p.74 / C-2 | 1 伤、1.0 nm |
| cw_762mm | 7.62mm/M134 | Gun | 17-1 p.73-74 / C-1-C-2 | 0.4-0.6 伤、0.7 nm |
| cw_amraam | AIM-120C-7 AMRAAM | Aam | 16-1 p.83 / H-8 | 44 nm、2295 kt |
| cw_martlet | Martlet | Aam | 16-1 p.79 / H-4 | 4.3 nm、989 kt |
| cw_sea_eagle | Sea Eagle | Ascm | 16-1 p.79 / H-4 | 60 nm、600 kt、41+D6/2 |
| cw_sea_skua | Sea Skua | Ascm | 16-1 p.79 / H-4 | 8.2 nm、560 kt、19+D6/3 |
| cw_penguin | Penguin Mk2 Mod7 | Ascm | 16-1 p.80 / H-5 | 20 nm、528 kt、33+D6/2 |
| cw_paveway_iv | Paveway IV | Bomb | 16-1 p.79 / H-4 | 9 nm、40 伤 |
| cw_mk82 | Mk82 500 lb | Bomb | 16-1 p.78 / H-3 | 35 伤 |
| cw_zuni | Zuni 5in FFAR | Rocket | 16-1 p.77 / H-2 | 24 伤 |
| cw_depth_charge | Mk11 depth charge | Bomb | 17-1 p.80 / E-1 | 10/5 伤 |

内置复用而未重定义的条目：`ciws_30mm`（Phalanx/Goalkeeper 的通用替身）、`gun_30mm`（DS30/GCM 30mm）、
`torp_homing`（Spearfish/Mk48 通用）、`lwt`（Sting Ray/Mk46/MU90 通用）、`ascm_std`（Harpoon 通用）、
`slcm`（Tomahawk 通用）、`asw_rocket`（Limbo 通用）、`aam_ir`（AIM-9 通用）。这些条目的参数沿用内置值，
不是本卷新定义；其 targets/guidance 已由内置语义保证。

## 传感器条目

| id | 名称 | 类型 | 说明 |
|---|---|---|---|
| cw_mad | MAD | Magnetic | 声呐浮标/MAD 不在数据书平台表内独立成项；1.5 nm 为引擎标尺估计 |
| cw_sonobuoy | 被动声呐浮标场 | SonarPassive | 12 nm 为引擎标尺估计（Merlin 30 枚、S-2 32 枚、CH-148 32 枚） |
| cw_irst | IRST/FLIR | Ir | 空 25 / 海 15 / 弹 10 nm，引擎标尺估计 |

## estimated 值清单

以下字段/条目不是数据书直接印出的数值，均为引擎标尺推算：

1. 所有平台的 `cruise_speed_kts`（约最大速度的 55-70%；潜艇取静音巡航 8-10 kt）。
2. 所有平台的 `turn_rate_deg_s`、`accel_kts_per_s`、`noise_db`、`visual_factor`、`vp_value`、`chaff`、`decoys`。
   潜艇 `max_depth_m` 由数据书深度带推算：Int IV≈300 m、Int V≈450 m、Deep I≈400 m、Int III≈250 m（estimated）。
3. 所有武器的 `pk`、`salvo`、`reload_s`、`directors`；数据书给的是 ATA/伤害/射程/速度，不给这些引擎字段。
   例如 Aster 30 的 0.65 pk、Sea Ceptor 的 0.60 pk 为估计。
4. `cw_mad`、`cw_sonobuoy`、`cw_irst` 的探测距离；`cw_depth_charge` 的 10 伤（数据书 Mk7 为 10/5 伤）。
5. 飞机 `endurance_s` = `Cruise Range` ÷ `Cruise Speed` × 3600（估算），不是数据书的续航时间。
6. 航母/两栖舰航空联队编成：
   - `uk_qe`：卷内只给「Airwing size 40，最多 36 F-35B」；24 F-35B + 10 Merlin HM.2 + 6 Wildcat 为估计编成。
   - `uk_illustrious`：8 Sea Harrier FA.2 + 9 Sea King HAS.6 + 3 Sea King ASaC.7，取自 17-1 Annex R PDF p.99 的典型 1987-89 编成。
   - `uk_ocean`：12 Merlin + 6 Wildcat，按「20 架直升机/600 名陆战队」的上限估计。
   - `au_melbourne`：14 A-4G + 4 S-2E + 6 Wessex，取自 p.8 的 1971 年战时选项。
   - `ca_bonaventure`：8 Tracker + 13 Sea King，取自 17-1 Annex R PDF p.93 的 1967 年编成。
   - `au_canberra`：10 MRH90（甲板容量 18），取自 p.15 的航空联队行。
7. 舰载机挂载为「一个代表性构型」，不是同时挂满所有选项：
   - F-35B：2 AMRAAM + 2 Paveway IV（内置多用途）；4 AMRAAM 纯空优构型未同时挂弹。
   - Sea Harrier FA.2：2 AMRAAM + 2 Sea Eagle；AIM-9 构型未叠加。
   - A-4G：4 AIM-9 + 6 Mk82；Zuni 火箭未叠加。
   - S-2E：2 鱼雷 + 4 深弹。
   - Merlin/Wildcat/Lynx/SH-2G：按 ASW 构型只挂鱼雷，Martlet/Sea Skua/Penguin 未叠加。
8. `uk_type45` 的 Harpoon 8 发按 2015 年加装构型录入；数据书同时注明 2023 年 12 月 Harpoon 退役。
9. `uk_type23` 采用 LIFEX 构型（32 Sea Ceptor、4 Harpoon）；基础构型为 32 VL Sea Wolf、通常 4 Harpoon。
10. `uk_type26` 的 24 个 Mk41 单元载荷「未确认」，因此没有装填 Tomahawk/VL ASROC/LRASM；只装确认的 48 Sea Ceptor。
11. `uk_type31` 只有 SSM/VLS 预留，未装反舰导弹。
12. `uk_type82` 与 `uk_type42` 共用 `cw_seadart`（Mod 2/3，80 nm）；1978 年 Type 82 的早期 Sea Dart 射程更短，属高估。
13. `uk_county_1` 使用 `cw_seaslug_mk1`（15 nm）；`uk_county_2` 使用 `cw_seaslug`（Mk2，25 nm）。
14. `au_perry` 使用 SM1MR 数据；该级 2010 年后部分换 SM2MR，但受 Mk92 限制只到 40 nm，未另建变体。
15. 潜艇的 SLBM（Trident D5/Polaris A3）不建模：引擎没有 SLBM 武器类型，强行套用 `slcm` 会给出错误的目标/射程语义。
16. 所有鱼雷统一用 `torp_homing`（14 nm、250 伤）或 `lwt`（6 nm、45 伤）；Spearfish/Mk48/Sting Ray/Mk46/MU90 的差异未细分。
17. Harpoon 统一用 `ascm_std`（60 nm、560 kt、42 伤）；数据书 Harpoon IC/II 为 80 nm、561 kt、40 伤，射程略低、伤害略高。
18. 声呐型号到通用 id 的映射：Sampson/SPY-1D/S1850M/CEAFAR → `radar_air_xlong`；Artisan/Type 996/997/1022/SMART-S → `radar_air_med`；
    Type 992/1006/1007/SharpEye/Scanter → `radar_surf_sm`；Sonar 2050/2091/2150/SQS-505/Spherion → `sonar_p_hull`+`sonar_a_hull`；
    Sonar 2087/2031/CANTASS/Sea Lancer → `sonar_p_tow`。这些是量级映射，不是型号性能的精确复现。

## 已知的制导/挂载语义简化

- `cw_paveway_iv` 是激光/GPS 制导炸弹，但引擎的 `Bomb` 只有 `BallisticGun` 一种落弹方式；本卷按自由落体炸弹处理。
- `cw_seaslug`/`cw_seadart`/`cw_seacat` 在数据书里是波束/半主动制导，引擎没有对应 `Guidance` 变体，统一用 `HomingAir`。
- `cw_depth_charge` 是深水炸弹，`Bomb` 类型 + `targets.sub` 在引擎里按自由落体反潜武器裁决。
- 反舰导弹的 `SeaSkimmer` 用于所有掠海弹（Exocet/NSM/Harpoon/Sea Eagle/Sea Skua/Penguin），不区分弹道末端。

## 未收录 / 与任务说明的差异

- **印度海军不在本卷内。** 17-1 的封面、目录和正文只覆盖 United Kingdom、Australia、Canada、New Zealand；
  「Alphabetical Index of Ship Classes」中没有 Kolkata/Visakhapatnam/Vikrant，正文检索也没有 P-15A/B 或 BrahMos。
  因此本卷 **没有** `in_` 前缀平台、没有 P-15A/B、没有维克兰特。
- **没有 `cw_brahmos`。** 任务说明把 BrahMos 列为新武器，但 17-1 不含印度海军，也没有 BrahMos 条目。
  资料库中唯一出现 BrahMos 的是 `26.（数据库）菲律宾海军.pdf` p.25（印度出口型，2025 年菲律宾），
  其速度为 1650 kt 高弹道/1056 kt 掠海，也不是任务说明里的 800 kt。按「读不到的就少录，禁止编造」，
  本卷不创建 `cw_brahmos`。
- `uk_qe` 的 3 座 Phalanx 是卷内 2023 构型；卷内还列有 M134/M2，但引擎没有 7.62mm 舰载武器 id，
  统一归入 `cw_762mm`/`cw_50cal` 时只保留主要 CIWS 与少量小口径座。
- `au_melbourne` 的 40mm 座数按卷内 port/stbd 总数抽象为 8 座；实际单/双联装混编，引擎不区分联装。
- `uk_illustrious` 的 3 座 Goalkeeper 用 `ciws_30mm` 表示（卷内为 30mm Goalkeeper）；`uk_qe`/`uk_type45` 的
  Phalanx 也用 `ciws_30mm`，因为引擎只有这一个 CIWS id。
- 飞机 `noise_db` 填 0（引擎不对空中目标做被动声呐探测）；这不是数据书值。
