# v4 防空裁判事件与本方开火提示

v4防空原始结算日志含敌目标修正、命中概率和D100；破片日志还含来袭导弹实际速度。它们原先发给某一玩家侧，使规则解释日志越过了双盲观察边界。现在仅将以下四种原始事件在v4下保留为`Referee`：

- `aaw2026 / 8.1.3.source_gun`
- `aaw2026 / 8.1.1.packet_terminal`
- `aaw2026 / 9.5.packet_terminal`
- `operations / 8.1.5.fragment_roll`

原始detail、概率、骰值和裁判日志仍完整保存；双方`observe`都不再收到这些事件。其他事件保持既有可见性，包含本方勤务和源发射回执；v1–3的原trace分发规则保留。

手动和自动枪炮共用的实际`fire_at`入口新增本方`8.1.3.gun_release`提示，detail严格只有`weapon`、`rounds`和`physical_mount`，unit为本方射手，contact为空。它不包含敌方实体ID、目标属性、命中结果、概率或骰值；不引入会被其他计账器误当武器释放的新EventKind。事件在原gun_roll、耗弹、原始trace、破片和退役调用完成后发出，原裁决调用顺序保持。

## 验收边界

新`aaw_observation_privacy_2026`包含六项：实际本方接触→手动CIWS、自动fire_control_scan→CIWS、明确作者MR的SAM双弹包实际终端、F/A-18E/AIM-120D双弹包实际终端、实际operations破片裁决，以及v3兼容对照。它们确认原始裁判detail仍在、双Side不可见、本方耗弹和合法发射提示可见。

SAM/AAM测试把已实际创建并登记的弹体放到明确终端几何后调用`advance_projectiles`，验证实际终端分支和日志投影；它们不宣称验证完整飞行时序。来袭防守夹具为明确作者目标，不冒充真实IC。v3有实际破片链；因v3没有v4包/枪炮执行器，另三项v3 trace调用仅是分发兼容控制，已在测试中注明。

根代理先正常编译运行新target，在`artifacts/rules-antiship-20261024/privacy-source-red.log`获得1绿、5个真实失败；无夹具编译失败冒充源红。生产窄修在该结果之后进行。旧AAW25中与本问题对应的断言同步更新为原始source_gun仅裁判可见、纯本方gun_release可见。

根代理随后在`artifacts/rules-antiship-20261024/privacy-joint.log`实际完成旧AAW25、新隐私6及反舰11项，共42个case执行通过、零失败。这里是这些定向target的执行数，不是整库、HTTP/MCP或完整飞行验收；后续联合验收由根代理完成。

独立只读复核确认枪炮及破片随机身份分别来自tick/actor/burst/count和每actor独立event_sequences；此处新增末尾提示不参与这些标签。子代理不运行Cargo，仅做格式和静态检查。本专题仅保护上述四条已核原始事件，不以定向测试声称全部观察路径已完成审计。
