# 数据库版本谱系

状态定义：`current` 表示同一出版线中日期最新；`historical` 仅用于差异回归；`parallel-current` 表示范围不同、不可相互覆盖的现行补充。

## 主数据库

| 数据族 | 已发现谱系 | 现行基线 | 状态说明 |
|---|---|---|---|
| America’s Aircraft | 早期 → 2024-08-23 → 2025-12-01 | `4-4.Americas_Aircraft__1_Dec_25.pdf`，126 页 | `current`；封面确认包含截至 2025-12-01 的更正 |
| America’s Navy | 早期 → 2023-03 → 2024-08-23 → 2025-12-01 | `5-4.Americas_Navy_1_Dec_25.pdf`，136 页 | `current` |
| China’s Navy | 早期 → 2023-03 → 2025-12-01 | `6-3.（数据库）2025版Chinas_Navy_1_Dec_25.pdf`，105 页 | `current`；同时包含中国舰船与航空器 |
| Russia’s Aircraft | 早期 → 2024-08-23 → 2025-12-01 | `8-4.Russias_Aircraft_1_Dec_25.pdf`，92 页 | `current` |
| Russia’s Navy | 2023/早期 → 2024-08-23 → 2025-12-01 | `9-4.Russias_Navy_1_Dec_25.pdf`，112 页 | `current` |
| Commonwealth Aircraft | 早期 → 2025-12-01 | `16-1.（数据库）Commonwealth_Aircraft_1_Dec_25.pdf`，99 页 | `current` 候选；需核对哈希/内部日期 |
| Commonwealth Navies | 早期 → 2025-12-01 | `17-1.Commonwealth_Navies_1_Dec_25.pdf`，112 页 | `current` 候选；需核对哈希/内部日期 |

## 中国海上力量并行出版线

`Modern Chinese Maritime Forces` 与 `China’s Navy` 不是简单的新旧关系：前者覆盖中国海军、海警、海上民兵及其他国家海上机构，后者是 Harpoon 数据附件式的海空平台数据库。

谱系：早期版 → 2023-01-01 → 2023-03-01 → Second Edition → 根目录 `11-4.Modern_Chinese_Maritime_Forces_Second_Edition26V2.pdf`。

根目录版本封面确认为 2026-07 数字更新、资料截止 2026-07-01、188 页，标记为 `parallel-current`。后续建模时可用它补充组织和平台信息，但具体 Harpoon 游戏参数优先取同版本规则兼容的数据附件；冲突必须进入差异台账。

## 地区与专题数据库

- 日本：2014 Naval SITREP → `Japan’s Self Defense Forces` 2026，后者作为现行候选。
- 瑞典：NSITREP 63，文件名标注 2025-11-13。
- 芬兰：NSITREP 64，文件名标注 2023-08-05。
- 伊朗：Naval SITREP 65，文件内部/文件名年代标注存在“2025/October 2023”冲突，暂不判定现行日期。
- 菲律宾、韩国水面舰艇、含台湾地区数据：来源为专题刊物或节选，需定位实际数据页和出版日期。

## 数据导入规则

1. 每条实体记录 `source_document_id`、出版线、数字更新日期、内部页码和有效年代。
2. 同一平台不同年代配置必须拆成 variant/fit，不覆盖历史配置。
3. 最新文档不是无条件覆盖：仅在同一出版线、同一字段语义和同一有效年代内使用优先级。
4. 旧版保留用于差异测试和复现旧想定，但默认查询排除 `historical`。
5. 自动文件名分类不作为有效性证据；封面日期、版权页、变更说明和具体条目需分别核验。

