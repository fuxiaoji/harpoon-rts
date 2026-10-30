# 靠泊转库与搜救连续链：声明子集验收记录

本批声明子集已完成最终联合验收：**Rust126目标1596通过、0失败、5既有忽略，Node86/86**；实际HTTP、MCP stdio、跨进程恢复/完整重演及双语界面均通过。状态为 **`tested_declared_subsets`，`final_signoff=true`**，只签收本报告明确范围，准备脚本本身不作运行证据。

证据独占 `artifacts/rules-facility-sar-next-20261026/`；机器台账为 [rules-facility-sar-continuation-20261026.json](rules-facility-sar-continuation-20261026.json)。上一批353输入/1014证据保持独立，本批不回写其源码快照、失败、报告或旧战局。**整章、全书、全部可选和全部真实Annex装配仍为false**；无完整原子分母，不给完成百分比。

## 来源、作者输入与实时映射

权威仍为2026-01-01英文数字更新版 `14-2.HpnV_Rules_1_Jan_26.pdf`，SHA256 `8196ffc715bbcccd862d8597c607de7816dac5baee31fa659f89dfaf4323700c`。先检索三本中文核心RAG，再核对英文原页；本批复用既有直接渲染并保留其来源SHA，不改原PDF或旧图。设施与SAR准备合同、独立复核及后续修正均在新证据根下，旧准备文档中的“候选/未运行”是其落盘时事实，不由当前结果追改。

- 英文PDF116/11-1 §11.1：结构累计半DP或关键损伤后功能失效，满DP为不可修复残骸。PDF118/11-3 Ship Berth行为SizeB、400DP、armor0，失效后不能靠泊补给。Command Bunker的GCI失效、Magazine Bunker的D10回合gun-only是不同效果。
- 靠泊生产者的独立初存3、目标初弹2/容量4、0.01nm站位圈、0节、每弹3秒及精确Depot/Berth/recipient/mount指派，均为明确实验作者输入；不声称原书通用后勤数值、真实052D装配或实际基地性能。原数据库不编辑，实验初弹只在内存夹具或隔离overlay中声明。
- 靠泊同时性是明确RTS合同：命令期只收请求，在实际移动、共同伤害及火水等本tick效果后统一结算。先取消/动态失效，再复检到期原子转库，最后按稳定recipient/service身份预约新工作；同tick取消优先、相同请求合并、同服务不同数量拒绝。客户端先后不决定争库赢家。
- 英文PDF137/14-11 §14.8给人数/签名、无尾迹半程、超过六小时后D6节及符合当地地理的随机方向、停船且上风的舰船救起、海况修正和容量原则。PDF63/5-24、64/5-25给实际Visual表、同lookout/Tac骰和实际距离小于0.3kyd自动发现。
- 人数明确内部档为1–9 G/Stealthy、10–29 E/VSmall、31–99 D/Small、101以上 C/Small；30/100原文交叠，源helper和native均待裁。当前人数为尚未救起者，不因作者知道损失位置就给玩家marker。
- 每Tac舰船救起量为 `max(D6−max(SS−2,0),0)/2 × 10人`；半组保留为5人。原书容量文字是crew或passenger complement，本批passengers均0，只签明确crew/实验fit容量，不签通用相加解释。
- 连续180秒实际停船、在站位圈且上风才获得一次舰船尝试，是单列的RTS工作映射。原书没有逐秒bank/reset文字；站外order年龄不算工作，失格重置、实际尝试耗一整段、同Tac不重复骰。500m救援圈、90°允许流向及实验complement同样是作者参数，不冒充原书硬件常数或真实海流地理。

SAR损失夹具通过生产 `damage::sink_by` 接受**明确已裁损失的消费者边界**，随后真实Visual发现、公共JSON动作和正常tick救起；没有手填幸存者、Contact、FCS、HP、原骰或tick。它不证明完整自然武器命中→爆炸/击沉→全部生还原因。设施200DP半损、Depot原生sink及变侧案例也分别是已裁损伤/损失/所有权边界，未冒称一场实际攻击或自然俘获机制。

## 九条四路记录

各行独立列Source、Data、Execution、Tests；可共享测试文件，不能把同一案例反复相加算规则数。Tests依据最终358冻结输入及实际运行签收明确子集，历史中间失败仍保留，不外推整个机制或全书。

| 声明域 | Source | Data | Execution | Tests与当前范围 |
|---|---|---|---|---|
| 独立Depot与服务指派 | §11.1、Ship Berth行及明确实验合同 | `ground_2026.berth_depots/berth_services`严格字段；一个Depot+weapon只有一份初存；目标容量/时间/站位显式 | 唯一同方真实Base Depot/berth与Ship recipient；精确单一aggregate mount；首域拒任何Ground-target Depot、所有特殊物理装填与移动服务 | B01/B09/B10/B15/B16/B19；原十例先在schema丢字段红，B19另见特殊Magazine错误接受红；最终所引核心target通过。拒绝守卫不关闭Magazine服务 |
| 同阶段预约与实际转库 | 明确post-damage实时阶段/竞争合同 | 序列化requests/jobs，源库存/目标容量预约从共享账推得 | 收集后稳定排序；到期复检后一次同时debit/credit；取消优先、重复合并、冲突数量拒绝 | B02/B05/B06/B11/B12/B17/B18；中间18例已绿；最终21/21与实际HTTP/MCP通过。queued回执不保证取得服务 |
| 精确泊位失效与中断 | §11.1半DP/critical、Ship Berth失效 | 源设施状态与单独服务身份；未把共享parent当服务依赖 | 精确指派泊位失效、死亡、缺失、离位、运动/身份变更时取消并放预约；旧due不免费补弹；完整World保存 | B03/B04/B07/B08/B13/B14；实际200DP半损与native Depot loss边界，中间18绿；最终所引核心target通过。不称完整武器毁设施链 |
| 共同入口、权限、私有事件与双语控件 | 实验服务ACL及既有玩家控制合同 | 两个嵌套Ground动作、strict私有键拒绝、recipient专属本方投影、固定Service.owner审计 | shared只读预检和apply复检；变侧Cancel与unknown/foreign同拒绝；事件归原owner/缺身份Referee；UI仅用本方服务 | B09/B20/B21；实际事件曾发Red、Cancel曾错误通过的独立红保留。Node86已绿，最终核心/HTTP/MCP/UI均通过。HTTP两玩家请求共4合法cmd；本次UI已手控，只排1Ground并保持原manual，不称该单令新取得人工 |
| SAR人数内部与原生Visual | §14.8人数行、§5.8真实目视表 | `sighting_profiles`各内部档、0.5无尾流一次、30/100空隙 | 真正Ship Visual scanner/commit测量快照与本方不透明别名；不合成Combat Contact；无私有坐标fallback | C01/C02及原始发现前置红；实际测试9/10/29/31/99/101，30/100待裁；最终所引核心target通过。不把纯FLIR尺寸helper算完整FLIR链 |
| SAR连续工作、数量与容量 | §14.8停船/上风、D6/2十人组、SS每级减1、crew或passengers；180s连续性单列RTS | 可持久RescueWork；同Tac额度；显式station/complement | 正常Begin/RecoverDue/自动共用资格；每tick积实际工量，离站不银行化；尝试后清工、成功清旧pending；不超群余量/实验容量 | C03–C06/C09；C04/C06最早发现前置红，不能称该时海况/部分救起断言已红；C09实际离站200s后到站立救9人已红；最终所引核心target通过 |
| SAR真实存活与平台种类 | §14.8实际舰船救援；其他平台另有规则 | 实际kind与RescueFit分别校验，不由作者fit改变物理种类 | 手动及自动recover都核alive/actual Ship；原生沉没残骸不继续工作，Base/Sub不能冒Ship | R01–R04首跑1绿3红；死人残骸、Base、Sub各实际救起7人，阳性Ship控制实际通过；最终4/4。未扩称直升机/水上飞机完整链 |
| 严格六小时与有地理声明的漂流再发现 | §14.8“超过六小时”、原D6节及本地地理方向 | 明确`>21600s`；持久原D6/声明方向；未知current保持pending | 21600仍合法原位测量，21601后才建流；已知漂流点可真实Visual再测，未知方向不发布假静止新点 | C07/C08/C10；C07原始observed_at停21599红，C10另见21600提前建流红；改严格端点后C08最后快照预期同步为21600；最终所引核心target通过。不称全球海流/岸线漂流模型 |
| 观察者清理后的真实旧报告 | §14.8实见marker及明确持久化/隐私合同 | `SurvivorReport.side`可选原侧；旧None只据仍在的同方observer保守识别 | 新实际测量记录原侧；原观察者死亡/cleanup后同方保最后position/time/class；未知漂流不随私有真点刷新 | R05先静态后combined-1真正红：T121原报告尚在、T122唯一LOOK被清理后变空；含完整World恢复；最终所引核心target通过。不是免费持续航迹 |

新模块在 `ground2026/berth.rs`，SAR实际消费者在 `expanded_damage2026.rs`、`expanded_damage2026/sar_native.rs`，规则/RTS合同分别在 `ground-2026.json` 与 `expanded-damage-2026.json`。旧 `ground::resupply_allowed` 父级扫描helper仍未被当作producer；本批真实服务直接核精确泊位，避免旁边另一个坏泊位误封当前服务。`gci_available`也未因此完成。

## 原始运行与纠错边界

| 日志 | 已实际观察 | 不能外推 |
|---|---|---|
| `facility-source-red-1.log` | 根复制路径错误后找不到Cargo target | 未编译/执行候选，不算机制红 |
| `facility-source-red-2.log` | 正常编译0绿10红，最早均为depot配置Null≠3 | 下游库存、容量、时钟尚未到达 |
| `sar-source-red-1.log` | 正常编译3绿6红；较大人数发现、30端点、漂流重发现、离站借工实际红 | C04/C06先停在native发现门，海况/部分救起不能同时算独立红 |
| `sar-guards-red-1.log` | 正常编译1绿3红，死亡/错误Base/Sub各实际救7人 | 已裁sink与阳性控制不是完整武器损失生还链 |
| `combined-1.log` | 设施18绿、SAR9绿，R05旧报告清理后丢失1真实红 | Cargo在该target停止，后面的4 guard本次未运行 |
| `boundary-red-1.log` | `--no-fail-fast`：设施18绿2红、SAR9绿1红 | 分别为特殊Depot未拒、变侧事件泄漏、21600提前漂流；不是最终通过 |
| `cancel-owner-red-1.log` | 只筛B21，正常编译0绿1红，变侧Cancel预检错误接受 | 20个filtered不算实际执行 |
| `final-workspace-1.log` | 完整126目标1595通过/1失败/5既有忽略，唯一失败是MCP旧Legacy形状夹具期望新服务入队，源门实际正确拒绝`11.berth requires explicit hv2026-rts-v4` | 属夹具适用性错误，不能放宽来源门。根仅将两新动作纳入parse-only/Legacy必须拒分组26→28，仍真实反序列化；正域另由新核心与实际MCP验证 |
| `protocol/mcp-runs/mcp-final-1/` | 首次真实运行完成T20主链后，harness把双方公开`scenario_description`误当私密后勤而失败，原协议/退出回执保留 | 只修辅助检查公开字段的方式，未改生产、358输入或16夹具；第二标签实际重跑通过，不称首次通过 |
| `final-node-1.log` | 86通过、0失败、0跳过 | 不是实际浏览器或实体平板证据 |

原始候选、逐版生产副本、回执和所有失败均保留。GBK输出/辅助Python相对路径错误只作为工具辅助问题记录，不伪造规则红；修正辅助后不重写旧结果。报告也不把静态发现的成功后旧pending清除当作独立已见红，它由后续正常移动/成功用例断言覆盖。

## 冻结输入与最终验收状态

第一次358输入清单 `final-candidate-1-inputs.json` 的SHA256为 `9ef023b1b99373da2c4bd05ffa8f5de9c575094159881dca31344600a22ff4a6`；首轮整库失败及原源码副本仍保留。修正上述MCP夹具后，仅 `harpoon-mcp/src/contract_tests.rs` 一项输入变化，未改生产实现。根重捕获当前358输入为 **`final-candidate-2-inputs.json`**，SHA256 **`c29aa12ae8338444e6ba1dbaac9846aa5eef138c08b73eec843aa5df1ee06eba`**，实际源码副本在 `source-snapshot-final-2/`。验后 `after-acceptance-inputs.json` 与该最终清单逐字节相同、SHA仍为c29aa12a…6eba；原16项文件及前批1014份证据逐项SHA不变。二进制回执与外层总manifest由根单独登记，不改变本报告声明域。

| 验证 | 当前状态 |
|---|---|
| Node | 当前`final-node-2.log`实际86/86通过，第一轮同为86/86保留 |
| Rust整库 | `final-workspace-2.log`实际126目标1596/0/5、Cargo退出0；四个新target实际21/10/4/1全部通过，非规则数。首轮126目标1595/1/5保留 |
| 实际HTTP | `protocol/http-runs/http-final-3/validation.json`实际通过：7拒绝各完整payload不变；2玩家请求共4合法cmd，反序争库/到期原子转库通过 |
| 实际MCP stdio | `protocol/mcp-runs/mcp-final-2/validation.json`已实际通过：108对、0协议错误、8预期工具拒绝、子进程0退出；只签靠泊运输，不称SAR救援 |
| 跨进程恢复 / 完整命令重演 | HTTP同一回执：PID47248→48568同程序SHA，T0排队World/inbox与T20完整World/inbox一致；1批16事件完整重演`a09f0be9f6af1352` |
| 双语界面 | `protocol/ui/validation.json`实际CUA T20→24、1次点击1Ground、已手控保持；T21预留1/T24 stock1→0、SHIP2弹药2→3；2批19事件完整重演`c66eb7e1cd7f6176`，实体平板未验 |

隔离协议准备中的Depot初存、精确assignment与overlay均明确实验；准备目录保留原件副本，不把构造脚本算运行。HTTP在两舰尚未手控时，各玩家请求经既有运输排 `set_unit_automation(false)` 与Ground，两次请求共4条合法命令。当前UI从该HTTP存档恢复后已手控，运输跳过重复auto-off，一次点击只排1个Ground；不能据此声称单Ground新取得人工或排队时已完成转库。

实际MCP第二次运行PID22076，冻结程序SHA256 `83dd57b8240a747638c9e9a87015126edc886dcaf32fe77333f9078dc8105e92`。反序提交先SHIP2后SHIP，T1仍由SHIP/lab稳定取得2发预留，due7；T6无交付，T7 stock3→1/SHIP弹药2→4，SHIP2仍2，续至T20不重扣。另起实际取消/重提链在T2释放预约、SHIP2于T3重用库存、due9/T9一次到账，T20不重复。8非法请求各保持完整暂停双侧观察，下一真实step也无隐藏队列/库存/弹药/控制变化；Red仅得公共场景说明和All场景开始事件。

单条Ground在该MCP实际运行中两舰`manual_control`均false，不能声称自动持久人工；玩家UI本次已手控而仅排1Ground，其实际保持原manual另有UI回执。stdio108请求/108匹配响应及1个无响应通知、8预期tool错误、stderr仅启动横幅；关闭本方stdin后正常0退出、stdout/stderr EOF、读取线程关闭、无强制终止。14项MCP证据manifest及独立事后审计、16夹具/358输入保持的实际回执均在协议目录。SAR损失/发现/救起没有在这次stdio发生。

server/MCP构建 `final-build-1.log` 成功，既有`unused_mut`警告保留。冻结server SHA256为 `fba1312cc36e4b168ddc26055f2e28c1a138bfc5f5363dcfb03e15bd51094e70`，MCP为 `83dd57b8240a747638c9e9a87015126edc886dcaf32fe77333f9078dc8105e92`，实际进程回执对应相同冻结程序。

HTTP第三次实际运行逐项验证7拒绝：未知服务、零数量、超容量、超库存、错误接收舰、错误阵营、私有stock。每个都保存完整payload前后相同。两次合法玩家请求按反序提交，共4条明确命令，T1仍由SHIP/lab取得预留，T7一次stock3→1/ammo2→4，另一舰仍2。T0待执行队列检查点由另一真实PID加载，完整World/inbox相同；两进程至T20也完全相同，原生save全命令重演为1批16事件/`a09f0be9f6af1352`。跨进程实际覆盖的是排队T0，不冒称本次另有中途job检查点；中途job的完整World恢复由核心B07证明。

HTTP最初两次并非生产失败：`http-final-1`在非法私有字段的纯文本400响应处被helper强行JSON解析中断，未到合法转库；`http-final-2`误期待两条而实际收到4条合法排队命令，在推进前停止。第三标签修正辅助后实际通过，原failure/transcript/queued保存均保留。HTTP运行后为UI helper增加独立plan_id前缀，不能拿后来辅助脚本代替该次原始transcript。

实际界面从T20恢复局选择SHIP2和本方服务，保留默认1发草稿，完整payload保持；只真实点击一次。因before.manual_control已true，此次只排1Ground，全部非inbox payload仍相同。T21真实预留1/due24，T24 stock1→0/SHIP2弹药2→3、预约清空、manual仍true；无库存补给按钮、无任务取消按钮禁用。UI完整重演2批19事件/`c66eb7e1cd7f6176`。这里保的是既有人工状态，不声称该单Ground自动取得持续人工。

UI helper先误期待2cmd，再遇恢复历史已有plan_id的409 duplicate_plan，两者均在推进前；修辅助后沿同一真实队列审计并用独立前缀推进，没有再点或重发合法补给。中英各1600×1000与1024×768的DOM实测无横向溢出、地图存在；按钮/数量输入约44px，select40px，不能称全部控件均44px。浏览器捕获error日志为空；viewport已reset，IAB tab3留作交付、最终中文。截图在对话展示，没有本地截图文件、实体平板或SAR界面发现/救起正域声明。详细辅助纠错见 `protocol/harness-corrections.md`。

## 保留余项与后续顺序

GCI原生surprise/管制生产者、Magazine Bunker完整gun-only服务、所有物理逐架/鱼雷管/RBU重装、油料/AIP与在航补给、真实Annex后勤装配继续开放。SAR的FLIR/cue/夜间/云冰/其他平台组合、30/100源交叠、crew与passengers同时非零的容量解释、上风零边界、完整自然损失/爆炸/坠机生还、目击损失自动marker、全部地理漂流与胜利计分亦未完成。

全书规则验收后才按用户顺序推进无UI并行训练引擎、Lua TCP、蒙特卡洛、交互式独立仿真、XML明文剧本、DIS对接；综合近岸高端装备想定与收益/侦察反潜/饱和/航空迂回/EW护航共享FSM仍在其后。本批不提前记录这些交付，不新增真实LLM比赛，不提交或推送。
