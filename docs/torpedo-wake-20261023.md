# After-following：ASW许可与Wake来源链

当前状态以 [2026-10-24 续接验收说明](torpedo-wake-20261024.md) 为准；下文保留前一实施窗口草案及当时测试状态，不能把旧夹具的绿结果当作已核半程时序通过。

2026-10-24 续接复核：上一轮终段/拖曳/500yd实现已由根统一编译，`source-recheck-red-20261024.log`中原11项Wake全部通过；新注册的实际另舰穿尾流案例真实失败，确认不能只在文档列TBD后仍自动击原靶。其后已实现局部真实分叉pending门，新增两组不误停反例与完整pending序列化断言，现14项待根统一测试，未提前记为通过。分叉仍未自动掷ACM/改跟，保留完整弹体、实际交叉舰/线段/交点/时刻及原骰给裁判。

独立原页复核另指出通用§6.3.8的半程后首探测阶段及同回合同类武器共骰，尚未和当前Wake首次穿流placement时点对齐。该时序不宣称严格规则闭环，不自行添加无来源的覆盖解释；具体启用门与修复由根统一处理。11项绿只证明当前明确子程序，不等于Wake全机制验收。

本包接在877/0/5冻结包之后，证据写入`artifacts/rules-after-following-20261023/`。上一包`rules-following-20261023`证据、存档和程序不改。本文件是正在实施的分项账，不能据模块存在称Wake或全章已完成。

来源已按仓库harpoon-v-rules流程检索中文核心RAG并视觉复核英文2026 PDF80/7-5、110/10-1、114/10-5、28/3-6；精确型号另核Russia2025 PDF90/F-1与91/F-2。图像、RAG、逐页文本、来源事实与前期红例草案在after-following/wake。PDF90打印页为F-1，F-2是备注页，不混页码。

| 规则子集 | Source | Data | Execution | Tests |
|---|---|---|---|---|
| ASW发射准入与物理目标分层 | §9.7/10.1允许按本方定位/分类/源火控解与己方投放条件决定释放；对潜专用无对舰DP，不能伤真实Ship | 现有Mk54源参数与执行修订说明；不新增无明文的initial target-center range硬门 | fire_at移除truealive与实际class元数据前置检查；advance的实际domain保留，未知class/目标缺失入完整Referee pending；航空组接共同manual/auto早分支和9.7门 | 原13项ASW目标中的9条新红已留存，修复待联合检查；2条无据1000nm拒绝已改为远报告且缺源FCS，不伪称射程硬门来源 |
| 65-76A精确源fit | F-1：Wake-H/3、50kt/27nm、I、197DP对舰/无对潜、650mm、P/S、1991–2002；F-2备注E海况≤4；P/S核心表0–25m | torpedo-2026/ru_65k源定义；离轴不设默认，源海况/运行深度字段；历史1998测试 | 所有实际发射消费同一SourceWeapon与物理初装；无来源offset拒绝；海况5/运行25.1m拒绝 | 首4实入口组已有1绿3红；新海况/深度负例待联合 |
| 明示尾流几何和actual crossing | §7.5.5/10.2.6必须穿真实尾流才寻的；宽度、寿命、前史没有此处默认数值 | WakeConfiguration.source/half_width/lifetime/history_policy/显式initial_segments，全部作者条件标注；无None默认几何 | 自真实Ship运动生成有时限线段；扫掠胶囊距离与相交点；stable segment/crossing ID | 真实Fire、无前史静止船不锁、穿越后变相位、在航保存；已经真实入场拒绝红后接模块，等待绿证 |
| placement+1和75%made-good | PDF114/10-5；额外射程限制须对齐110/10.2地理发射点测距，不能凭燃油直觉另将27nm减20.25nm | wake_placement_modifier=1、wake_progress=.75 | 原始源D10缓存；沿时间顺序且几何连续的尾流链前进，缺链不直取隐藏目标；50kt的闭合37.5kt | 独立60秒0.625nm锚与保存恢复待联合；失配/过期或未实现终段保完整pending，不称自动补全 |
| Wake终段/25kt且90°规避 | PDF114：Gen3无规避.70；实际探测后至少25kt且90°，evasion-only+.05=.61 | 已核基础表及wake_evasion_add等字段 | 当前终段明确pending，待新真实红留证后实施；源Ph不改 | 新真实terminal伤害/实际SetCourse+SetSpeed四反例已注册未跑，不算红证 |
| Gen3/4towed独立双骰 | PDF114：先ACM-only抗骗，失败鱼雷与拖曳器均毁；通过才独立终段base或evasion-only+.05 | gen3/4 eligibility、既有完整表；多有效对象优先级未明不猜 | 待真实红后接EW consume_towed；当前明确pending | 真部署早代towed/非towedACM/jammer排除、多towed pending、固定原始种子42失败或16通过再92 miss已注册未跑 |
| Wake500yd右180°再攻 | PDF114/10.2.7专段 | wake_reattack_past_yards=500、right_turn=180；共享准确yards换算 | 当前past/return明确pending，待新红留证后实施 | 真实miss后500yd=0.2468682505399568nm/角度/保存原骰案例已注册未跑 |

随机案例使用原始Rng固定种子和purpose，无生产forced_die。常规seed20261023/id3：towed42、attack34；seed26/id3：towed16、attack92。纯表边界与真实模型执行分开计账。

RTS几何适配：书未给尾流宽/寿命/前史，故实例中0.02nm半宽与600秒只是明确作者条件，生产无这些默认值。跟随按源线段到其端点，再接时间更晚且端点连续的同船线段，不把捕获后的直达真实舰位当尾流引导。断链、过期或多义分支尚无明确继续规则时保留完整弹体和原骰给裁判，不偷偷改成声学或legacy命中。

仍未完成：另一舰穿目标尾流的实际分支诱偏；VSmall映射与多分支/多towed顺序；完整失锁/搜索和Poor再攻击冲突；53-65K备注N的发射IntI与运行P/S分离；完整Oscar快速重装/现代改装和附件服役年门。DualWire精确反馈另为TBD，本包不猜其精度/保鲜/刷新。

本方观察沿前包边界：原始鱼雷体不投影；未知存活不转成0或假active；发射账保留原始ContactId，不因物理Wake目标切换或秘密结果重写本方声明。当前测试40项旧鱼雷及11项Wake由根统一cargo；本文件待真实日志更新。
