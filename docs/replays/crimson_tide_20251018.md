# Crimson Tide 全机制冒烟复盘（2026-10-18）

- 想定：`scenarios/crimson_tide.json`（seed 20251018）
- 驱动：`cargo run --release -p harpoon-server --example crimson_tide`
- 结果：**SMOKE PASS** — 18 项机制全覆盖，两次运行 digest 与事件计数完全一致
- 性能：8 小时战役 ×2 次（57,600 tick）0.76s 用户态 ≈ **75k tick/秒/核**

## 机制覆盖矩阵

| 机制 | 事件 | 次数 | 首次 |
| --- | --- | --- | --- |
| 探测（雷达/ESM/目视/声纳） | detection | 327 | t=4s |
| 接触识别 | contact_identified | 41 | t=8s |
| 发射反探测 | counter_detected | 332 | t=553s |
| 武器释放 | weapons_release | 332 | t=553s |
| 来袭预警 | incoming | 245 | t=556s |
| 防空射击 | sam_shot | 218 | t=958s |
| 拦截裁决 | intercept | 107 | t=980s |
| 箔条 | chaff_deployed | 21 | t=716s |
| 诱饵诱偏 | decoy_fooled | 29 | t=741s |
| 鱼雷告警 | torpedo_detected | 24 | t=2208s |
| 舰炮 | gun_action | 80 | t=16350s |
| 命中 | impact | 98 | t=762s |
| 关键命中 | critical_hit | 2 | t=1274s |
| 击沉 | sunk | 22 | t=1101s |
| 航空出动 | air_launch | 7 | t=301s |
| 航空回收 | air_recovered | 8 | t=1352s |
| 岸基受损 | base_damaged | 28 | t=16350s |
| 想定结束 | scenario_ended | 1 | t=28800s |

## 战役进程（Blue 判胜，时间限制）

- t=301s：CV 弹射 CAP（2×fighter 站位 44,44）、ASW 直升机（2×helo）
- t=553s：RED 导弹艇进入 SSM 包线齐射 → 双方探测/反探测链全速运转
- t=958s：Aegis 区域 SAM 开始拦截掠海弹；t=1101s 首舰击沉
- t=2208s：SSK 伏击线进入 9nm，重型鱼雷齐射；CV 诱饵对抗
- t=16350s：侦察机目视固定 KURE 基地 → DDG SLCM + 打击机对岸；近距炮战
- t=28800s：时间限制，VP 判定 Blue 胜

## 确定性

`digest 1cefa14acb4ec77b` 两次一致；事件计数逐项一致。
