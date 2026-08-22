# 引擎架构与公共边界

## 技术栈

- Python 3.12–3.13，`uv` 锁定依赖。
- Pydantic 负责 JSON 契约和不可变领域模型。
- pytest、Hypothesis、ruff、mypy 作为提交门禁。

## 分层

- `domain` 只定义状态和公共数据契约，不依赖规则、CLI 或控制器。
- `rules` 保存带稳定 `rule_id` 的规则解释和核验状态。
- `engine` 持有权威世界状态、阶段机、命令、记录式随机源和事件日志。
- `adapters` 负责场景/存档 JSON；`controllers` 只依赖 observation/action 契约。
- CLI 只调用 `GameEngine` 公共 facade，不接触内部规则对象。

## 控制器接口

人类、LLM、状态机和 RL 控制器统一使用：

```text
observe(side_id) -> Observation
list_actions(side_id, unit_id?) -> ActionOption[]
explain_action(action_id) -> RuleExplanation
submit(command) -> ValidationResult
lock(side_id) -> PhaseStatus
resolve() -> Event[]
```

`ActionOption` 同时返回可用与不可用行动。不可用项必须携带原因、规则标题、来源章节和核验状态，使 LLM 不需要解析规则 PDF，也不会把未实现能力误认为合法动作。

## 确定性与信息隔离

- 命令提交不改变世界状态；双方锁定后按稳定 command ID 顺序统一裁决。
- 随机结果由场景种子与 `random_draw_index` 生成并记录。
- 事件、在途武器 ID 不使用随机 UUID。
- 存档携带状态哈希、事件、待裁决命令、平台和武器定义。
- `Observation` 只含己方单位与己方接触，不暴露敌方权威单位状态。

## 严格规则门

已核验的射程、航速、DP 和损伤公式可执行。未核验的命中表不以自创概率替代：导弹抵达后生成 `weapon_arrival_requires_adjudication`，由裁判提交命中/未命中结果。航空、潜艇、对陆、电子战和水雷接口目前可发现但不可用。

