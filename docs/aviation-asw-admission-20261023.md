# 航空反潜投放准入：本方证据与物理裁决

状态更新至2026-10-24：after-following本域Mk54十八项定向验证已通过，等待整包最终验收。最新`validation/contact-mine-green-wake-group-red-20261024.log`记录Mk54十八项与旧鱼雷四十项全绿；该联合日志仍有Wake分域红例，不能称整份日志或全工作区全绿。较早`validation/joint-source-red-green.log`的五个航空目标48次执行（当时Mk54十三项）保留历史含义，不改记成新版五目标53次已执行。上一following包56目标877/0/5是独立已验批次。全书、第9章及完整9.7均未完成。

## 来源和解释

按仓库Harpoon skill先检索三本中文核心RAG，再视觉核对权威Harpoon V2026-01-01 English PDF109/印刷9-8 §9.7.1、PDF73–74/印刷6-5–6 §6.3.7–6.3.8。复核图片、RAG结果与提取文本在根`artifacts/rules-after-following-20261023/aviation/`。源表Good/Fair/Poor就位9/6/3及其修正、偏航和后续Ph仍由原来源执行器消费，本包没有修改这些数值。

9.7.1要求飞机知道潜艇位置，可以来自自身探测器或友方通信；同时满足Low、至多半FMP、每Tactical Turn一枚，直升机允许悬停。知道位置、来源分类为Sub，并不等于获得未观察的实时存活、精确深度和真实声学型号。正常许可必须由本方当前located报告、来源类别、真实本机Flight/挂架/库存/高度速度和来源FCS共同成立。

对未知声学配置或实际不适用目标，应先记录真实释放与一次扣量，再由裁判的物理分支确定可否捕获和攻击；缺资料保存Referee pending。不能通过命令是否扣弹把隐藏alive/depth/class变成查询接口，也不能把共享quality当作Good解。

## 实际调用与数据

`aircombat-2026.json`新增v4独立`executor_revision`及`airborne_asw.own_evidence_admission`、`range_boundary`说明。该表已经属于v4表指纹，旧版操作表和冻结following快照保持对应版本。

`weapons::manual_fire`在目标真实body查询之前将v4 Torpedo/Lwt转交`source_torpedo_release`。该分支保留命令所选ContactId并检查本方发射条件，再调用`torpedo2026::fire_at_contact`；不先用真实目标存活、combat_kind或通用targets表拒绝。

自动扫描保留挂架顺序。v4源鱼雷用`source_torpedo_contact`先取本方指定报告；只有自由开火时才从本方当前located、来源已分类且武器允许的目标域选择。距离排序采用己方报告的估计位置，禁止读取真实目标位置/存活/型号。原自由开火`quality>0.25`只选候选报告，不能替代独立源FCS。未指定且停火不释放；普通武器延迟调用原解析路径，旧profile继续原行为。

`aircombat2026::air_torpedo_gate`保留有效Flight、本方当前located/已知Sub、Low/悬停、半FMP和滚动180秒时钟，移除目标真实alive/kind/depth谓词。`torpedo2026::fire_at_contact`的隐藏alive/目标声学profile前置门已移到物理路径；Mk54在实际捕获/终端阶段必须遵守来源submarine/surface能力，不能因错误Sub报告对实际Ship造成伤害。完整物理待裁证据只给裁判。

独立复核发现同一裁判body的新旧报告并不等于玩家已确认是同一轨。三个玩家入口现将精确ContactId贯穿资格、估计位置/距离、发射方向和LaunchRecord；所选陈旧或仅方位A不能借另一located B通过，所选有效A也不能被排序更早的B替换。兼容`fire_at(...target...)`只接受唯一合格报告，出现多个候选时拒绝猜选；实际玩家命令用精确接触入口。

9.7.1物理互扰仍按实际弹体判断；原Side事件中的`both_miss`和秘密弹数已改为Referee事件。本方仍收到自己实际入水/下次投放时刻与一次扣量的回执，秘密另一枚空投鱼雷的存活不再改变这些即时玩家事件。裁判物理结果可以不同，不能将它当作本方已知结果。

本包的验收范围限于上述ASW源鱼雷准入链；普通武器、所有战损观测、共享TDL与所有附件型号不能据此泛记完成。

## 红测与验收边界

`mk54_external_fit_2026`保留上一包四个精确MH60R四Mk54/每枚285kg fit测试，新增九个真实入口用例。主代理已在`artifacts/rules-after-following-20261023/validation/turn-wake-asw-red.log`留得旧四绿、新九红，然后才改生产。

新增测试覆盖：手动alive/depth/class三例；指定目标与自由开火自动alive/depth/class三例；手动与自动的缺接触、陈旧、仅方位、未分类/错误分类、缺源FCS负例各一例；以及真实浮航目标/缺声学资料的600秒物理无伤与裁判待裁一例。合法释放实际检查4→3弹、外挂1140→855kg、同己方回执和完整World保存恢复。水下环境、目标声学卡与Good解均为明确scenario_assumption夹具，不冒充完整MH60R实装FCS。

第一次负例曾把本方报告移到1000海里并根据Mk54六海里run期待拒绝；核对后发现没有初始目标中心硬门来源，该断言已撤出。当前`distant-report-without-source-fcs`同时移远报告并删除显式来源解/本机轨迹，只按缺FCS拒绝。不能为通过该测试添加一个未证的距离阈值。10.2规定的实际run仍从发射datum测量。

`artifacts/rules-after-following-20261023/validation/joint-source-red-green.log`中本域五目标合计48次测试执行通过，其中Mk54十三项包括新增九项。实际隐藏alive/depth/class的手动、指定自动和自由开火反事实，己方证据/FCS负例，600秒物理域与未知声学待裁、完整World续演均已执行。旧鱼雷四十、检测六十一和核心库一百一十九项也在同日志对应目标通过；这些回归不重复计入航空机制分母。完整工作区、服务存读档与旧版本重演结果仍由本批主代理最终验收后统一补充。

2026-10-24追加五项真实红测：手动所选陈旧A/当前B、手动所选仅方位A/located B、指定A且禁止自由开火、有效所选A的位置与释放身份、秘密Red空投弹存活反事实。`validation/contact-wake-timing-red-20261024.log`实录原十三绿/新增五红，修复后`validation/contact-mine-green-wake-group-red-20261024.log`中Mk54十八项与旧鱼雷四十项通过。测试先经真实Red Fire生成秘密空投弹，再仅变其alive；未手造互扰标志或绕过释放入口，逐变体完整World恢复一致。十八项是工程测试次数，不是规则条数或全章完成度。

## 明确保留的下一来源项

R-054继续开放：9.7.1写明入水后按6.3.8判定就位，成功后立即攻击（10.2.5）。当前`torpedo2026::advance_flight`仍以`placement.is_none() && run_nm >= initial_range/2`触发首次就位，空投Search也共享这个通用半程门。当前十八项没有关闭同tick入水就位/成功立即攻击。后续须共同核对10.2.5与实际投点几何，先留来源失败案例；不通过改Ph、改目标真实深度前门或猜投点半径绕过。

明确Good的极远报告是否应被其他来源位置规划/RTS许可程序拒绝仍为TBD；不记成初始范围门已完成。完整共享声纳/TDL火控、目标深度报告程序、航空常规/核深弹、真实击沉确认和完整附件挂载继续保留。

2026-10-24最终联合验收：完整工作区59目标933通过/0失败/5忽略，本分项日志及HTTP/历史重演范围见[本包报告](all-rules-after-following-20261023.md)。前文分项与中间红绿为历史记录；整章/全书和明列待裁项仍未完成。
