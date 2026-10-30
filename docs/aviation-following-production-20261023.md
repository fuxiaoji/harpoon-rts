# 航空续进包：准入边界、己方声明与明确外挂

上一包已冻结为Rust823通过/0失败/5忽略、Node67；其表指纹、程序和战局证据保存在`artifacts/rules-next-20261023`，不重解释旧v4档。本包的新来源与红绿记录另放`artifacts/rules-following-20261023`。本页只关闭已核航空子集，不表示全章/全部附件完成。

## 表面投放准入与合法瞄准点

先按三本核心资料检索`9.6.4`、空袭表、评估；中文核心105–107页与玩家表交叉，英文Jan2026 PDF105–107/9-4–9-6的原页和表格已有视觉记录。尺寸/装甲属于裁判攻击裁决输入，原文没有让一个已定位但型号未知的接触自动公开这些物性。英文PDF75/6-7 Assess和仓库双盲契约要求将来源接触与观察战损分开。本包因此用显式v4投影策略处理输入许可，不编造尺寸识别概率、装甲估计或偏差伤害。

`surface-boundary-red.log`已确认原7项通过、新4项失败：隐藏目标是否有来源尺寸、设施是否受保护，会改变己方投放许可；没有Side分配的全局point key可被任意一方猜用；己方观察又没有合法参数发现接口。

现已落生产：扣弹前只读本机fit/弹库/显式载荷外挂、本方current located且已知Ship/Base类别的接触、本机释放几何与时间账，以及想定明确分配给本方的来源点/NoAA条件。有效释放一次减弹、一次减外挂并先给本机回执，之后裁判才查看目标真态。缺目标/已失效/类别不符/缺来源size或armor/受保护地面critical未核，保存`pending_source_target_properties`；偏位或移动保存`pending_source_placement`。没有来源不产生概率、骰组、命中和伤害，实际释放仍被消耗。原75% Mk82例、4+2骰组、Level单机salvo1和35DP/穿透4均未改。

`SurfaceAimpoint.allowed_sides`为想定作者的Side列表，缺省为空。未知key与未授权key统一拒绝，不能靠真实目标类型或装甲做许可探针。航空模块自己的`own_view`仅输出本方有效点的key、声明坐标/source、受控固定条件与该点可使用的显式NoAA声明；不带隐藏entity id、真实位置、尺寸、装甲或敌方点集合。此schema属于有记录的RTS想定授权接口，不冒称原书JSON规范。一般双盲接触轰炸、保护设施、动态防空修正仍未完成。

`artifacts/rules-following-20261023/validation/following-core-red-green.log`已验证表面投放11项全部通过。随后只读审查又发现：未授权请求先检查全局NoAA，会在不同作者声明下给不同拒绝理由。新增真实反事实测试比较已知但敌方点/未知点、全局NoAA存在/缺失四种反馈；`following-core-second.log`已留该项红、旧11项继续通过。之后才将授权检查移到读取NoAA之前，`following-core-third.log`已验证12项全部通过；己方输出字段用`no_aa_declared`准确表示声明存在，不冒称整条动作必然可执行。

## F/A-18E同机体构型受油

RAG空中加油命中中文核心39/40页，原4.8流程与伙伴加油时钟保留。再次视觉美国航空2025 PDF92/B83，F/A-18E/F接收燃油率5.2kg/nmi，伙伴商数法也明确列在备注2；Mk82构型是同一B44机体。计划仅在独立v4接收率数据补该alias，不增加该轰炸构型的buddy装置或加油机资格。

真实新增测试已进入`aviation_2026`：受油方为明确6395kg Mk82装配，加油方仍为显式300USG/920kg伙伴装置。要求720秒真实完成、返航预算、重载耗油、外挂不变和完整World保存相同。`following-core-red-green.log`记录旧20项通过、新受油项因缺接收率而真实失败；之后才补独立v4的5.2来源alias。第二轮仍被后续真正来源门拦住：新夹具没声明精确机型翼表，而consolidated Ford默认翼只有fighter/striker等占位类型，返航储备流程正确找不到兼容回收平台。补显式想定翼表后，测试还要求删除该翼表时真实`apply`拒绝原因为`no compatible friendly recovery base`，保留原生产限制；`following-core-third.log`已验证航空21项全部通过，没有用放宽回收门代替5.2来源修正。

## MH-60R BlockI明确四Mk54外挂

三核心RAG空投鱼雷命中中文核心109页，对应英文109/9-8；真实就位、每机时钟和互扰已有独立三项验收，但旧夹具是明确mass-neutral，没有证明外挂质量。

本包重新视觉美国航空2025 PDF69/B64、PDF93/E1 AnnexF，证据在`artifacts/rules-following-20261023/aviation-draft/us-aircraft-p069.png`与`us-aircraft-p093.png`。B64明确BlockI的4枚Mk50/54构型、最大payload1480kg；AnnexF的Mk54与Mod1均列285/292kg，备注C明确直升机/固定翼版本，45kg是战斗部而非挂载质量。海军附件PDF84/F2独立核对相同重量。固定翼292kg、内部携带、HAAWC套件和Mod2不能由直升机285kg静默覆盖。

已先落独立v4来源构型`us_mh60r_mk54x4`，继承原MH60R机体来源行，准确4枚Mk54、总质量1140kg，不带替代四油箱或可选Hellfire；泛型两弹`us_mh60r`不被追改。先落来源fit数据/数据库构型装配，再跑三项真实测试；`following-core-red-green.log`记录精确fit的质量初始化两红、泛型/不完整声明一绿，并非unknown class失败。之后才把初始化、成功释放和Jettison接到同一精确host+weapon质量绑定，`following-core-second.log`已验证三项全部通过。

后续读取实际入口还发现：无效外装配令Flight状态缺失，但空投许可只读机体spec仍能发射，不能以“没有质量账”冒称实际攻击已被阻断。追加第4项真实Fire测试后，`following-core-third.log`已实际留红：未知host或不完整外部声明仍能生成鱼雷。之后才在`air_torpedo_gate`要求初始化的有效Flight，检查先于任何目标真态；明确mass-neutral机制夹具仍有自己的Flight，不被新门误拒绝。`validation/workspace.log`已验证Mk54四项、旧ASW三项与aircombat八项全部通过。

期望真正Fire请求两枚仍只投一枚：ammo/external4→3，质量1140→855kg；剩余三枚投弃归零且不新增攻击，高度/速度拒绝不扣量，完整World续演相同。不能只增加一个重量辅助函数或让所有Mk54库存共用同一个数值就计此项完成。

源鱼雷水文、受控目标参数和Good就位解在测试中仍明确标为`scenario_assumption`，不把这些夹具称作真实型号全套声学与火控已验。现有空投门还读取真实目标alive/kind/depth判断潜航，而己方known_category不等于已有深度/存活观测；本包没有关闭这条航空反潜双盲许可余缺，须下一包来源化处理。一般反潜构型、航空深弹和其他附件继续待补。

## 联合绿验

根代理统一运行`artifacts/rules-following-20261023/validation/workspace.log`：完整Rust工作区56目标877通过/0失败/5忽略；其中航空勤务21、Mk82投放12、精确Mk54外挂4、旧ASW3、aircombat8均通过。工程测试数不是规则分母。源码、来源数据和测试已冻结，后续HTTP/跨进程/旧局由根代理另验，不把其待验状态算作已完成。本包只新增独立v4表/构型/授权策略，旧v2/v3数据表和冻结next程序未修改。
