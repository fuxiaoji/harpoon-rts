# 游戏内 AI 参谋与双 Agent 对局

更新时间：2026-10-19（Asia/Shanghai）。实现位于 `crates/harpoon-server/src/agent.rs`，前端在 `web/command.html`、`web/app.js`、`web/i18n.js`、`web/theater.js`、`web/style.css`。

## 1. 功能范围

游戏内 AI 参谋（下称“参谋”）是一个**受限玩家视图**的 LLM 指挥助手：

- 每个回合只调用 `harpoon_core::observe::observe(world, side, since, max_events)`，看到与人类玩家相同的己方单位、己方接触估计、己方可见事件和公开想定/数据库信息；
- 不读取 `World` 的敌方真态，不读取裁判事件，不读取未探测接触的真值；
- 可生成全部 17 种 `Command`；服务端按己方单位、接触、武器、弹药、甲板机型和任务类型二次校验后，才送入与人类/MCP 相同的权威命令 inbox；
- 支持 `advise`（只给建议）与 `execute`（合法命令直接执行）两种自主度；
- 每个模拟 `cadence_s`（默认 300 秒 = 5 分钟）主动汇报，并保存跨回合长期记忆；
- 支持玩家对话、立即汇报、执行上次建议、清除记忆；
- 支持独立的双 Agent 六小时完整对局，供验证/演示，不干扰当前玩家战局。

本功能不是“让 LLM 直接改世界”。权威裁决、事件、命中、损伤、胜负仍完全由 Rust 引擎承担；LLM 只产生意图和命令。

## 2. 玩家视图与信息边界

### 2.1 观察

每回合的上下文由以下公开/己方材料组成：

- 当前 `Observation`：己方单位、接触估计（含质量、陈旧、最后更新、估计航向/航速）、己方可见事件、任务/胜利状态、天气；
- 想定公开信息：ID、名称、描述、时长、胜利条件、任务区（如果有）、战区元数据；
- 数据库公开卡：仅本方单位涉及的平台/武器/传感器，以及**已识别接触**对应的平台卡；未识别接触不会按型号猜测；
- 长期记忆摘要、最近 4 份参谋报告；
- MCP 工具名和命令 schema 说明。

敌方真实位置、真实损伤、未确认战果不会进入任何 prompt。终局前参谋只能看到 `own_confirmed_kills` 和 `own_losses`；`VictoryResult` 的 `blue_kills`/`red_kills` 在终局才写入观察。

### 2.2 命令校验与执行

`validate_command` 逐项检查：

- 单位属于本方且仍在 `Observation` 中；
- 接触在本方接触表中；
- 武器存在于该单位挂载、弹药足够、齐射数合法；
- 飞行甲板存在、机型有 ready/cycling 记录、任务坐标/接触合法；
- 深度、高度、航速、航路点数量等数值范围；
- 跟随的领舰属于本方且不是自己；
- 箔条余量等消耗条件。

命令通过后进入 `Game.inbox`，在下一个模拟 tick 由 `sim::tick` 统一处理。LLM 输出中的非法/幻觉 id 会被记录为 `rejected`，不会到达引擎。命令形状的兼容变体（数组坐标、`recon` 的 `station` 别名、缺省 `rounds` 等）会在反序列化前归一化，但不放宽所有权、射程或弹药校验。

## 3. 记忆与 5 分钟汇报

### 3.1 记忆

每个阵营有独立的 `Memory`：

- `summary`：模型返回的长期记忆摘要（最多 2400 字符）；
- `turns`：最近 16 条 `{tick, report, intent, commands}`；
- `last_event_seq`：下一次观察的事件游标，避免重复灌入历史事件。

记忆在每个成功回合后写入 `tmp/agents/<scenario>_<seed>_<side>.json`（git 忽略），随新游戏加载；`POST /api/agent/reset` 可清空，`purge_memory=true` 同时删除文件。记忆不包含 API 密钥。

### 3.2 汇报节奏

- 默认定时器是模拟时间：每 300 个 tick 触发一次；
- `TurnReport.tick` 记录**触发 tick**，不是 LLM 返回时的游戏 tick；慢请求不会把 05:00 的简报标成 12:42；
- 暂停时不会自动汇报；“立即汇报”按钮可随时发起一次手动回合；
- 若上一回合仍在请求中，定时器不会叠加并发；下一次可用 tick 再触发，因此高倍速 + 慢模型时汇报可能延迟。建议开启参谋时使用 30× 或更低倍速；
- 对话、手动汇报不改变下一次定时汇报的节拍。

### 3.3 输出契约

模型必须返回一个 JSON 对象：

```json
{
  "intent": "一句话意图",
  "report": "给指挥官的简报",
  "memory": "更新后的长期记忆",
  "commands": [],
  "error": null
}
```

报告控制在约 260 个中文字符/180 个英文词以内，命令最多 8 条。解析器兼容 Markdown 代码围栏、前后说明文字、尾逗号和常见的坐标/任务形状变体；`finish_reason=length` 会作为明确错误返回，而不是把截断内容当命令执行。

## 4. 模型配置

参谋使用 OpenAI 兼容的 `POST {base_url}/chat/completions`。默认值：

| 环境变量 | 默认 | 说明 |
| --- | --- | --- |
| `HARPOON_AGENT_BASE_URL` | `https://api.deepseek.com` | 也可回退到 `HARPOON_LLM_BASE_URL` |
| `HARPOON_AGENT_API_KEY` | 空 | 也可回退到 `HARPOON_LLM_API_KEY`；不会写入记忆文件 |
| `HARPOON_AGENT_MODEL` | `deepseek-flash` | 也可回退到 `HARPOON_LLM_MODEL` |
| `HARPOON_AGENT_CADENCE_S` | `300` | 60–3600 秒 |
| `HARPOON_AGENT_LANGUAGE` | `zh-CN` | `zh-CN` 或 `en` |
| `HARPOON_AGENT_MAX_TOKENS` | `65536` | 输出预算；见下 |

DeepSeek 会按 `max_tokens` 做余额预授权。一次六小时长局中，从 T+00:50 起每侧出现 44 次（合计 88 次）HTTP 402 `Insufficient Balance`；长局结束后用同一 key 重测 128k/64k 均返回 200，说明这是账户余额/预授权状态问题，而不是本地崩溃或解析逻辑。默认 65536 是保守预算，实际参谋回合完成量约 1.5k–7.7k token，正常不会截断；余额充足时可调高，但请以 `/api/agent/status` 的用量和账户余额为准，不要把 128k 当作必然可用。

也可以在运行中通过 `POST /api/agent/config` 修改模型、key、语言、节拍与预算；API key 只保存在进程内存，不回传、不落盘。

## 5. HTTP API

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/api/agent/status` | 全局状态、两侧状态、用量统计、双 Agent 任务 |
| GET | `/api/agent/:side/status` | 单侧状态 |
| POST | `/api/agent/config` | `{side?, enabled?, autonomy?, cadence_s?, max_tokens?, model?, base_url?, api_key?, language?}` |
| POST | `/api/agent/:side/report` | 立即生成一份汇报；同步等待 LLM |
| POST | `/api/agent/:side/chat` | `{message}`；回答玩家并可选产生命令 |
| POST | `/api/agent/:side/execute` | 执行最近一次建议（advise 模式） |
| POST | `/api/agent/reset` | `{purge_memory?}` |
| POST | `/api/agent/duel` | `{scenario, seed?, max_ticks?, cadence_s?}` 启动独立双 Agent 对局 |
| GET | `/api/agent/duel` | 最近一次双 Agent 任务进度/结果 |

`/api/agent/status` 的 `sides.<Side>.usage` 包含 `turns`、累计 prompt/completion/cache token 和每回合平均值。`reports` 返回最近简报，`last_proposals` 返回最近命令建议。

聊天响应包含 `reply`、`commands`、`rejected`、`applied`、`tick`、`model`、`latency_ms` 和完整 `status`。`execute` 只执行仍能通过当前观察校验的建议。

## 6. 前端面板

`command.html` 右侧栏顶部加入“AI 参谋 / AGENT”：

- 状态：待机 / 在线 / 分析中 / 错误 / 未配置密钥；
- 模型与下一次汇报倒计时；
- 启用/停用、允许下令、立即汇报、执行建议；
- 最近 4 份简报（定时/手动/对话、已执行/建议、命令数、拒绝数、completion token）；
- 可展开的长期记忆；
- 对话记录与输入框（Enter 或发送按钮提交）；
- 中英切换时静态标签、动态标签、空态文案同步；模型生成内容按配置语言生成。

面板每 3 秒轮询 `/api/agent/status`，但只有内容变化时才重绘报告/聊天 DOM，避免无谓布局抖动和输入焦点丢失。

## 7. 六小时击杀数想定

`scenarios/dual_carrier_kills.json` 从 `dual_carrier_hotseat.json` 派生，保留四航母、34 个初始单位、航空联队和卫星战区；移除 `area_denial`，设置：

```json
"duration_s": 21600,
"victory": {"target_vp": 0, "kill_count_at_end": true}
```

`VictoryConfig.kill_count_at_end` 在到期时比较**击毁敌方平台数**：

- 舰艇、潜艇、飞机、基地各计 1；
- 导弹/鱼雷不计；
- 飞机燃油迫降不计为敌方击杀；
- 双方相等判平局。

引擎在 `damage::sink` 中维护 `World.losses`（权威损失）和 `SideState.confirmed_kills`（本方通过接触确认的击杀）；计数不依赖仍在场景中的尸体实体，因此 `cleanup_dead` 后仍然有效。`VictoryResult` 增加 `blue_kills`/`red_kills`；`Observation.victory` 只暴露本方确认击杀、本方损失、模式、剩余时间和公开阈值。

HUD 在击杀数模式下显示“确认击杀 X · 损失 Y”、剩余时间和六小时决胜规则；任务简报说明计数口径、AI 面板和首轮操作。区域拒止想定仍使用原有 `mission` 路径，不受影响。

## 8. 双 Agent 完整对局

`POST /api/agent/duel` 在独立 `World` 中运行：

1. 加载想定，双方各持独立 `Memory`；
2. 每 300 秒（可配置）分别调用一次两个参谋，观察分别是 `observe(world, Blue)` 与 `observe(world, Red)`；
3. 两侧解析/校验命令后，在同一段 `sim::tick` 中提交；
4. 推进 300 tick，重复至想定结束或 `max_ticks`；
5. 结束时写 `tmp/agent-duels/duel-<id>-<scenario>.json`，包含双方完整报告、事件计数、命令数、用量统计、终局结果、击毁数、VP、状态摘要和说明。

任务状态通过 `GET /api/agent/duel` 查看；它与当前玩家战局完全独立，不会替换或暂停玩家对局。

## 9. 验证与实测

### 9.1 测试

- `cargo test -p harpoon-core`：107 项通过（97 项库测试 + 7 项 area-denial + 3 项新增 `kill_count`；原有武器/规则测试未回退）。
- `cargo test -p harpoon-server --bin harpoon-server`：20 项通过（新增 6 项 Agent 测试：命令形状归一化、尾逗号、探测/反潜任务别名、5 分钟节拍与触发 tick、用量平均、飞机 700 节不被误拒）。
- `node --test tests/*.test.cjs`：35 项通过。

### 9.2 DeepSeek Flash 单回合用量

在暂停的 `dual_carrier_kills` 开局连续手动汇报 3 次（同 seed 20261019），DeepSeek Flash `deepseek-flash`：

| 指标 | 平均值/结果 |
| --- | --- |
| prompt tokens / 回合 | 24,419 |
| completion tokens / 回合 | 1,494 |
| total tokens / 回合 | 25,913 |
| 第 2/3 次 cache hit | 23,552 / 23,680 |
| 延迟 | 7.5–9.2 秒 |

首轮 6 条命令、无拒绝；后续轮次稳定返回 6 条命令。军情交战后 prompt 会增至约 35k–55k、completion 约 4k–8k、延迟约 20–40 秒，属于接触/事件增多后的正常增长。

### 9.3 双 Agent 六小时对局（已完成局）

2026-10-19 使用 `dual_carrier_kills`、seed `20261019`、`cadence_s=300`、`max_ticks=21600` 完成一局：

- 双方各 72 份报告，tick 覆盖 0, 300, …, 21,300；无缺失、无重复。
- 终局 T+06:00:00：`blue_kills=46`、`red_kills=25`、`winner=Blue`、`reason=time limit: kill count`；双方 VP 为 0（击杀数模式不使用 VP）。
- 双方各 27 个成功决策回合，共 200（Blue）/211（Red）条命令；其余每侧 45 个错误回合：44 次 HTTP 402 `Insufficient Balance` + 1 次 invalid JSON。
- 命令拒绝：Blue 5 条、Red 1 条。其中 2 条 Blue `fire` 因 YJ-83K 弹药耗尽被正确拒绝；2 条 Blue 飞机 `set_speed=700` 被旧校验器错误拒绝（已修复为 0–2000 节并加回归测试）；Blue/Red 各 1 条 invalid JSON（已加单次重试）。
- 服务器日志无 panic/崩溃，仅启动与数据库卷加载；`event_counts` 取自引擎 4096 条事件上限的尾部，不是全量统计，`kills`/`result` 才是权威。
- 产物：`E:\CodexBuilds\harpoon-agent-20261019\preauth\preauth-duel-1.json`，214,443 bytes，SHA-256 `8ada9a1ad8d03599ae063f32b3f9196d81dd27e5f599de19d7ba0f8e653c44bf`。

该局后段因账户 402 没有持续产生新决策，但仍覆盖全部 72 个 5 分钟节点并由引擎正常完成六小时裁决。按用户要求未重跑整局；64k 预算、任务形状归一化、飞机速度校验和 JSON 重试已在源码与单测中落实。

### 9.4 浏览器验收

在独立 8799 服务、1600×900 视口实际验证：

- 击杀数 HUD 显示“任务剩余 06:00:00 / 确认击杀 0 · 损失 0”及决胜规则；
- AI 面板显示 `deepseek-flash · 5 分`、待机/就绪、启用按钮；
- 启用后状态变为在线，出现“下次汇报”和“允许下令”；
- 手动汇报返回并渲染中文简报、命令数、token 数和记忆；
- 对话输入 + 发送按钮返回中文回答与 6 条建议；面板状态在对话后保持完整；
- 切换 English 后静态/动态标签变为英文，击杀数简报变为英文；切回中文正常；
- 点击“执行建议”接口返回 `{"queued":4,"rejected":[]}`。

## 10. 边界与后续

- 参谋是战术顾问，不是规则裁判；报告可能误判，命令合法不等于战术正确。
- 不提供外部 MCP 客户端能力；提示中的 MCP 工具清单用于让模型理解同一权威接口和命令 schema。
- 记忆按“想定 + seed + 阵营”保存，不是跨对局的战略记忆；换 seed 会建立新记忆。
- 高倍速 + 慢模型会延迟 5 分钟节拍；需要严格 5 分钟节拍时使用 30× 或更低倍速，并观察“下次汇报”倒计时。
- DeepSeek 的 `max_tokens` 预授权会消耗余额快照；生产配置不要使用 128k。实际用量以 `/api/agent/status` 的 `usage` 为准。
- 尚未实现：多参谋并行投票、外部知识库检索、浏览器端 API key 设置页、跨局长期战略记忆、语音播报。
