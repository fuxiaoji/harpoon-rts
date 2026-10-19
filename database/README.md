# 数据库卷（database/volumes）

官方数据库卷的结构化转录目录。每个 `<name>.json` 是一个 `harpoon_core::db::Database`
片段（platforms / weapons / sensors），由服务器启动时按文件名顺序合并进
`Database::built_in()`（卷内条目覆盖内置泛化条目）。`_` 开头的文件被加载器忽略。

## 文件清单与出处

| 文件 | 数据书 | 版本 |
| --- | --- | --- |
| us_navy.json | 5-4. America's Navy | 1 Dec 25 |
| cn_navy.json | 6-3. China's Navy | 1 Dec 25 |
| ru_navy.json | 9-4. Russia's Navy | 1 Dec 25 |
| commonwealth_navies.json | 17-1. Commonwealth Navies | 1 Dec 25 |
| us_aircraft.json | 4-4. America's Aircraft | 1 Dec 25 |
| ru_aircraft.json | 8-4. Russia's Aircraft | 1 Dec 25 |
| commonwealth_aircraft.json | 16-1. Commonwealth Aircraft | 1 Dec 25 |
| small_navy_korea_tw_ph.json | 20. 韩军 / 21. 台军 / 26. 菲律宾 | 混合 |
| small_navy_se_fi_ir.json | 27. 瑞典 / 28. 芬兰 / 29. 伊朗 | 混合 |

先导数据 `crates/harpoon-core/src/rules/db_fleets.rs`（26 类）保留；
卷内同名 id（us_/cn_/ru_/cw_ 前缀）按卷值精化覆盖。

## 校验

```bash
# 全量合并校验 + 输出统计
cargo run --release -p harpoon-server --example dbcheck
# 单卷校验（数据录入代理并行时使用）
cargo run --release -p harpoon-server --example dbcheck -- --only database/volumes/us_navy.json
# 导出合并库（供工具/大模型读取）
cargo run --release -p harpoon-server --example dbcheck -- --emit database/consolidated.json
```

## 约定

- id 前缀按国家/卷：us_ cn_ ru_ cw_ jp_ kr_ tw_ ph_ se_ fi_ ir_
- 同型多舷合并为一个类；变体用 `_a` `_b` 后缀
- 每条目的数值出处（卷+页）写在条目旁的 `_source` 注释不存在于 JSON ——
  出处统一记录在各卷同名 `.provenance.md`
- 拿不到精确值的字段宁缺勿造：量级推断必须在 provenance 里标 estimated
