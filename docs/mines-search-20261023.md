# 水雷扫描来源纠错与工程子集（2026-10-24）

当前定向验收：错误的 Type193M 自动参数已撤销，真实profile拒绝、显式作者夹具与源表边界14项全部通过。生产、数据、测试与两类HTTP夹具继续冻结；完整工作区和真实HTTP仍待主代理最终验收，不将此定向绿等同整包完成。沿用 rules-after-following-20261023 窗口，上一 following877 冻结证据未改。

此前“Commonwealth2025 PDF90/K-2 的 Type193M 为 Gen3/HFa/Keel/0.4nm”是来源错误，现撤回。K-2没有 Type193M 行；PDF65/A-61仅证明 M1151 于1971装配 Type193M 猎雷声纳，不能补齐代次、频率、安装或50%距离。旧 source-facts 的 Type153、Type162 行也混读；本次只保留重新亲核的行，其他候选全部待核。旧错误草案、数据、报告和HTTP另存 mines-search/source-correction-20261024/retracted-*；旧红绿日志保留原字节。旧绿只证明那套输入的执行，不能支持错误来源。

按 harpoon-v-rules 重新检索核心RAG与具体型号，亲视英文2026 PDF49/5-10、50/5-11、51/5-12、52/5-13、126/13-5和 Commonwealth2025 PDF65/A-61、90/K-2。有效事实见同窗口 source-facts.json，原PDF与图片哈希见 source-manifest.json。原PDF只读。

| 项目 | 已核事实 | 当前实现与范围 |
| --- | --- | --- |
| 真实 Ton M1151/Type193M | 1971装配说明有据，所需接收器参数未知 | published profile 保留 blocked/TBD，数值null，不注册数据库接收器、不创建检测绑定、不发回波 |
| 底雷搜索与强度 | §5.4.6.1专用MH才能探测底雷，雷体主动强度VSmall；PDF49 E–G行=-1 | 独立Body，复用声纳修正表，不借投放舰尺寸、噪声、航向或消声瓦 |
| 作者工程夹具 | 虚构平台/接收器 Gen3/HF/Keel/0.4nm 全部显式作者输入 | 仅 scenario_assumption: 身份与源标签；报告携带 evidence_basis/source，不授真实型号来源资格 |
| 最近印刷中心格 | PDF52取最近格，PDF50–51完整非等步中心0.5至100nm | 任意正有限modified50含端点，先选超区间端点避免极大浮点假tie；内部等近继续待裁，不增行或外推概率 |
| 实际搜索 | 每接收器/雷体/180秒保留原骰，实际开机、健康、电源、盲区、代际模式和源修正 | 本方opaque未分类回波；精确米深、显式平底环境、仅直接Keel路径，缺条件留裁判阻断 |
| 分类与中和 | §13.8.3要求搜索→分类为雷→逐雷中和 | 回波不进入ClassifiedMine、普通contacts/FCS或计分实体，不授Hunt；分类率/时钟继续TBD |

实际入口 mines2026::step → search::step 扫描所有未中和物理雷体，不以雷owner过滤。本方报告只含opaque ID、classified=false、首末成功时刻、observer/sensor、来源类别/说明与报告几何；BodyID、型号、布设方、真深度和失败骰只在裁判状态/事件。已有回波不因隐藏真态变化立即消失；其他接收器的成功不能代替当前设备的实际状态。

SearchFit.scenario_assumption 明确声明虚构接收器。平台ID与两处source须带 scenario_assumption: 前缀，主动传感器ID须带 sonar_a_scenario_assumption: 前缀以兼容原生开关，并检查有限正距离、支持代次与Keel安装。实际SensorDef仍须由作者数据库提供并装舰，配置不自动增加器材。真实身份、假标签、缺参数、非法距离/代次/安装均不能走作者分支。报告保留作者标签且可保存恢复，旧缺省字段兼容。

新拒绝红测见 validation/source-recheck-red-20261024.log：13项中11绿/2红，准确复现坏来源仍发回波和 f64::MAX 伪tie。原 mine-search-turn-red.log、joint-source-red-green.log、turn-mine-green-wake-red.log保留历史含义。原生电力恢复第11例已在新红窗实过：T599保存、T600原生修复、T601扫描，不由测试提前清故障。

当前14项覆盖作者标签、Side/隐藏远雷/owner反事实、实际设备状态、独立原骰/全World恢复、精确深度、源修正、真实profile阻断、浮点端点/内部等近与作者假设不能伪装真实装备。本轮定向结果见 validation/contact-mine-green-wake-group-red-20261024.log：矿目标14通过/0失败/0忽略，0.16秒，包括真实193M阻断、作者正例/标签、MAX100与距离尾端、原生断电恢复和全World存档。该混合日志的Wakegroup仍有预期红，不能据此称整包验收；动态布雷19、常规水雷11、detection61及完整工作区/真实HTTP由主代理收尾，不沿用旧坏source绿作为本次来源验收。

HTTP正例为 mines-search/http-scenario.json / http-volume.json：想定 authored-mine-search-http，actor ASSUMED MH1，虚构ID带作者前缀，仅验算法、标签与存档。负例 http-blocked-scenario.json / http-blocked-volume.json 使用真实Type193M profile声明及刻意提供的虚构数字支架，必须仍无绑定、无回波，不能由手填SensorDef绕过来源缺口。旧RNG见证对应旧传感器键，已撤回为历史；实际HTTP结果由主代理记录。

真实MH全参数与装配正例、矿分类器、离底系留雷、间接传播、起伏海底路径、SQQ32/Type2093未知距离和PAP/SLQ未知耗材仍未完成。完整搜索→分类→Hunt链与全水雷章未完成。


补充纠正：首轮修复把仅PDF50末行15误当完整表端点，已亲视PDF51续表，正确最高中心100。生产始终使用完整表，未裁剪行或改变概率，只纠正测试oracle和说明。contact-wake-timing-red-20261024.log中矿14项4绿10红，含这一错误期望和9个关机作者正例：原生初始化与SetSonar仅识别sonar_a前缀，原虚构ID不符合。当前改用 sonar_a_scenario_assumption:active_mh_receiver 并补真实开机断言，平台/两source/本方报告仍保留作者标签；14项现已定向全绿并保持冻结，完整工作区与真实HTTP仍pending。

2026-10-24最终联合验收：完整工作区59目标933通过/0失败/5忽略，本分项日志及HTTP/历史重演范围见[本包报告](all-rules-after-following-20261023.md)。前文分项与中间红绿为历史记录；整章/全书和明列待裁项仍未完成。
