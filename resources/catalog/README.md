# 原始资料分类规范

原件当前位于 `【20260405】鱼叉5（本体+中美俄数据库）/`，保持不动。分类通过 `inventory.csv` 和后续 manifest 表达。

类别定义：

- `rules/core`：完整主规则、中文译本、旧版规则。
- `rules/player-aid`：玩家手册、速查表、表单、计算器。
- `rules/damage`：致命损伤、扩展损伤、损伤点说明。
- `rules/errata`：订正、变更日志和专题规则修订。
- `data/aircraft`：航空器与航空武器数据库。
- `data/navy`：舰艇、潜艇、传感器和舰载武器数据库。
- `scenario`：可直接推演的想定与配套表格/地图。
- `research`：战术、运筹、历史和技术参考文献。
- `software`：既有程序、存档和地图资源，仅作为行为参考，不作为新引擎依赖。
- `archive`：重复包、旧版本和无法立即判定的资料。

`inventory.csv` 由 `scripts/build_inventory.py` 生成，覆盖展开目录以及工作区根目录单独存放的 PDF/ZIP；字段包括相对路径、扩展名、字节数、SHA-256、PDF 页数和初步分类。自动分类只用于导航，规则有效性仍需人工确认。
