# 鱼叉 RTS（harpoon-rts）

经典兵棋《Harpoon》的电子化改编：以 Harpoon V 规则为裁决基线，把桌面回合制
改造为**实时战略（RTS）**——连续时间轴、可随时调节时间快慢（暂停~512×），
原"双盲人工裁判"由服务端**确定性裁决引擎**承担。

完整实施路线与验收标准见 [规则、前端与教学交付计划](plan.md)：逐章核验全部规则，分别验收严格规则和 RTS 改编，补齐玩家操作与递进教学。计划中的待办尚未代表功能完成。

## 核心特性

- **统一大厅**：根入口提供继续指挥、作战任务、训练学院、战局复盘和设置。任务确认后以暂停启动；原指挥台保留在`command.html`，浏览大厅不重置当前战局。见[大厅使用与验收说明](docs/command-lobby.md)。

- **确定性引擎**（Rust）：`种子 + 想定 JSON + 指令日志 ⇒ 完全一致的终态与事件流`。
  复盘、回归、RL rollout 全部可复现。实测 ~7.5 万 tick/秒/核（release）。
- **RTS 时间轴**：1 秒固定 tick 的权威模拟；时间倍速 0×–512×，服务端节拍驱动。
- **双盲电子裁判**：每方独立接触档案（persistent 误差向量、ESM 方位线、
  质量衰减），敌情真值永不下发；`observe()` 是唯一合法读接口。
- **规则贴合**：雷达视距闭式公式对齐规则表（1.23·(√h1+√h2)）、hp/伤害按官方
  DP/SDPS 量级标定、舰炮射界带、被动声纳航速-噪声关系。逐条改编见
  [docs/design/adaptations.md](docs/design/adaptations.md)，独立规则合规审计见
  [docs/compliance/rules-audit-001.md](docs/compliance/rules-audit-001.md)。
- **游戏内 AI 参谋**：右侧“AI 参谋 / AGENT”面板可启用 DeepSeek Flash 等模型参谋。每回合只接收与玩家相同的双盲 `Observation`，具备长期记忆、每 5 分钟模拟时间主动汇报、对话、建议/执行两种自主度；可开启服务端自动推进（1–60×，速度由玩家选择），并常驻显示战果/损失、弹出击杀/损失/重损卡和终局结算。所有命令仍由引擎按己方单位、接触、武器和甲板状态二次校验。支持独立双 Agent 完整对局；导演模式支持私有唤醒、双方同时暂停规划、模型请求速度取最小值推进。见[游戏内 AI 参谋与双 Agent 对局](docs/agent-assistant.md)。
- **预留接口**：
  - AI 接口：`CommandStaff` trait + 启发式参谋 + OpenAI 兼容 LLM 提供方
    （`HARPOON_LLM_BASE_URL/API_KEY/MODEL`，DeepSeek/GLM/vLLM/Ollama 均可）
  - MCP：`crates/harpoon-mcp` —— 引擎以 MCP stdio 服务暴露（tools/call）
  - 训练接口：`crates/harpoon-py` —— PyO3 绑定 + Gym 风格环境
  - 信息读取：REST `GET /api/state`、`GET /api/db`、WebSocket 推送

## 新手先读

**[图文规则教学手册](docs/manual/规则教学.md)** —— 界面导览、五个核心概念、
手把手第一局、真实战报案例、RTS 改编对照，全部基于真实截图与实测数据。
**[探测与编队篇](docs/manual/探测与编队篇.md)** —— 零基础声纳科普（主动/被动
到底差在哪）、预警机用法、舰载机任务分配、航母战斗群标准指挥流程。
**[实战教程](docs/manual/实战教程.md)** —— 反潜任务全流程、三域（空中/水面/
水下）打击协同、导弹齐射防御四步法，配真实对局截图。
**[双航母击杀赛战报](docs/manual/aar/双航母击杀赛战报.md)** —— 6 小时全量
战报 + 实况截图 + 状态机 AI 架构说明。

## 快速开始

Windows本地可双击 `启动鱼叉.cmd`（本机桌面另有“鱼叉一键启动”快捷方式）：自动检查构建并打开8795端口，初始暂停，重复启动保留战局。新增 [双盾行动：双航母区域拒止](docs/dual-carrier-denial.md)，包含辽宁/山东对福特/尼米兹四编队、权威区域任务胜负、规则参谋和有出处的NASA离线卫星底图。影像用于地理参考，地形裁决范围见简报。

新增 [训练学院与中英双语使用说明](docs/training-academy.md)：顶部“训练中心”进入11课76目标的递进教学（8课核心 + 3课实战），顶部或课程目录切换中文 / English。教学基于当前RTS改编配置，实际目标由服务器核验，支持检查点重试与结业记录。

看不懂开火与自主行动时，先点顶部“怎么玩”：会暂停并解释单发射击、指定目标持续交战、自动选敌、探测和战报。参见 [玩家操作说明](docs/player-operations.md)。

游戏内 AI 参谋使用 OpenAI 兼容端点，DeepSeek Flash 的最小配置：

```powershell
$env:HARPOON_AGENT_BASE_URL = "https://api.deepseek.com"
$env:HARPOON_AGENT_MODEL = "deepseek-flash"
$env:HARPOON_AGENT_API_KEY = "<runtime-only-key>"
cargo run --release -p harpoon-server -- --port 8799 --speed 0
```

大厅“作战任务”表单新增 AI 选项：勾选“与 AI 参谋对战”让红方由 Agent 指挥；“我的助手”可选关闭、半自动（只给建议）或全自动（AI 托管）；规划窗速度可选 1×/15×/30×/60×。该模式严格双盲、固定每 300 模拟秒规划，不使用私有事件暂停。新增 **3 小时舰队决战** `fleet_decisive_3h`，无区域任务，到期按击毁平台数决胜。

打开 `command.html`，在右侧 AI 参谋面板启用并勾选“允许下令”。服务端还提供独立双 Agent 对局：

```bash
curl -X POST localhost:8799/api/agent/duel -H 'content-type: application/json'      -d '{"scenario":"dual_carrier_kills","seed":20261019,"cadence_s":300,"max_ticks":21600}'
curl localhost:8799/api/agent/duel
```

`dual_carrier_kills` 是六小时击杀数想定：移除区域拒止，到期时击毁敌方平台更多的一方获胜。完整 API、记忆/节拍语义、用量统计和验收见 [游戏内 AI 参谋与双 Agent 对局](docs/agent-assistant.md)。

新增 [15分钟前端实操教学视频与章节索引](docs/frontend-video-tutorial.md)：在真实“太平洋风暴”一局中演示选舰、人工射击、持续目标指派、机动、航空和中文战报，完整推进到八小时裁决。

```bash
# 编译
cargo build --release

# 启动权威服务器（默认 8787，加载 scenarios/ 想定，服务 web/ 前端）
cargo run --release -p harpoon-server -- --port 8787 --speed 60

# 打开指挥大厅（原战术指挥台保留于 /command.html）
open http://localhost:8787/

# 跑一局确定性自对弈（HTTP）
curl -X POST localhost:8787/api/selfplay -H 'content-type: application/json' \
     -d '{"scenario":"crimson_tide","max_ticks":28800}'

# 全机制冒烟门（我方剧本驱动，18 项机制覆盖 + 确定性复现断言）
cargo run --release -p harpoon-server --example crimson_tide

# 核心测试
cargo test -p harpoon-core

# MCP 服务（在 MCP 客户端注册 command=cargo args=["run","-p","harpoon-mcp"]）
cargo run --release -p harpoon-mcp

# RL 环境（需 venv + maturin）
pip install maturin && maturin develop --manifest-path crates/harpoon-py/Cargo.toml
```

## 工作区结构

| 路径 | 说明 |
| --- | --- |
| `crates/harpoon-core` | 确定性引擎：几何/决定论RNG/数据库/移动/传感/武器/损伤/航空/想定/观察投影 |
| `crates/harpoon-server` | HTTP+WS 权威服务、时间倍速、AI 参谋、自对弈端点 |
| `crates/harpoon-mcp` | MCP stdio 服务（tools: new_game/get_state/send_command/step/selfplay…） |
| `crates/harpoon-py` | PyO3 绑定 + Gym 风格 RL 环境 |
| `web/` | 零构建 Canvas RTS 指挥界面（由 server 直接服务） |
| `scenarios/` | 想定 JSON（`crimson_tide` = 全机制冒烟想定） |
| `docs/design/adaptations.md` | 电子化改编声明（哪些桌面规则为何偏离） |
| `docs/compliance/` | 独立规则合规审计报告 |
| `plan.md` | 全规则范围、四联审计、前端命令覆盖、课程与阶段验收 |

## 工程纪律

- 每个里程碑一个 git 提交并同步 GitHub（`fuxiaoji/harpoon-rts`，私有）。
- 规则合规独立审计：审计员与实现者分离，违规项必须修复或补改编声明。
- 冒烟门（`crimson_tide` example）是集成回归必要条件；完整规则、数据、交互和教学还须通过 `plan.md` 定义的逐项验收。
- 授权资料（规则 PDF、数据库卷）不入库（见 `.gitignore`）。
