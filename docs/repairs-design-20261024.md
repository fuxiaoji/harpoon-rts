# 14.6战后修理窄范围设计，待根代理核实

状态：仅来源复核与设计，未注册测试、未落生产。按项目Skill已重新核心RAG检索“修理”（无结果）、“维修”“14.6”，并亲视repairs-source/source-pages/english-rules-pdf136.png。权威为2026-01-01英文PDF136/14-10，源转录在`artifacts/rules-closure-20261024/repairs-source/`；不修改该来源包。

## 拟先闭范围

仅显式v4、真实执行过工程critical且可逆效果贡献完整记录的水面舰工程损伤。第一包支持每个实际critical增加恰好一级25%降速的情形；潜艇/单机critical增加两级，以及任何缺ledger的旧伤不猜重建，保存pending。使用真实critical→实际失速→真实DamageControl→时间检查→原骰→恢复一档→实际移动链，不止概率函数。

原书检查为战斗结束后6/12/24/48小时；每critical独立D100，6/12小时为剩余DP/原DP的一半，24/48为完整比率，D100小于或等于Repair Roll成功。成功只修该critical带来的一档工程降速，不恢复舰体DP，不修别的critical，不补弹，不解除临时电力停机/火灾。失败48小时不能继续海上修理。工程critical附随火灾/电力继续原损管时钟。

## 玩家命令与时序

复用Command::DamageControl的四个字符串：

- `begin_repairs`：玩家明确宣告该舰战后修理起点，默认manual；不从静默、敌人真态、时间压缩或位置推断战斗结束。重复命令幂等，不刷新原起点/已失败检查/48小时封顶。
- `repairs_auto`：已有起点后自动在6/12/24/48小时处理到期检查；可在manual之间切换，不清原骰/起点。
- `repairs_manual`：保留检查钟和所有已到期快照，关闭自动投骰。
- `repair_due`：只投已到期且尚未投的检查；不能请求任意概率、骰面或提前检查。重复/保存恢复均沿用原结果。

逐秒step在到期点保存剩余DP/原DP/比率快照；manual仅延后“玩家投骰”，不让晚点执行改变原检查时点输入。manual错过多个检查时，按6→12→24→48顺序处理已积累的到期检查；一旦该critical成功，停止后续。它是明确的连续RTS人工调度策略，需要根代理确认。若切auto时已有到期检查，同一次控制动作可按原快照处理。

原生D100使用稳定`critical identity + checkpoint`独立随机身份（hash时刻用既定due或0，不能用用户实际按按钮的tick）；每critical/每checkpoint保存一次，模式切换、重复命令、save/restore不能重抽。

## 比率与舍入

拟直接按原D100整数面与原始比率比较：`d100/100 <= remainingDP/originalDP × (0.5或1)`。不把中间百分率另做四舍五入；原书37/100早期半值0.185，整数骰18成功、19失败，正好对应原例所写18%。这只是原D100≤比率的直接比较，不宣称原文存在通用向下取整。若根代理认为仍须保留舍入待裁，则收窄到整数百分率及早期可整除2的实际状态；其他输入保存pending，不能自定四舍五入。该决策在注册前核定。

## 可逆效果账与旧档

`Runtime.repairs: Option<repairs2026::State>`仅v4，进入指纹。每次工程effect成功执行后，以原critical identity登记tick、类型、实际增加的降速级数；并按同identity去重。只认这个前瞻账。`damage.engine_hits`是历史计数，不当作某条critical的身份；修理不回减它。`damage.engine_levels`当前总数必须与可追踪未修critical的贡献总数一致，才可自动恢复；出现旧伤/外部改写/未知基线保存pending。

多条完整、均发生在明确战后宣告之前的独立一级工程critical，可按各自原identity分别投骰、每成功一条减一级；累计贡献仍可验证。不把一个两级潜艇critical擅自拆成两条。战后宣告之后再次受同系统critical，源“compute new roll and start over”重置时序未明：保已有骰/检查/新伤账并pending，不自动重启或续借旧检查钟，不允许再次begin绕过。若要首包进一步只支持一条critical，也可收窄，不能猜多hit计时。

## 模块API与根代理共享钩子

新模块我方提供：

```rust
pub fn tables() -> &'static serde_json::Value;
pub fn record_engineering(w: &mut World, unit: EntityId, identity: &str, levels: u8);
pub fn control(w: &mut World, unit: EntityId, action: &str) -> Option<Result<(), String>>;
pub fn step(w: &mut World);
pub fn own_view(w: &World, unit: EntityId) -> Option<serde_json::Value>;
```

根代理提供/接入：

1. `damage_runtime::Runtime.repairs: Option<repairs2026::State>`、v4初始化/缺字段行为/指纹，rules/mod.rs注册；旧v2/v3/Legacy不启用且不新增输出字段。
2. `effect("engineering")`真实增加engine_levels后调用`repairs2026::record_engineering(w,t.id,identity,n as u8)`；使用短化后的实际critical身份，不用普通Impact或全局事件环重建。重复原critical不重复登记。
3. 窄恢复函数`damage_runtime::repair_engineering_level(w: &mut World, unit: EntityId, critical_identity: &str) -> Result<(), String>`：仅v4、已核存活水面舰、完整源修理账允许的一档恢复，减damage.engine_levels恰好1；不回减历史engine_hits、不改HP、不清电力/火水/其他系统。模块只有通过源D100的critical才调用，根也可在hook二次核查其check身份。若根希望我方模块直接修改该唯一字段，可以明确批准，避免跨域编辑damage_runtime。
4. `configure_control`先委托新模块control识别上述四动作，已有三种损管动作继续原义；规则step每tick调用repairs::step；本方UnitObs/agent字段按已有损伤视图挂own_view，敌方不获得真实伤态/计时/骰。原骰仅本方受损舰或Referee，不能给攻击方。

## 真实红测试计划（核实后注册）

1. 真实工程critical产生降速；300秒静默不修、未begin到6小时仍不修。
2. `begin_repairs`明确起点；6小时前1秒不投，6小时边界auto原D100成功恢复一档，实际移动消费新max，HP/其他critical/火电保留。
3. 原D100在50%/25%等来源精确边界成功/下一面失败，12与24小时倍率不同；失败骰保存不重抽。
4. manual到期仅保存检查输入而不投；repair_due真实下令才处理，切auto/manual、重复begin/due不刷新钟或骰。
5. 48小时全部失败后海上封顶，重复begin/模式切换/更晚时间都不再骰。
6. 完整World中途保存/恢复的工程伤账、检查、模式、原骰及后续移动完全相同；Red看不到Blue私有账。
7. 缺真实critical账的engine_levels>0旧伤不得补猜；两级潜艇/单机、再次系统受损/未核修理对象保存pending。

测试通过公共源critical执行或实际Astern缠桨造成伤态，再走真实sim DamageControl恢复。长小时边界可在已保存真实伤态/钟的夹具上设置待测tick为due−1，再执行真实sim::tick验证门，不声称逐秒模拟了48小时或HTTP实测。用原生seed找边界，不改概率/硬塞骰。没有用户指定时长的战局结束会中止tick，夹具须明确duration覆盖48小时。

## 明确保留

14.6完整桥/甲板/舵/传感器/武器架逐部位恢复尚不由此包覆盖；火复燃D6−2、补片/抽水/弹开、潜艇潜深约束、货舱/结构进港维修、电池/多级工程和未明再击时钟、14.7可选完好率全部分开保留。不得给普通战损套14.7固定30/35/40/45%或港口加成，也不能因窄工程修理绿关闭14.6。
