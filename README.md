# 鱼叉兵棋科研项目

目标是以 Harpoon V 为规则基线，逐步实现可验证的兵棋游戏引擎、人类对战、LLM 智能体、状态机智能体，以及 Transformer + 强化学习智能体。

当前阶段只处理“规则理解与资料治理”。原始资料保持只读，不改名、不搬移；Git 只追踪研究笔记、结构化清单、代码与测试，避免将大体积或受版权保护的原件纳入仓库。

## 目录

- `docs/rules/`：规则基线、机制摘要、术语与待确认事项。
- `docs/architecture/`：后续引擎与数据架构设计。
- `resources/catalog/`：全部原始资料的分类清单与版本谱系。
- `resources/processed/`：后续生成的结构化数据；大体积文本/渲染结果不入 Git。
- `scripts/`：可重复执行的资料盘点与提取工具。
- `tests/`：后续规则案例与回归测试。
- `agent.md`：协作约束和工程纪律。
- `plan.md`：分阶段路线图和验收标准。
- `havedone.md`：只记录已验证完成的工作。

## 规则基线

- 权威裁决基线：`14-2.HpnV_Rules_1_Jan_26.pdf`（2025-12-01 印刷，2026-01-01 数字更新）。
- 中文对照：`1-3.（核心）兵棋推演手册26版.pdf`（2026-02-19 数字版）。
- 快速操作参考：`2-2.（推演手册2026版本）Harpoon_V_Player_s_Handbook_(Chinese_Language).pdf`。
- 扩展损伤：`3-2.（扩展损伤手册26版）Harpoon_V_Expanded_Critical_Hit_Tables_(Chinese_Language).pdf`。

发生歧义时按“最新英文规则 > 官方订正/变更日志 > 对应中文译本 > 速查与辅助资料”的优先级裁决，并把差异写入 `docs/rules/open_questions.md`。

## 引擎快速开始

```powershell
$env:UV_CACHE_DIR='.uv-cache'
uv sync --extra dev
uv run pytest -q
uv run harpoon actions soviet --unit smyshlennyy-mod-kashin
```

跨命令对局使用 `--state` 读取存档、`--write` 写回存档：

```powershell
uv run harpoon --write tmp/session.json submit '<Command JSON>'
uv run harpoon --state tmp/session.json --write tmp/session.json lock nato
uv run harpoon --state tmp/session.json --write tmp/session.json lock soviet
uv run harpoon --state tmp/session.json --write tmp/session.json resolve
uv run harpoon --state tmp/session.json replay --jsonl tmp/session.jsonl
```

当前为严格规则子集：Tattletail 的六舰数据、反舰导弹射程/飞行/损伤、战备转换、DP 阈值和胜负规则已启用；未核验命中表由裁判裁决，其他章节能力会在行动列表中显示为不可用并解释原因。详见 `data/rules/coverage.json`，不得把接口存在误报为规则已完成。

## 本地游戏界面

```powershell
$env:UV_CACHE_DIR='.uv-cache'
uv sync --extra dev
uv run harpoon-ui
```

浏览器打开 `http://127.0.0.1:8897/`。默认存档位于 `tmp/ui-session.json`，刷新页面不会丢失进度。

操作流程：

1. 选择 NATO，点击单位，在“行动”页提交机动或等待命令，然后锁定本方。
2. 切换苏方，选择 `Smyshlennyy`，提交导弹攻击并锁定。
3. 切换裁判，逐次点击“推进当前阶段”。
4. 导弹在下一回合移动阶段抵达后，裁判点击海图上的导弹进行命中裁决。
5. “重置想定”会清空当前 UI 对局并重新载入 Tattletail。

首次游玩可点击右上角“新手教学”。玩家固定指挥 NATO，苏方由确定性脚本 AI 操控；界面会逐步高亮下一项操作，并带领玩家完成首个战术回合。
