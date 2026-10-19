# 游戏内 AI 参谋与双 Agent 对局

更新时间：2026-10-19（Asia/Shanghai）。实现位于 `crates/harpoon-server/src/agent.rs`，前端在 `web/command.html`、`web/app.js`、`web/i18n.js`、`web/theater.js`、`web/style.css`。

## 1. 功能范围

游戏内 AI 参谋（下称“参谋”）是一个**受限玩家视图**的 LLM 指挥助手：

- 每个回合只调用 `harpoon_core::observe::observe(world, side, since, max_events)`，看到与人类玩家相同的己方单位、己方接触估计、己方可见事件和公开想定/数据库信息；
- 不读取 `World` 的敌方真态，不读取裁判事件，不读取未探测接触的真值；
- 可生成全部 17 种 `Command`；服务端按己方单位、接触、武器、弹药、甲板机型和任务类型二次校验后，才送入与人类/MCP 相同的权威命令 inbox；
- 支持 `advise`（只给建议）与 `execute`（合法命令直接执行）两种自主度；
- 每个模拟 `cadence_s`（默认 300 秒 = 5 分钟）主动汇报，并保存跨回合长期记忆；
- 支持服务端自动推进与速度选择：`auto_run` / `auto_speed`，浏览器关闭后仍按所选速度运行；参谋每回合做战术决策，时间速度由玩家在面板选择（模型不直接改全局时间，避免乱暂停/狂飙）；
- 支持玩家对话、立即汇报、执行上次建议、清除记忆；
- 战场反馈：地图 HUD 常驻“战果/损失”，新击杀/损失/重损弹出战斗结果卡，终局弹出结算对话框；
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

报告控制在约 260 个中文字符/180 个英文词以内，命令最多 16 条；潜艇进入鱼雷射程时应优先齐射 2–4 枚，单枚鱼雷通常会被航母诱饵骗走。解析器兼容 Markdown 代码围栏、前后说明文字、尾逗号和常见的坐标/任务形状变体；`finish_reason=length` 会作为明确错误返回，而不是把截断内容当命令执行。

### 3.4 自动推进与速度

**严格双盲固定规划窗（大厅 AI 对局，推荐）**

- 大厅新任务表单提供：是否与 AI 参谋对战（红方 Agent）、我的助手（关闭/半自动/全自动）、初始规划窗速度。
- 每 300 模拟秒是一个**公共**规划窗；双方各自调用 `observe(world, side)`，私有事件（新接触、来袭、损失）不会暂停世界，因此不会通过暂停泄漏情报。
- 只有**执行型（execute）**助手参与规划暂停：双方执行型都在时等双方完成；纯建议（advise）助手只后台汇报，不阻塞时钟。规划完成后恢复速度，命令在下一 tick 生效。
- 半自动：助手只给报告/建议，命令仍由玩家下达；AI 对手的 execute 命令自动执行。
- 全自动：双方 execute 各自返回 `desired_speed`（5–60），全局推进取**较慢值**；没有玩家操作时也可推进。
- 规划窗暂停是公开、固定节拍的，不是私有事件触发；这是严格双盲模式。建议 5 分钟局用 15×；较长战役可用 30×/60×，但单回合 LLM 延迟会拉长每个规划窗。
- 大厅配置会落到 `/api/agent/config`；完整对局仍走 `/api/game` + 普通权威引擎，不经过私有唤醒导演器。

**单方自动推进**

- `auto_run` 与 `auto_speed` 是每侧配置；启用后，服务端在游戏暂停且未结束时把速度设为该值，因此不需要浏览器保持打开。
- `auto_speed` 允许 1–60×；全自动双方会用模型返回的 `desired_speed` 取 min。
- 玩家点顶部暂停会关闭自动推进并停在 0×。

**私有唤醒导演（实验 API，不用于大厅严格模式）**

- 每轮模型返回 `desired_speed`（5–60）和下一组 `wake_conditions`；全局速度 = 双方请求的**最小值**，并保留 5× 下限。
- 任一方的私有唤醒条件成立时，世界立即停在当前 tick；双方同时规划，命令在下一 tick 同时生效。
- 为防止“规划风暴”，任意两次唤醒之间至少间隔 **60 模拟秒**；时间唤醒本身下限 120 秒；单次规划超时 90 秒。
- 导演模式由服务端推进，浏览器只观察；速度按钮和单方 Agent 控制在导演局中禁用。
- 私有唤醒会暂停双方，因此“出现暂停”本身可能向对手泄露存在触发事件；这是操作者选择的快速对抗模式（模式 2），不是严格双盲评测模式。

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
| POST | `/api/agent/config` | `{side?, enabled?, autonomy?, cadence_s?, max_tokens?, auto_run?, auto_speed?, model?, base_url?, api_key?, language?}` |
| POST | `/api/agent/:side/report` | 立即生成一份汇报；同步等待 LLM |
| POST | `/api/agent/:side/chat` | `{message}`；回答玩家并可选产生命令 |
| POST | `/api/agent/:side/execute` | 执行最近一次建议（advise 模式） |
| POST | `/api/agent/reset` | `{purge_memory?}` |
| POST | `/api/agent/duel` | `{scenario, seed?, max_ticks?, cadence_s?}` 启动独立双 Agent 对局 |
| GET | `/api/agent/duel` | 最近一次双 Agent 任务进度/结果 |
| GET | `/api/director` | 导演局状态：阶段、tick、触发方、双方速度、唤醒规则 |
| POST | `/api/director/start` | `{scenario, seed?, max_ticks?}` 启动双方导演局（会替换当前战局） |
| POST | `/api/director/stop` | 停止导演推进 |

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

## 6.1 战果与损失反馈

- 地图左上“战果/损失”常驻计分来自 `Observation.victory.own_confirmed_kills` / `own_losses`，不读取敌方真态；终局权威结果仍以 `VictoryResult` 为准。
- 新事件会弹出顶部战斗结果卡：
  - `contact_sank` → “确认战果”；
  - 本方 `sunk`/`air_shot_down`/`ditched`/`base_destroyed` → “己方损失”；
  - 本方 `critical_hit` 或显著 `impact`（伤害 ≥25 DP 或剩余 HP 低于 50%）→ “损伤告警”。
- `impact` 弹窗只接受在 `Observation.units` 中找到的本方单位；如果事件 `side` 与单位归属不一致，宁可不在 UI 弹窗，也不把敌方飞机/舰艇误报成己方损失。
- 终局时弹出结算对话框，显示胜负、原因、本方战果/损失、双方权威击杀和 VP；可关闭，关闭后本局不再重复弹出。
- 事件日志仍保留完整原始字段，便于对“带 Red 名称却对 Blue 可见”的事件做引擎归属审计。

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

HUD 在击杀数模式下显示“确认击杀 X · 损失 Y”、剩余时间和按剧本时长生成的决胜规则（不再写死六小时）；任务简报说明计数口径、AI 面板和首轮操作。区域拒止想定仍使用原有 `mission` 路径，不受影响。

`scenarios/dual_carrier_duel.json` 是基于同一兵力的 **5 分钟纯杀伤对抗局**：无区域任务，`duration_s=300`、`kill_count_at_end=true`，专用于导演局演示和录像。

`scenarios/fleet_decisive_3h.json` 是 **3 小时舰队决战**：同一四航母/护航/航空/潜艇兵力，无区域任务，`duration_s=10800`、`kill_count_at_end=true`。到期按击毁平台数决胜；大厅可直接选择 AI 对手和半自动/全自动助手。

## 8. 双 Agent 完整对局

`POST /api/agent/duel` 在独立 `World` 中运行：

1. 加载想定，双方各持独立 `Memory`；
2. 每 300 秒（可配置）分别调用一次两个参谋，观察分别是 `observe(world, Blue)` 与 `observe(world, Red)`；
3. 两侧解析/校验命令后，在同一段 `sim::tick` 中提交；
4. 推进 300 tick，重复至想定结束或 `max_ticks`；
5. 结束时写 `tmp/agent-duels/duel-<id>-<scenario>.json`，包含双方完整报告、事件计数、命令数、用量统计、终局结果、击毁数、VP、状态摘要和说明。

任务状态通过 `GET /api/agent/duel` 查看；它与当前玩家战局完全独立，不会替换或暂停玩家对局。

### 8.1 实时导演器（私有唤醒）

`/api/director/start` 替换当前战局并启用双方 execute：

1. 初始在 tick 0 双方规划；
2. 每方维护白名单唤醒规则：`time`、`new_contact`、`incoming`、`own_loss`、`own_damage`、`contact_range`；
3. 每 tick 只按该方 observation 求值；任一规则触发即暂停世界；
4. 双方并行调用 LLM，返回命令、`desired_speed` 和新唤醒规则；
5. 命令在 T+1 同时提交，恢复速度为双方请求的 min；
6. 终局写回导演状态并在 UI 弹出结算。

导演器状态同时显示阶段（planning/running/over）、触发方/规则、双方速度和当前 tick；前端顶部有 `DIRECTOR` 横幅。地图可用 `HarpoonUI.setMapView(cx,cy,scale)` 精确定位特写。

大厅的“AI 对手/助手”默认**不使用**这条私有唤醒导演路径，而是使用上面的严格双盲固定 300 秒规划窗；`/api/director/*` 保留给实验/演示。

## 9. 验证与实测

### 9.1 测试

- `cargo test -p harpoon-core`：107 项通过（97 项库测试 + 7 项 area-denial + 3 项新增 `kill_count`；原有武器/规则测试未回退）。
- `cargo test -p harpoon-server --bin harpoon-server`：22 项通过（新增 6 项 Agent 测试 + 2 项导演器唤醒规则测试：命令形状归一化、尾逗号、探测/反潜任务别名、5 分钟节拍与触发 tick、用量平均、飞机 700 节不被误拒、唤醒规则 JSON、类别别名/时间兜底）。
- `node --test tests/*.test.cjs`：37 项通过。

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

### 9.4 8795 自动推进实机（2026-10-19）

在 8795 的 `dual_carrier_denial`、seed 20261019 上，从 tick 0 启用蓝方 Agent（execute）并打开 `auto_run`、`auto_speed=15`，由服务端独立推进：

- 服务端从暂停自动切到 15×，Agent 按 300 模拟秒周期汇报并下令，未手工推进时间。
- T+01:20:58（tick 4858）想定结束：红方 `mission_zone_held` 获胜；蓝方本方战果 11、本方损失 9；权威击杀 Blue 11 / Red 9。
- 页面 HUD 实际显示“战果 11 / 损失 9”；终局结算对话框实际弹出，显示“敌方胜利 / 美方航母连续占据任务区满3分钟”、本方战果/损失、权威击杀和 VP。
- 自动推进在终局停止；Agent 面板仍显示自动 15×，但游戏 `over` 后不再推进。下一次新开想定会沿用 `auto_run` 配置，除非先取消勾选或点暂停。
- 战果/损失/受损弹窗渲染通过同一真实 DOM 预览验证；实际比分变化和终局弹窗均已实机验证。

### 9.5 浏览器验收

在独立 8799 服务、1600×900 视口实际验证：

- 击杀数 HUD 显示“任务剩余 06:00:00 / 确认击杀 0 · 损失 0”及决胜规则；
- AI 面板显示 `deepseek-flash · 5 分`、待机/就绪、启用按钮；
- 启用后状态变为在线，出现“下次汇报”和“允许下令”；
- 手动汇报返回并渲染中文简报、命令数、token 数和记忆；
- 对话输入 + 发送按钮返回中文回答与 6 条建议；面板状态在对话后保持完整；
- 切换 English 后静态/动态标签变为英文，击杀数简报变为英文；切回中文正常；
- 点击“执行建议”接口返回 `{"queued":4,"rejected":[]}`；
- 自动推进开启后服务端速度从 0 变为 15×，面板显示“自动 15×”；游戏在浏览器关闭后仍可继续推进；
- 战果/损失计分显示 11/9，战果/损失/受损三种弹窗均实际渲染；终局结算弹窗 DOM 内容包含战报原因、双方击杀与 VP。

### 9.6 5 分钟双 Agent 导演局（2026-10-19）

- 想定：`dual_carrier_duel`，seed 20261019，`max_ticks=300`。
- 私有唤醒、双方暂停规划、min 速度、60 秒唤醒间隔、120 秒时间唤醒均实际执行。
- 终局 T+00:05:00：蓝方 9 杀、红方 7 杀，`time limit: kill count`，蓝方胜；VP Blue 128 / Red 90。
- 视频：`E:\CodexBuilds\harpoon-agent-20261019\delivery	win-shields-5min-agent-duel-realtime.mp4`，5:08，1600×900 H.264，SHA-256 `4ebd3fef13f6c815406988c1627c2cb7845c023ff863f75d86b0e9b9bafb3c5f`。
- 说明：IAB 自带录制在本机只产出 0.3 秒文件，因此视频是**逐帧截图约 0.33 fps + ffmpeg 25 fps 封装**的完整记录，不是 25 fps 连续录屏；帧内包含导演暂停、双方速度、战果条、AI 简报和终局结算。
- 同局暴露并修复了“10 分钟剧本仍显示六小时规则”的硬编码文本；现在 HUD、简报、Agent prompt 均按 `duration_s` 生成。

### 9.7 严格双盲模式与 3 小时决战烟测（2026-10-19）

- 大厅实际渲染了 AI 对手、助手半自动/全自动、规划窗速度选项；从大厅启动全自动 AI 对局后，`/api/time` 速度为 15×，双方 `auto_run=true`，`/api/director` 为 idle，没有私有暂停。
- 半自动 API 测试：红方 execute 的命令落地（applied=true），蓝方 advise 只产生建议（applied=false），速度保持 30×。
- 全自动 5 分钟局（15×）：固定 tick 1 规划窗暂停，双方各 8 条命令，applied=true；终局 4/4，draw（VP 32/36），证明固定规划窗能让模型在高速局内真正下命令。
- 3 小时决战 `fleet_decisive_3h` 烟测：初始规划窗暂停，双方 desired_speed=15，随后 15× 推进；tick 304 时蓝方 6 杀/0 损。随后切到半自动混合测试并推进到 tick 1279；红方 execute applied=true、蓝方 advise applied=false。未跑完 3 小时全局，不宣称完整验收。
- Agent 打磨：系统提示加入舰队决战原则；接触列表按非陈旧/质量/最后更新排序并截断 60 条；事件 40 条、历史 3 条；默认 LLM 超时 60 秒；只有 execute 侧参与规划暂停，advise 不阻塞。

### 9.9 加权击杀分、弹药纪律与页面冻结修复（2026-10-19）

**加权击杀分与按类别记录**

- `victory.kill_weights`（想定配置）：`ship`/`sub`/`aircraft`/`base` 各自分值。到期按加权分裁决，`result.scored=true`，`result.blue_points/red_points` 为加权分，`blue_kills_by_kind/red_kills_by_kind` 为按类别击杀数；未配置时全部为 1，行为不变。
- `VictoryObservation` 新增 `own_kills_by_kind`、`own_losses_by_kind`、`kill_weights`、`own_kill_points`；HUD 计分条显示括号细分（舰/潜/机），简报和结算对话框显示分值与细分。
- 两个航母想定（`carrier_duel_3h`、`dual_carrier_kills`）配置为舰 10 / 潜 5 / 机 1 / 基地 10，并把 AEW 初始阵位移到舰队后方、CAP 前出。
- 测试：`kill_weights.rs` 3 项（加权裁决与细分、无权重回退、观察字段含 `own_in_flight`）。

**弹药纪律与在飞弹药**

- `ContactObs.own_in_flight`：本方射向该接触、尚未落地的弹药数（只算本方，双盲不破）。目标卡新增"本方已在飞 X 枚"行。
- Agent 提示新增弹药纪律：一个接触每规划窗不超过约 4 枚在飞反舰弹；饱和后先停火，用 BDA 判伤再决定补射；潜艇鱼雷和飞机弹药要留储备。

**预警机保护、反潜与潜艇主动攻击（提示条令）**

- AEW 保持在本方 CAP 线后 25–40 海里；敌战斗机逼近 30 海里内即召回或引导 CAP 拦截。想定 AEW 初始位已后置。
- 有潜艇接触时：放飞 ASW 直升机（asw 任务）、引导反潜舰艇、近距 ASW 火箭开火。
- 己方潜艇：向敌最后已知位置机动，进入鱼雷射程对被跟踪舰船齐射 2–4 枚，不得消极跟踪。
- 明确告知模型 `victory.kill_weights`：只打飞机会输掉加权战，要打舰体。

**页面"卡住"根因与修复**

- 根因：服务端 WS 推送任务在 broadcast 通道积压（`RecvError::Lagged`，通道容量 64）时直接退出——TCP 连接保持打开但不再推帧；前端在 WS 打开时停用轮询，页面从此冻结且仍显示"实时连接"。两个浏览器标签页分别冻结在 tick 3532/5279，均为战斗密集期。
- 修复：服务端 `Lagged` 改为 continue（跳过错过的唤醒，发下一帧最新状态）；前端新增 WS 停滞看门狗——WS 打开但 8 秒无帧时每 3 秒用 HTTP 轮询兜底刷新。看门狗随页面刷新即生效（无需重启服务端）；服务端修复在下次重启后生效。
- `agent.rs` 提示中命令上限文案已与引擎一致（16 条）。

**Token 用量与费用（DeepSeek Flash 官方价目估算）**

| 范围 | 回合 | 输入（未命中） | 输入（命中） | 输出 | 估算费用* |
| --- | --- | --- | --- | --- | --- |
| 三小时航母决战（至停用） | 每侧 27 | ~1.32M | ~0.12M | ~0.31M | ~$0.95 |
| 六小时击杀数（进行中，至记录时） | 32/33 | ~2.83M | ~0.14M | ~0.33M | ~$1.05 |
| 当日累计（两侧） | 119 | ~4.14M | ~0.26M | ~0.64M | ~$1.63 |

\* 按官方 peak 价（输入未命中 $0.30/M、命中 ~$0.014/M、输出 $0.60/M）估算；off-peak 输入半价则约再省 40%。以 `/api/agent/status` 的 `usage` 为准，实际账单以 DeepSeek 控制台为准。

### 9.8 三小时航母决战、BDA 与日志过滤实机（2026-10-19）

- 新增 `scenarios/carrier_duel_3h.json`：保留双方 2 艘航母、2 艘攻击核潜艇和初始航空兵，移除水面护航；潜艇初始位于距敌航母约 12 海里伏击位。到期按击毁平台数决胜。
- 8795 实机：seed 20261019，双方 execute + auto_run，15×，固定 300 秒公共规划窗。前 20 分钟双方航母 BDA 均为 0，但航母已因拦截碎片、火灾和进水损失少量 DP；这不是隐藏血量或重生，而是反舰导弹被 ESSM/RAM/CIWS/箔条/诱饵分层拦截。单枚鱼雷也会被航母诱饵骗走（`decoy_fooled`）。
- T+22:55 起出现真实命中：蓝方首次看到福特“1 次命中 · 舰体约 98%”；红方看到山东 2 次命中。随后双方改为鱼雷/导弹饱和攻击，BDA 持续累积；T+39 左右蓝方看到福特 43 次命中、0% 舰体，尼米兹 22 次命中、约 30%，红方仅剩尼米兹约 476/1605 DP。蓝方两艘航母已被击沉并从己方单位列表消失，战果计数增加，未再出现“满血重生”。
- 跑满 3 小时模拟：T+03:00:00 想定结束，红方胜，`time limit: kill count`，权威击杀 Blue 29 / Red 34，VP 0/0；蓝方两艘航母均损失，红方尼米兹以约 476/1605 DP 存活。为节省 LLM 成本，tick 5378 后停用 Agent 并以 60× 推进剩余 1.5 小时，因此不是双方全程持续决策的完整 LLM 对局；前 5378 秒的 Agent 行为和全部武器/损伤/胜负裁决是真实模拟。
- 结论：此前“航母打不沉”是可观测的防御链和单发鱼雷被诱骗，不是 HP 显示隐藏或实体复活。有效打法是潜艇进入 22 海里内齐射 2–4 枚、空中/潜射多波次饱和，先耗尽诱饵和点防御弹药。
- 新增 `SideState.bda` 与 `ContactObs.bda_hits/bda_damage/bda_hp_fraction`：只在该方持有新鲜航迹且实际命中时累积，不读取敌方隐藏真值；目标卡显示“X 次命中 · 舰体约 Y%”，无命中显示“尚无确认命中”。
- 修复 BDA 归属：新增 `damage::apply_hit_from(..., shooter_side)`，生产命中路径传入实际发射方；BDA 只记给发射方，友军误击不再把战果/估损记到敌方。新增 5 项 `bda` 集成测试，覆盖有航迹命中、无航迹、累计、陈旧航迹、友军误击和旧接口无归属。
- Agent 提示命令上限由文案中的 8 条校正为与引擎一致的 16 条，并加入“潜艇在鱼雷射程内优先齐射 2–4 枚、不要为 CAP 轮换让潜艇空等”的战术约束；该提示修改需重启服务后才会进入新回合。
- 日志：作战记录新增“重要/全部/交战/探测”筛选和“战果 / 结算”按钮；重要筛选实际只渲染约 17 条，缓冲区仍保留 400 条用于回看；“战果 / 结算”在战斗进行中显示本方确认战果/损失和“当前战果”说明，终局显示权威结果。
- 浏览器验收：目标卡实际显示 BDA（1 次命中 98%、33 次命中 21%、39 次命中 9%），重要/全部筛选切换、结算弹窗、关闭按钮均可用；1366×768 截图通过，1600×900 下 IAB 截图偶发双层合成，已改用 1366×768 取证。
- 证据：`E:\CodexBuilds\harpoon-agent-20261019\carrier-duel\Blue-final-10800.json`、`Red-final-10800.json`（终局双方观察与 result）；更早的 tick 2479、3404 快照和 `agent-status-final.json` 同目录。
- 新发现但未在本轮修复：终局蓝方观察有 225 条声呐浮标水面接触，而红方实际仅 34 枚浮标；原因是重新探测会为已陈旧航迹新建接触，旧记录尚未到 900 秒清理期。它不会伪造击杀，但会挤占接触列表和 AI 的接触窗口，后续应合并陈旧航迹并在自动选敌/参谋上下文中降级或隐藏浮标。
- 验证：`cargo test --workspace` 140 项通过（核心 97 + 区域拒止 7 + BDA 5 + 击杀 3 + 击杀权重 3 + 服务端 22）；`node --test tests/*.test.cjs` 38 项通过。8795 保留六小时击杀数实况（旧引擎，WS 修复靠前端看门狗兜底）；8799 运行最新 release（加权分、在飞弹药、WS 修复）并开启双方 Agent 烟测。

## 10. 边界与后续

- 参谋是战术顾问，不是规则裁判；报告可能误判，命令合法不等于战术正确。
- 不提供外部 MCP 客户端能力；提示中的 MCP 工具清单用于让模型理解同一权威接口和命令 schema。
- 记忆按“想定 + seed + 阵营”保存，不是跨对局的战略记忆；换 seed 会建立新记忆。
- 严格双盲模式每个 300 秒公共规划窗会暂停执行型 Agent；慢模型会拉长该窗。5 分钟局建议 15×，30×/60× 不保证每个窗口都快速完成。
- 3 小时决战的完整对局验收尚未执行（只做了 tick 304 烟测）；场景和 Agent 行为可用大厅/API 启动。
- 普通自动推进的全局时间由玩家配置；导演模式由双方模型返回 `desired_speed`，服务端取 min 并限制 5–60×。
- 导演模式私有唤醒会暂停双方，存在“暂停本身泄露触发”的快速对抗取舍；严格双盲评测应改用公共唤醒或固定规划窗口。
- 视频为截图采样而非连续录屏；IAB 录制 API 在本机异常提前结束，已记录。
- 早期测试局出现 `impact` 携带 Red 单位名却对 Blue 可见：来源是 BDA 广播曾把“受害者的敌方”当作射手。现已由 `apply_hit_from` 传入实际发射方，友军误击不再产生敌方 BDA；UI 仍只在本方单位列表中显示目标损失。
- DeepSeek 的 `max_tokens` 预授权会消耗余额快照；生产配置不要使用 128k。实际用量以 `/api/agent/status` 的 `usage` 为准。
- 尚未实现：多参谋并行投票、外部知识库检索、浏览器端 API key 设置页、跨局长期战略记忆、语音播报。
