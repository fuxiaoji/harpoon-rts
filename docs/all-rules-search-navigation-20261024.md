# 初始空投搜索、紧急上浮与共同控制接口：已验来源子集

全书与相关整章仍未闭合。这里验收的是下表列出的执行范围；撤掉无据电池默认值不是完成电池规则，螺旋能运动也不是完成捕获、机会目标或再攻。

## 来源和逐项结果

先查三中文核心RAG，再对真正2026英文140页原件逐页核版/亲视，SHA为8196ffc715bbcccd862d8597c607de7816dac5baee31fa659f89dfaf4323700c。导航候选及独立核页见artifacts/rules-navigation-remaining-20261024/{luna-source,peer}；搜索来源沿用已核PDF79/7-4，不使用已撤回2021候选。精确Source/Data/Execution/Tests与未决见[机制台账](search-navigation-all-mechanisms-20261024.json)。

| 机制 | 已实际执行的范围 | 保留的边界 |
|---|---|---|
| 有线裁判隐私 | 原source_launch质量/线态及周期wire_check原骰仅Referee；本方真实发射/弹库仍公开，2案例 | 完整DualWire报告未闭合 |
| H1初始空水域螺旋 | 真实Mk54投放/失败Placement后150yd圈、500yd圈钟、每圈两带、限深反射；原生Motion扫掠保证，11案例 | 可能接触/互扰保守pending，不伪造捕获或最近目标；首次上返Shallow以后的程序、完整R-054未完 |
| Emergency Blow | 真实速度按高一档上升，31+仍8带/Tac，连续三带最低承诺、重复不重置、到达与普通SetDepth真正换模式，11案例 | 零速、VeryDeep及全部损伤/操纵组合不据此声称完成；Blow没有CrashDive专门Annex门 |
| 被动Loud和遮蔽 | Blow基础Loud+6与独立成立的螺桨空化+4合并；扫描与同5°遮蔽共用资格，3真实动作案例 | 只有Blow自身空化的额外+4条件仍R-063待裁，不能默判总6或10 |
| 电池边界撤回 | v4不再从旧100满电/.05每秒消耗/.1充电/20半速造出续航；critical未知值不补100，LLM/UI保持未知，10 Rust+3 Node案例 | FullBatteryFit、当前charge、完整周期均速、充电、Snorkel、AIP未实现 |
| 玩家/AI/MCP/FSM/LLM | 同一只读来源预检先于入队/夺权，37外层71形状；FSM从本方操作识别Blow并保留现有意图，普通/维护/协同路径均不打断；显式玩家允许AI才放回控制 | 实际代码/协议与LLM投影测试不等于真实LLM对局或所有规则组合 |

## 明示RTS改编

径向75码入圈、右旋、500码/档位速度圈钟与150π码实际圆周的分账、物理带端点之间连续插值是明示执行选择，不是原书另给的公式。首次上返Shallow保留待裁，不猜下个循环。Blow逐秒消费当前速度档的源带数，最少三带前阻止普通改令，是持续动作的RTS承诺；FSM30秒复查是辅助策略，不改变源时钟。真实水深/舰体/传感器/管装配没有可靠来源时不替玩家填值。

## 实际联合验收

完整workspace为73目标1203通过/0失败/5既有忽略；相对上一冻结包新增50个独立测试名、没有删除旧名。Node70通过，应用语法及限定新增模块格式检查、两个仓库diff检查通过。数字是执行次数，不是规则覆盖率。201个Rust/Cargo/data/database输入在最终回归与实际协议前后SHA一致；web独立实际操作和最终Node复验。

最终协议目录artifacts/rules-navigation-remaining-20261024/http/acceptance-3，所有10份validation均ok；1/2轮失败保留：第一轮工具误把可省略depth_operation当必有键，第二轮H1工具用了不支持Hover的旧SetThrottle别名。工具分别改成optional读取、原生aviation Throttle Hover并核真实状态；三轮使用完全相同二进制，未为工具错误改裁决。

- HTTP原生本方声纳测量形成Good FCS，T544合法Plan/Fire，不注入接触/作者Good；至T579终局，220事件/2批，完整World本地/跨进程与重演hash1b766761254f5a8c。
- MCP实际stdio37/71，其中68只是传输入队，三个占位源动作被正确拒绝；合法Plan/Fire、真实扣弹和持续手控另验。不能将71种形状称71条规则执行完成。
- Blow实际20kt由300到50m用108秒；另有180秒浮航、完成三带后正常潜深替换，HTTP/MCP均执行。T41保存续至T108完整World本地/跨进程一致，20事件/2批，hash1cfce5c5ac8b153b。450m沿用现库上限；12节浮航/1000m水深等逐项scenario_assumption，不是真实全装配。
- H1 HTTP原生辅助舰声纳产生本方定位/分类CID，真实飞机原生Hover/Fire扣一雷；该支作者Good明确仅用于机制隔离，不证明原生飞机FCS。原Placement身份10.placement:5:0，T362真实D10=10对chance9失败，D6=[1,4]偏20°。T375约半圈保存，T402继续，185事件/2批，hash5ec9a8ff328b39c0，本地/跨进程完整World一致且原骰不改。MCP只证明真实释放/扣弹/手控/公开隐私，不单独证明隐藏螺旋轨迹。
- 两份旧v4（ASCM与Plan）均以源指纹不匹配拒读，拒读前后当前World完全相同。旧v3原档T299/7事件、续至T329/15事件分别hashab2ffcb9c57e6195/933df72aebd01cdd。历史Sol10800/360批/7505事件完整重演hash22b2238739248f90；原档及operations-v3原SHA不变。

server SHA e9bcf50b1a3e0368bfe7413a8e147e2dc8f58a3ca4d0aa6714213a1ea9256433；MCP SHA eab6b03b99388fb4efbe4a4714e7a46c080d195074377a894c543146db54cca3。协议与UI专用进程已退出，不动既有服务。

## 玩家界面和人工接管

真实浏览器点击/填值/时间控制确认：非法0m和最低承诺内改令拒绝，失败不夺控制；合法重复命令started_at保持1、work30→33秒；到达50m仍人工；另下浮航命令54秒时仍50m、180秒后0m；只有明确点击允许AI才解除人工。新增按钮旁显示本方实际目标/进度，中英切换、1600×1000与1024×768检查、未知电池和无fit隐藏入口通过。修复缓存版本及拒单文字可读性，证据ui/ui-validation.json、command-responses.json及final/readable-final截图。已有侧栏需滚动/展开；没有实体平板、整站或真实LLM比赛声明。

## 候选纠错与全书余项

导航Luna110电池数值格/44转向格匹配不代表整包正确：Blow/CrashDive资格混淆、旧10节正文表格冲突、AIP20节边界、碰撞原始DP等由独立原页复核逐条纠正，未消费错误字段。未来电池Annex只确认精确四ID的额定容量/技术、深度带和首速等离散字段，未录生产；candidate rule_context旧10节断言须撤回，整包不可直接使用。MaxDepth的IntIV/IntV不能把旁列200/250电池单位当米数，见R-064与原页审查。

完整鱼雷捕获/机会/重获/再攻与5000yd同型互扰、ASCM弹道/W3/多弹/EW/再攻及附件、硬舵/规避/碰撞/搁浅/完整电池AIP/CrashDive/VeryDeep、航空低空/云地形/续航加油与装配、TDL共享FCS/舰炮/深弹/BMD、损伤修复余项、机场港口功能和E00–E13可选规则仍须逐条Source/Data/Execution/Tests闭合。324目录和96表候选不能作为原子规则分母，也不给猜测完成百分比。未提交推送。
