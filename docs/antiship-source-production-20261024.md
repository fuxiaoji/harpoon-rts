# 精确 RGM-84D IC 来源反舰链

本包先注册typed配置、37号Antiship Plot命令、完整存档字段、精确新ID与显式blocked stub，再由根代理运行真实Plot→Fire。两条均在实际发射数0≠1处失败，见 artifacts/rules-antiship-20261024/source-red-first.log；配置、挂载、来源型号与Plot已编译并载入。生产裁决仅在该真实结果之后实现。首轮生产编译通过、6项中3绿3失败，原因是测试预期的弹体编号少1；已修正预测编号并保留47/48边界及全World断言。另补入口本方CID与共享validate_plot检查。根代理core-corrected.log已实跑7条native与4条共享入口全部绿色；这只说明两个定向target，全workspace与控制器/MCP另验，此处不称全章完成。

来源先查核心RAG，再亲视英文2026 PDF74/6-6、76–78/7-1–3、94–95/8-14–15和119/12-1；精确舰卷IC、Ford与备注来自America Navy2025 PDF7/A4、77/D1、79/D3。原页、RAG与来源事实位于上一包只读 antiship-source/antiship-design。完整320格未更改。

新ID us_rgm84d_ic 只在显式v4注册，表示舰面发射RGM-84D IC；旧us_harpoon、空射us_agm84与v1–3数据及指纹不改变。原行 I/TARH/3、3–80nm、561kt、VSmall、40DP+D6/2、9cm、L/W3分别有独立字段。L为可选popup，M才表示重攻；IC无M，当前no-popup首例不得获得苜蓿重攻。registration中的salvo1/reload0是明确作者即时物理释放占位，不是IC发射器射速来源；实际准入要求作者launcher完整max_rounds_per_turn，依已保存的本回合发射数限制，不能借旧45秒自动推定。

所有入口通过本方selected ContactId、真实Plot、来源挂载/库存与own FCS，不读取秘密敌alive/kind/class/位置来决定合法释放。作者Good/知道被攻击只存在Referee配置，并有scenario_assumption标签；玩家Plot只有本方航向、速度、enable、waypoints、popup及contact/weapon，严格拒绝注入quality/敌ECM等字段。FCS使用真实观察者报告时不从共享接触自动猜TDL。首受控平台是明确作者试验平台，不能被描述成真实舰型已核挂载。

发射后保存完整Flight与Salvo。实际半程越界仅设标记，在该tick Detection结束hook掷6.3.8 D10；同射手同exact武器类型同180秒逻辑turn共用原D10与两D6。失败在真实下一腿应用左右D6及幅度×5°，排除原定目标。不同FCS质量或绘制路线共组暂明确拒绝，不能借旧解缓存或默默按目标另分骰。

启用前按本方绘制航向飞行，不追真位置。启用后按实际位置、总45°/半角22.5°搜索锥和Gen3 TARH对Large/Medium的25nm范围筛候选；所有敌我舰及有效EW对象均须面对来源资格，不以“已绑定敌舰”过滤掉未知候选。首包只执行明确clear环境、单一原定source Ford候选；多目标、他舰、EW/缺源候选、杂波、成功就位与实际几何不符都保存完整Referee pending，不回旧链。

Ford源参数为Large和联合Gen4 J&D，不能拿搜索雷达Gen6代替。反制只在有明确本方来袭知识证据时折入8.4.1；受控例的作者知道来袭与硬杀伤不可用单独标示。每枚导弹独立源D100，≤47锁定、48失败，概率不受legacy pk/chaff库存或每秒.45诱骗影响。锁定后仍须真实飞行到接触点，存活才调用原来源损伤队列。已实际拦截则不入命中队列。

失败的no-popup IC保持原航向并消耗剩余航程，不每秒重骰、不改跟原目标、不增加无M重攻。所有原骰、候选、阶段、最后实际位置/航向、未完理由与完整弹体随World保存；pending不通过显示事件环冒充存档。

own observation只输出自身Plot与执行器可用标记，不含敌signature/ECM、候选、placement、概率、D100或锁定结果。秘密证据统一Referee trace，不能复用默认向actor侧输出的通用aaw trace。本方库存/释放回执仍来自真实物理发射。

IC来袭枪炮入口另经真实红测：incoming-aa-and-mr-first.log中现有AAW25项绿色、ASCM10项9绿1红，唯一失败为实际登记no-popup Flight因getter笼统缺MR而无法被枪炮裁决。之后新增no_popup_incoming_profile：signature3，MR保持null，其余无末段机动/非高俯冲/非sprint条件来自IC源行、实际无popup Plot与英文89/8-9。只对真实登记的无popup IC返回该行；popup/缺Flight仍待裁。core-joint.log实际AAW25、ASCM10、入口4、MR6全部绿色，共45项。真实Ford雷达仅用RCS可算、Visual缺实体尺寸待裁，以及strict SAM未知MR拒绝均在新增负例中已验。

最后另注册实际近防炮开火用例：本方已分类Missile接触→manual_fire/fire_at→原D10084、Ph0.84击落→保存Flight Intercepted，完整World恢复后续演500tick不入战斗部损伤队列。用原seed同时选择8.1.5独立碎片10%未中，并检查原碎片事件存在，不能把击落无条件等同无碎片。这使原生共11项；antiship-final-targeted.log已实际运行全部11项绿色，不能重复加入上轮45项通过数。source metadata状态为targeted_passed_joint_pending，全章仍false。

未完成边界：新IC incoming物理尺寸与MR仍缺原页，雷达VSmall不等于物理size，不填MR0；严格SAM仍因MR待裁，不因此整体阻断独立枪炮AA输入。真实完整Ford硬杀伤/探测装配未闭；当前已知来袭、硬杀伤禁用的主攻击作者隔离例及单独作者PD几何不能替代自动防空全链。popup/W3路线、其他型号/发射方式、多目标/他舰、复杂decoy、非clear杂波与带M重攻另待来源执行和独立验收。
