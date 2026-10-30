# 航空反潜入水与Mk82投放生产包

本文件保留rules-next批次的来源与验收记录。后续明确Mk54构型和投放准入边界以`aviation-following-production-20261023.md`、`aviation-asw-admission-20261023.md`各自标明的批次状态为准；旧记录中的真实浮航前置拒绝已在after-following列为待修的信息探针，不能继续作为本方实时深度知识。完整9.7入水立即就位/攻击仍有下一源红待补，不能以本文件标题视作全程序完成。

这是747通过的上一验收批之后的新一包。旧程序保存在主代理冻结的 `artifacts/rules-all-20261023/continuation/harpoon-server-v4-verified.exe`，旧日志与战局不重算。新实验仍明确选择 `hv2026-rts-v4`，`aircombat-2026.json`增加执行修订及源机制数据，改变v4表指纹，不能用新表解释旧v4存档。

## 来源与录入

按仓库规则Skill先检索核心中文RAG的空投鱼雷、武器就位、空袭表和非制导投放，再回看英文Jan2026 PDF105–109/印刷9-4–9-8。航空反潜条文及投放概率表已有来源图；本包重新视觉检查PDF106/107。美国航空2025-12-01 PDF49/B44有F/A-18E的3个副油箱、2枚AIM-9、2枚AIM-120、6枚Mk82挂载；PDF98/H1-3的Mk82行为241kg、35DP，Glide Low/Medium穿透4/4，Level Low/Medium/High穿透4/4/4。新视觉证据 `artifacts/rules-all-20261023/aviation-next/us-p098.png`。没有把Mk82SE、Mk82AIR、BLU-111或泛称LGB当成同一精确武器。

## §9.7.1 实际生产闭环

`air_torpedo_gate`不再只存在于辅助测试：武器执行器在人工和自动航空鱼雷发射前消费它，成功入水才调用`air_torpedo_dropped`，实际水下推进消费`air_torpedo_interfered`。Low与50%FMP、直升机悬停例外、当前已定位的本方/共享潜艇航迹、每机180秒共同投放时钟、同目标空投互扰均有真实命令案例。过期/只有方位、未确认潜艇类别及浮航目标不被当作本条的潜航攻击许可。

源6.3.8的就位和第10章鱼雷属性/攻击由水下模块负责，不能把融合航迹quality直接称为Good。独立测试使用明确`scenario_assumption`的Good解、水文及目标属性，检验规则执行，不将夹具条件称为该潜艇型号已全部核验。本包不宣称全部航空深弹、核深弹和所有航空鱼雷型号已完成。

实际先失败的三个案例在 `artifacts/rules-all-20261023/master/next-red-airborne-asw.log`；接生产后 `next-green-airborne-asw.log` 三项全部通过：超投放高度/速度不发射、不扣弹；人工请求两枚实际只投一枚、自动扫描继续服从同一180秒时钟；双机同目标互扰先于伤害并保存续演完整World相同。

## §9.6 来源Mk82子集

精确构型 `us_f18e_mk82`继承同一F/A-18E来源性能/机载系统，以6枚Mk82替换泛称制导炸弹挂架。试验载荷6395kg由3×1485kg副油箱、2×162kg AIM-120D、2×85kg AIM-9X-2、6×241kg Mk82组成；外挂声明、总质量及剩余燃油仍由想定显式提供。新构型的舰面操作与APG79行存于`aviation-2026.json`独立`operation_aircraft_variants`/`operation_radar_variants`，经过world profile包装函数只供v4使用；`operations-v3.json`及旧v3表指纹保持原样。

`Command::AirCombat`的`surface_attack`动作实际消费已核挂架及弹库，以来源物理尺寸、Advanced瞄准具、速度与显式想定NoAA条件计算最终行。Glide每最多4枚一组、每组单D100，六枚分4+2；单机Level投六枚仍按salvo1，最多一枚命中。命中将来源35DP/穿透4及投放剖面送正常损伤队列，地面目标送同tick聚合的陆上损伤流程；未命中没有其他效果。实际投放递减外挂质量，Level投放的30秒增量内MR为0。每架同180秒窗口一次投放，单舰每窗口最多12架；本窗口参与过缠斗的飞机离场后仍不得攻击舰船/地面。

本窄执行只接受Low/Medium的Level/Glide和准确原文释放几何：Low及Glide从声明瞄准点上空，Medium Level距该点3海里且朝向该点。想定必须在`aviation_2026.surface_aimpoints[key]`明确给出有来源的`point`，并声明`controlled_range:true,fixed_target:true`；`surface_attack`命令必须选择该`aimpoint`，本方当前接触估计也须匹配这个已知点。命令准否仅使用本机、己方接触和明确声明点，不能通过真实目标位置作动作许可探针。此处是有显式来源条件的精确靶场子集，一般双盲接触轰炸仍未完成。

裁判若发现实际目标偏离固定声明点或正在移动，不猜偏差半径、命中或未中：实际投放和扣弹照常发生，完整原始裁判记录进入可保存的`pending_surface_attacks`，等待有来源的就位裁决。最终概率、D100、骰组、命中数、目标真坐标、物理尺寸和穿透过程仅在`Visibility::Referee`的`9.6.source_bomb_attack`中；Side只收到本机释放回执以及正常观测流程允许的结果，不能读取pending。把声明、邻接和火力阶段改成显式命令下的即时裁决，是记录在数据中的RTS过程改编，不声称逐阶段重演。持续180秒直线进场历史、完整自动轰炸航线和释放容差的可用性仍待进一步做。

NoAA必须由想定作者在`aviation_2026.no_aa_surface_attack_source`中显式声明并标明来源，例如受控无防空靶场的`scenario_assumption`；它不是从玩家可见态势推导的普适事实。没有该声明一律阻断，执行器不查询隐藏敌方单位、武器库存或裁判事件尾部，避免动作许可泄露真态。有动态防空环境的HeavyAA/NoAA账本尚未接入，不猜修正。受保护地面关键损伤映射未明时阻断。普通Fire/自动武器循环不得落回旧Bomb/Rocket的近似炮击路径。

`surface_bombing_2026`七项包含真实Glide/Level命令、6枚耗弹/4+2骰组/Level单机salvo1、外挂减载、30秒MR0、完整World重演及未知装配/错误释放几何/VLow/缺防空声明/缺瞄准点/缺目标来源/无显式载荷及外挂质量/非有限本机坐标阻断。首轮目标`cn_052d`没有已核物理尺寸，正确未释放；修正夹具使用`cn_001`的China2025 PDF10/A3来源A尺寸与零甲板装甲。固定种子2的Glide两组D100为18与36，来源35%基础行上移8行为75%，应逐枚进入六个35DP实际伤害；这项期望等待后续定向编译验证。缺NoAA声明的可执行性在不同隐藏武器库存下须保持相同；隐藏目标位置变化不改变本方命令/释放回执，裁判raw不进入任一Side观察，偏差pending保存后续演完整World须相同。

另验新operation/radar行只在v4可见，不回写旧表，并追加真实舰面生命周期红测：`LaunchAircraft`必须进入队列、出生时保留6枚外挂、同一个体回收后使用Small非制导挂载30分钟重装与40分钟警戒总4200秒准备、完整World恢复相同。原`operations::load_type`只识别显式guided、对任何Bomb返回未核，先红后由根代理仅对v4来源variant接unguided；旧v3规则不变。`third-core.log`记录真实生命周期通过；最后一个Glide红例为测试把受击方Impact和攻击方BDA的副本重复计成12项，按受击方Side过滤后，`artifacts/rules-next-20261023/validation/fourth-core.log`内`surface_bombing_2026`七项全部通过，六个来源35DP真实伤害已验证。该日志另有鱼雷straight红例，不能叫整份/全工作区全绿。攻击方精确BDA的既有信息边界不包含在本轰炸raw trace收口结论内，后续单独修。

Lob、集束、火箭、扫射、PGM的完整制导与烟尘状态、其他挂载和敌方实射防空修正继续分开待办。地面实际命中路径虽已接队列，独立来源伤害验收仍待补，不合并计为完成。已知坐标靶场子集不代表一般双盲接触轰炸完成。
