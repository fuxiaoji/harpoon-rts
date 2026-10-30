# 第5章探测机制录入与实际扫描验收（2026-10-23）

显式`hv2026-rts-v4`新扫描链。**整章尚未完成**；实际扫描、只录表/纯API、附件缺口、待裁分别记录，未把37或40个测试当机制完成率。旧Legacy/v2/v3行为保留。

来源：英文2026-01-01 PDF40–68，目标舷角图PDF111。先核心中文RAG，再英文原页与表格视觉。数据75×17声纳概率、12×7×7表面目视、71×6视觉检测行，另声纳速度/层/CZ/BB/ES/IR/MAD/尾流/陆地/星弹。Source SHA、原页文本、24张规则表图与勘误图及red/green日志见根`artifacts/rules-all-20261023/detection/manifest.json`。

`detection2026::evaluate_world`进入`sensors.rs`实际v4扫描；裁判先检定/计时，成功结果才投射本方Contact。未知性能/噪声/签名/阵列深度返回Err，不回退旧估算探测。`known_category`仅实际分类证据，ES/初始被动声纳类别null，绝不复制真舰级、真航向或真航速。被动图标1海里锚点只画方位，不是测距；角度内均匀投影是明确RTS表示，骰值保存。普通雷达未分辨同编队仅一个回波，敌方任务群名字始终裁判私有。

勤务接口：`order_array`、`DetectionAction::{Tow,Dip,UltraQuiet,Masts}`、`array_speed_limit`、`own_view`。真实运动接15/25kt拖曳与3kt超静音；扫描生命周期处理21min作业、失阵列、发射噪声；只本方可看任务。吊放声纳必须明确匹配悬停直升机/已落水水上飞机或安装该声纳的静止舰艇，当回合可部署/回收；实际横移即永久断缆，完整World保存部署点。源国别深度/夹装阵列/完整浮标字段等仍在后续表中。

概率与回放：按receiver/目标/目的/180s窗口派生独立稳定随机，已掷骰和真实分类/部署持久化。声纳每窗口重验、12分钟alert；IR包线内保持；ELS第二次必须不同窗口且观察飞机移动。来源表头曾被自动抽取误算为声纳数据行，书内实际例回归先失败后排除表头，最终75行，不把未经核验抽取当完成。

| ID | 章节/PDF | 机制 | 状态 | 实际执行/限制 |
|---|---|---|---|---|
|DET-01|5.1 / 40|Active/passive information and hidden referee scan|tested|evaluate_world/update_sensors_v4；Explicit all detected evidence; failed scans Referee only; initial passive category null|
|DET-02|5.2.1 / 40|Surface automatic and fleeting/continuous masts|tested_subset|radar_range_world/radar_mast_probability；Source verified radars only; mast command/maximum counts still need annex binding|
|DET-03|5.2.1 / 41|Air combat-system recognition|existing_v3_subset|operations radar_clocks；Existing source subset reused; negative predictive adjustments remain unimplemented|
|DET-04|5.2.2 / 41|Signature/aspect/external stores/3-or9 formation uplift|tested_subset|radar_range_world/group_signature；Explicit group signature; ordinary unresolved group now publishes one contact, retaining contact identity across representative changes; full annex coverage still incomplete|
|DET-05|5.2.3 / 41|Radar range/bearing, 180s derived motion, no ordinary class leak|tested_subset|update_sensors_v4/update_radar_motion；No truth motion/class leak; same saved per-receiver projection for icon and own report; source FCS uses own measured geometry/180s motion, not fused side data; TDL/crossfix pending|
|DET-06|5.2.3 / 41|SAR/ISAR/NCTR special recognition|executable_subset|radar_identifies；Explicit Receiver::RadarFeatures source capability; actual ISAR class recognition tested; unknown fitted features not inferred|
|DET-07|5.2.4-6 / 41|Ship/air/ground radar roles, arcs, bands, HF nonsearch|existing_v3_subset|operations altitude_modifier/radar_profile；Unknown search fits blocked; OTH-B/HFSW scenario supplied only, not automatic satellite intelligence|
|DET-08|5.2.8-9.4 / 42|Horizon, intervening land, largest sea/rain/land/EW clutter|tested_subset|radar_range_world/operations clutter_modifier；No invented DEM; directional rain-squall clutter layer incomplete|
|DET-09|5.2.9.5 / 44|Evaporative/surface duct table/probability|executable_subset|duct_horizon/evaporative_max_size/surface_duct_chance/radar_range_world；Explicit Configuration.duct enters radar and ES; auto weather generation, SS-T/SRS1 size exceptions pending|
|DET-10|5.2.9.5 optional / 45|Ambiguous overrange duct contacts|not_implemented|none；Optional requires neutral/white shipping and ambiguous range UI|
|DET-11|5.3 / 45|ES search/weapon emissions, generation/LPI/SS-T ranges|tested_subset|evaluate_world Receiver::Es；Explicit emission and receiver metadata; no default from estimated DB; radio not modeled|
|DET-12|5.3 / 45|RWR gen0/1/2+|partial|Receiver::Es；Gen0 has no source horizon multiplier so blocks, later-gen capability explicit|
|DET-13|5.3.1 / 46|ELS Fair/Good after successive moving detections|tested_subset|State Track ELS window；Evidence quality retained; no truth coordinate granted; actual RF cross-fix is chapter6 integration|
|DET-14|5.4.1 / 46|Bow/keel/flank/sail baffles and generation|tested|sonar_bearing_eligible/evaluate_world；Diagram visually checked; inactive Gen1-3 flank search blocked|
|DET-15|5.4.2 / 47|Towed/VDS deployment/15kt/21min/25kt/sea and seabed loss|tested_subset|order_array/update_lifecycle/movement cap；Real SetArray/API->1260s lifecycle, movement15/25kt cap, sea/seabed loss and own-side view; clip-on and national depth prescriptions pending|
|DET-16|5.4.2.1 optional / 48|Ambiguity and stabilization|structured|array_stabilization_seconds；Time table tested; maneuver/ambiguous bearing lifecycle pending|
|DET-17|5.4.3 / 48|Dipping sonar moving break/depth|tested_subset|DetectionAction::Dip/order_dipping/update_lifecycle/evaluate_world；Immediate actual scan and recovery, explicit hovering/landed airframe, physical displacement permanently loses cable; actual command/tick/World save and replay tests pass; source attachment coverage remains incomplete|
|DET-18|5.4.4/6.3 / 48|Sonobuoy deployment/survival/channels/field monitoring|partial|buoy_drop_valid/buoy_monitor_range/buoy_fix；Tables/localization helpers tested; source exact loadout/channels and lifecycle not yet fully connected|
|DET-19|5.4.5 / 48|Acoustic intercept generation/range/bearing/type/12nm ranging|tested_subset|Receiver::AcousticIntercept/acoustic_intercept；AIR, masking and own scan share live deployed-array depth; recovered/lost/damaged/unpowered sonar cannot emit; Gen5 ranging preserves printed bearing error and does not reveal host category; bottom bounce/CZ AIR blocks pending bearingless warning representation|
|DET-20|5.4.6 / 49|Separate sonar mode/50% ranges/speed matrices/nearest probability row|tested_subset|Receiver::Sonar/sonar_probability；75x17 source rows, mode lock for Gen1-4; active direct scan needs no unrelated passive noise (EW13 green); full attachment bands pending|
|DET-21|5.4.6 / 49|Active size/aspect/anechoic and torpedo restriction|executable_subset|Receiver::Sonar/active_aspect_modifier；B/Medium is+1 per table despite example+2; torpedo capability explicit|
|DET-22|5.4.6 / 49|Passive noise/cavitation/sea+shipping versus rain/biologics/ice|tested_subset|Receiver::Sonar/sonar_environment；Noisiest environment; real UltraQuiet3kt source propulsion gate; real production launch stable-ID/event-sequence association, 180s Noisy/Quiet/Loud floor and ambiguous identity block; real NAV acceleration/deceleration +4 once; active sonar signal bands; diesel/snorkel pending|
|DET-23|5.4.6.5 / 55|Passive masking/acoustic jammer/explosion/nuclear blueout|partial|passive_masked/hears_active；Masking executes pre-roll against source probabilities, range ties and baffle/path; real chapter12 Loud jammer/10-degree cone passes EW13; explosion/nuclear blueout pending|
|DET-24|5.4.6.6 / 56|Next-turn passive classification, torpedo and decoy penalties|tested_subset|sonar_classification/evaluate_world；Only retained passive detection enables next-turn classification; active echoes cannot reveal narrowband class. An active torpedo seeker received by AIR grants Torpedo category without model/type. Mobile objects are now connected, but real mobile-decoy next-window classification and complete attack-speed regression remain pending|
|DET-25|5.4.7.1 / 57|Layer/environment restrictions and VLF immunity|tested|layer_modifier/Environment.validate；Actual array depth, not towing platform depth|
|DET-26|5.4.7.2 / 57|Bottom bounce depth intervals/frequency/generation|executable_subset|Configuration.propagation/Receiver::Sonar；Explicit rocky-flat bottom; no guessed GEBCO/DEM; actual scan connected|
|DET-27|5.4.7.3 / 58|CZ count/ring/region/generation/chance|executable_subset|Configuration.propagation/Receiver::Sonar；Actual scan connected; passive contacts remain bearing only|
|DET-28|5.4.8 / 59|Holding/reacquisition +15% for12min|tested_subset|State Track last_success；Deterministic one roll per window; replay serialized|
|DET-29|5.5 / 59|IR generation ranges80%/retention/moisture/half-range image|tested_subset|Receiver::Ir/evaluate_world；Explicit signature + source fit; no arbitrary range from DB|
|DET-30|5.5.1-3 / 59|Scanner/FLIR cue/IRST aspect/imaging|tested_subset|Receiver::Ir；FLIR explicit15/5degree tracking command pending; IR cloud blocking unresolved|
|DET-31|5.5.4 / 59|TV/LLLTV classification/visibility|executable_subset|none；Receiver::Tv actual scan requires existing contact, visibility and explicit optical range; no invented TV performance|
|DET-32|5.6 / 60|Laser ranging/designation/warning/LST/LRMTS|partial|Receiver::LaserRangefinder/laser_range；Ranging and cloud gate executes; warning/designation/spot tracking weapons integration pending|
|DET-33|5.7.1 / 60|MAD depth/size/titanium/masking/prior localizing/pass|executable_subset|Receiver::Mad/mad_probability；No own fabricated range; straight/level envelope; masking source fixtures not yet spatially connected|
|DET-34|5.7.2 / 60|Wide-area MAD30min/dwell/depth/area|structured|wide_mad_probability；Matrix tested; monitoring field and dwell runtime pending|
|DET-35|5.7.3 / 61|Wake detection generation/time/depth/speed/sea/three crossings|structured|wake_probability；Formula tested; real wake history/crossing lifecycle still missing|
|DET-36|5.7.4 / 61|Diesel sniffer/LIDAR ineffective sensors|not_applicable|none；No effective standard sniffer modeled; experimental LIDAR not silently included|
|DET-37|5.8.1-4 / 62|Visual surface/air and common formation roll|tested_subset|visual_base/visual_detection_range/Receiver::Visual；12x7x7+71x6+air tables; precise source modification helpers; runtime emission cues partly absent|
|DET-38|5.8.5 / 65|Periscope visual sea/feather/continuous/multiple masts|tested_subset|periscope_visual_probability/evaluate_world；Sea/feather/continuous and multiple-mast1.5range execute. Source literal continuous D100+20 is applied as probability-20; no physical-intent override|
|DET-39|5.8.6 / 66|Land spotting/camouflage/combined structures|executable_subset|land_visibility；Actual Receiver::Visual consumes land C–H table, observation-post eye-height table and camouflage; fired state persists beyond event ring. Unknown structure sizes remain blocked|
|DET-40|5.8.7.1 / 66|Cloud shared reciprocal hole/range/blocking|tested_subset|cloud_effect/Receiver::Visual；Same pair shares one die; in-cloud collision/hiding and joint formation roll pending|
|DET-41|5.8.7.2 / 67|Wind-driven squalls|not_implemented|none；No authored localized precipitation cells/lifecycle|
|DET-42|5.8.8 / 67|Starshell range/miss/illumination/silhouette|executable_api_subset|starshell_effect/starshell_detection_range；Parent AAW owner integrating gunfire;16-20 gray box contradictory distance blocks|
|DET-43|5.8.9 optional / 68|Graded visual identification|structured|source tables only；Basic rule immediate visual class ID executes; optional table not enabled|
|DET-44|5.8.10 optional / 68|Intermediate no-roll sighting|not_enabled|none；Explicit optional alternative not enabled|
|DET-45|5.8.11 / 65|Visual signaling range/message budget|structured|source tables; no simulation comms；Flag/light transport30character per-side per-turn pending|
|DET-46|5.8.12 / 68|Steam stack smoke source propulsion thresholds|not_implemented|none；Full source propulsion/fuel/national-origin metadata absent|

## 版本与勘误对照

[ATG官方2026-07-28勘误](https://www.admiraltytrilogy.com/cic/Harpoon/Hpn%20V%201%20Dec%202025%20Printing%20Errata%2028%20Jul%2026.pdf)本地p3–6视觉核对。1月本已含视觉列边界、云层下取整和新版可选识别表的大部分修改；声纳三种环境取最噪可澄清。7月删除Low空搜两行惩罚与1月仍存在的规则冲突，因此保留1月条件。未静默改版。

## 需要保留的来源疑点

- DET-Q01：声纳总修正<-10及恰等距最近行未给值/取整，阻断。
- DET-Q02：视觉基础距离>52kyd或0.3–0.5kyd没有直接检测行，阻断。
- DET-Q03：星弹16–20灰格是后4kyd，正文说后2kyd内轮廓，阻断。
- DET-Q04：B/Medium主动强度正式表+1、例子+2，以表为准。
- DET-Q05：连续桅杆原文字面D100+20降低发现率；严格按字面等价概率-20，反直觉处保留待澄清，未擅自改成概率+20。

测试中使用明确的机制夹具参数验证通用程序；这些不证明福特实际安装某种测试声纳/成像雷达。附件拟合与通用机制是两条独立验收线。完整工作区、UI和历史档最终回归由父代理统一执行。

## 本轮续作的失败证据与验收

根 `artifacts/rules-all-20261023/master/continuation-red-detection.log` 记录44通过/2失败：主动声纳错误获得被动窄带舰级，以及AIR不认真实ArrayJob深度。修复将扫描、遮蔽和AIR取深统一到实际已部署且未丢失的阵列。吊放新增真实Detection命令→sim tick→扫描及完整World存恢复/命令重演案例。后续 `continuation-domains-3.log` 记录50通过/1个Gen5 AIR角误差失败；修复公开点位和类别后，最终 `artifacts/rules-all-20261023/master/continuation-workspace-final.log` 的探测51例全部通过。全工作区747次通过、0失败、5忽略；次数含既有与重复目标，不是规则条数，不代表探测或电子战整章完成。

后续独立next包的第12章19项台账见 `ew-all-mechanisms-20261023.md/.json`；EW13例已在 `artifacts/rules-next-20261023/validation/second-core-run.log` 全绿，具体执行/未完成边界见 `ew-execution-20261023.md`。本段747次仅指冻结旧包。

新包探测55例在 `artifacts/rules-next-20261023/validation/third-core.log` 全绿，包括真实production launch稳定ID与显示名关联、完整World恢复、歧义显式阻断，以及NAV真实movement加减速噪声+4不叠加。`actual-launch-red.log` 的第一例是有效生产漏项红证据；同日志另一例先断在测试错误读取trace嵌套路径，不计第二个生产bug。`second-core-run.log` 为53通过/2夹具失败，分别按真实潜射门修正Shallow10m/5kt及按DB29kt修正明确NAV max；没有放宽生产门。随后 `fourth-core.log` 探测56例验证每个真实成功sensor的本机证据；`fcs-own-motion-red.log` 两条实际长时钟源红复现融合位置/运动借用，修复后 `fifth-core.log` 探测61例全绿，含本机180秒门、丢失重建、完整World恢复、融合字段污染独立与旧Track serde兼容。执行与RTS限制见 `fcs-own-reports-20261023.md`；本机仅方位定位与明确TDL/交叉定位仍未完成。
