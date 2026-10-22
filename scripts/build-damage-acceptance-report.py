"""Keep the implementation ledger separate from the frozen 2026-10-21 audit."""
from pathlib import Path
from collections import Counter
import json

ROOT = Path(__file__).resolve().parents[1]
baseline = json.loads((ROOT / 'docs/damage-rule-coverage-20261021.json').read_text(encoding='utf-8-sig'))
rows = {}

def mark(ids, status, implementation, tests='', remaining=''):
    for suffix in ids.split():
        rows['HV26-DMG-' + suffix] = dict(status=status, implementation=implementation,
            acceptance=tests.split(), remaining=remaining)

mark('PHASE', 'tested_rts_subset', '一秒命中账本、先收集再结算普通DP；跨阵营输入交错不改变命中身份。',
     'tick_hit_batch_is_order_independent_and_survives_save_before_resolution opposite_sides_network_interleaving_cannot_change_tick_outcome',
     'RTS-02-01/02：替代桌面阶段；即时空战/BOL等其他链路尚非全阶段复现。')
mark('DP', 'tested_source_subset', '已核附件使用固定普通DP；不再加±15%舰体浮动。',
     'yu6_uses_surface_dp_and_influence_fuze annex_variants_and_notation_are_not_inferred_from_damage_bands',
     '仅限damage-runtime-v2列出的武器；其余数据库标量尚未逐条核附件。')
mark('SPEED', 'tested_source_subset', '已核舰卡DP断点/离散航速由实际移动消费。','damage_speed_uses_annex_threshold',
     '未核平台仍有通用尺寸/速度回退，不能作为全舰卡完成。')
mark('MASSIVE', 'tested', '剩余DP≤25%限制武器、潜艇浮航与航母起降。',
     'massive_damage_prevents_carrier_launch a_massively_damaged_submarine_can_reach_the_surface')
mark('SMALL-DP', 'tested_rts_subset', 'F/G普通HP不跨战术时长累积；滚动180秒2倍原DP门槛。',
     'small_craft_uses_a_tactical_damage_window_not_a_one_second_threshold', '尺寸元数据覆盖待补；滚动窗为RTS-14-10。')
mark('RATIO RATIO-HIGH', 'tested_rts_subset', '同tick普通DP/命中后剩余DP查11×6表；超过1每0.2加一次，≥3压至10%原DP；修正浮点边界。',
     'ratio_thresholds_preserve_printed_decimal_boundaries small_damage_never_bypasses_one_percent_critical_gate', '异种命中的普通关键归属按普通DP加权，RTS-14-01。')
mark('RATIO-MIN', 'tested', '小于1%原DP不触发普通比率关键；独立附加程序不被这条取消。', 'small_damage_never_bypasses_one_percent_critical_gate')
mark('MISSILE-EXTRA MISSILE-FIRE', 'tested_source_subset', '依据附件明确骰式独立抽附加关键及自动火灾；DP不含骰值。',
     'carrier_uses_aviation_column_and_independent_automatic_fire annex_variants_and_notation_are_not_inferred_from_damage_bands', '未核武器不按DP区间推断附件骰式。')
mark('KEEL', 'partial', '单击60%原DP按鱼雷80%/炸弹导弹40%检定，记录骰数。', remaining='已接执行；尚需完整来源边界及排除条件独立验收。')
mark('LIGHT-GUN', 'partial', '≤65mm关键类别/半严重性/57–65mm进水减骰已接；不穿透不造成受保护关键。', remaining='炮射程段装甲、口径缺失与所有轻炮边界尚未全验收。')
mark('BOW-EXTRA', 'partial', 'v2不调用旧无依据首尾10%轻关键分支。', remaining='有依据的鱼雷首尾/尾流程序另项；尚需禁用旧分支的专门回归。')
mark('CLASS-AVIATION CLASS-MISSILE CLASS-ERA', 'tested_source_subset', '按平台role/尺寸/年代选择11列；已核航母/两栖航空舰不把护航舰直升机甲板算作航母。',
     'carrier_uses_aviation_column_and_independent_automatic_fire', '未核平台元数据仍须补齐；11列逐格执行边界尚未全覆盖。')
mark('CLASS-SUB', 'tested_source_subset', '水下直接命中采用潜艇重度列；浮航潜艇作为水面目标进入手动/自动开火与末段。新探测才更新接触类别。',
     'surfaced_submarine_is_a_legal_surface_missile_target major_control_damage_applies_both_bridge_and_cic', '轻度潜艇表虽已转录，真实近失/区域武器流程尚未连通；完整探测/鱼雷航深另审。')
mark('STARS ARMOR-PEN ARMOR-ASPECT', 'partial', '数据含逐列星号、belt/deck、Pen与局部CHP；导弹剖面参与普通DP及关键保护。', remaining='仅已核元数据；炮射程段/全部炸弹方式、各装甲等值边界待验收。')
mark('BOMB-PEN BOMB-DECK GUN-DECK AIRBURST-COUNT', 'not_implemented', '未作为完整对应程序验收。', remaining='炸弹方式/重量/穿透、集束与炮弹空爆数量、大炮远射自动甲板关键仍待实现。')
mark('TORP-TARGET-DP TORP-FUZE', 'tested_source_subset', '分对舰/对潜DP；触发/感应引信；触发鱼雷深浅设置。',
     'yu6_uses_surface_dp_and_influence_fuze shallow_contact_torpedo_uses_belt_and_never_consumes_tps', '已核Yu6/9/10、Mk48/54子集；全鱼雷设置/航深/有线制导未完。')
mark('TPS', 'tested_rts_subset', '两舷独立容量、单击R/3、同一区不可重复保护；触发深航才消耗。',
     'deep_contact_tps_is_separate_by_side_and_cannot_absorb_same_area_twice shallow_contact_torpedo_uses_belt_and_never_consumes_tps', 'RTS-14-04三分区D3、14-05按Enterprise例解释溢出，非书内空间表。')
mark('TORP-BOW TORP-FLOOD TORP-WAKE SUB-PRESSURE', 'partial', '窄首尾减伤、触发自动进水、尾流前两关键、按尺寸/直径/双壳自动耐压壳均有执行入口与骰日志。', remaining='仍缺所有源条件组合的独立验收；不能据函数存在报全通过。')
rows['HV26-DMG-SUB-PRESSURE']['acceptance']=['shaped_charge_applies_the_printed_modifier_to_the_roll_not_the_threshold']
rows['HV26-DMG-SUB-PRESSURE']['remaining']+=' 聚能装药按英文+20骰值字面实现，未按直觉改成提高阈值；见R-029。'
mark('MINE NUCLEAR BEACHED CREW-TRANSFER CARGO PERSONNEL', 'not_implemented', '本轮未完成对应完整程序。', remaining='人员仅计数/部分限制，货舱会明确记录未实现；水雷/核效应/搁浅/人员转移分别补源数据和执行。')
mark('AIRBURST-DP', 'tested_source_subset', 'PRH空爆0普通舰体DP、指定雷达优先关键；装甲阻挡弹片。', 'prh_airburst_has_no_primary_hull_dp', '普通炸弹/集束/炮弹空爆未全覆盖。')
mark('ARM', 'blocked_branch', '已实现发射前辐射检查、指定雷达、同目标改调谐与末次辐射位置。第三代关机末段缺明确倍率时保存待裁事件，不判中/失。',
     'anti_radiation_launch_requires_an_emitting_radar unknown_prh_shutdown_modifier_blocks_adjudication_instead_of_inventing_odds', 'R-027；不会把第二代的1/4无依据套到第三代。')
mark('AIRBURST-MAG', 'partial', '空爆武器关键排除弹药殉爆。', remaining='PRH路径已连接；炸弹/炮弹空爆完整来源分型尚缺。')
mark('DECK-LOC DECK-RECOVER', 'tested_source_subset', '首/尾甲板分别禁放飞/回收；重大火灾/严重舰体损伤实际联动。',
     'massive_damage_prevents_carrier_launch legacy_flight_deck_marker_cannot_be_silently_ignored', '中部弹射器只记录；实际起降吞吐率、VTOL、天气与甲板调度仍待。')
mark('HANGAR AIRCRAFT-STORED', 'partial', '机库抽D6架ready/cycling存放机；逐架火灾有不同身份；排除在空机。', remaining='RTS-14-08瞬时起飞库位；真实停放位置及可选挂弹/密集停放链未启用。')
mark('AMMO-FUEL', 'tested_source_subset', '航空弹药/燃料分支、殉爆、燃料火灾+2与损管+2；注水/惰化命令持久有代价。',
     'aviation_store_purge_leaves_unrelated_fires_burning emergency_magazine_flooding_is_an_owned_persistent_command', '弹药/燃料完整概率分支仍缺专门穷举验收；惰化UI本次单击未触发，只有核心案例。')
mark('AIR-PLOT', 'partial', 'D6分舰桥/塔台，塔台失能暂停勤务。', remaining='完整起降减速率与通讯联动未验收。')
mark('FIRE-SEVERITY FIRE-DELAY DC-CATEGORY DC-ROLL', 'tested_rts_subset', '逐火水严重性/首次540秒/后续1800秒、独立损管D10与大小年代合计等级；控制先于当期二次DP。',
     'ordinary_fire_does_not_drain_hull_during_first_seconds aviation_store_purge_leaves_unrelated_fires_burning damaged_carrier_save_preserves_casualty_clocks_and_command_replay', '损管变化按比例分摊但保留各自截止时间，RTS-14-03；全表组合边界未穷举。')
mark('DC-OPS', 'tested_source_subset', '重大火水限制航速/起降/深度；潜艇严重进水首次损管无效必须浮航。',
     'severe_submarine_flooding_gets_one_control_attempt_before_forced_surface', '风向30度、夜间照明与轻微火灾通气条件尚未完整实现。')
mark('DC-CATASTROPHE', 'tested_source_subset', '失控火灾累积25%/中间回合，注水抑制殉爆；倾覆25%/回合。',
     'conflagration_risk_accumulates_and_flooding_magazines_prevents_detonation', '倾覆完整概率边界仍待独立验收。')
mark('DC-CREW', 'partial', '抽调武器组后停火，1800秒后降有效严重性。', remaining='邻舰100码/同航向航速/低于10节/30分钟援助尚未实现。')
mark('ENG', 'tested_source_subset', '轮机降速度级、单机/潜艇双减、D6-2火与电力时长。',
     'mixed_battery_row_becomes_engineering_on_nuclear_submarines', '局部手动武器等电力例外尚未实现，完整源条件未全验。')
mark('BRIDGE', 'tested_rts_subset', '失能保持原运动、清旧航路；D6/2TT后降级，后续新航行令延迟2TT。',
     'bridge_casualty_holds_motion_and_delays_new_orders', 'RTS-14-06；航空起降率减半仍未实现。')
mark('RUDDER', 'tested_rts_subset', '卡死保存角速、失灵1/3转向/航速、D6TT后20/40/60三次解卡。',
     'jammed_rudder_ignores_a_new_course_and_disabled_rudder_slows_turning', '潜艇5%×航速的深度控制风险尚未实现。')
mark('CIC SENSOR', 'partial', '单个传感器失能进入实际探测/ESM/观测；CIC保留导航雷达。', remaining='CIC局部火控、ECM/通讯设施全类型与相应能力例外未完。')
mark('WEAPON REPEAT-MOUNT MAGAZINE', 'tested_rts_subset', '按物理架选择，允许再中已毁架；架上/架下爆炸按元数据；缺位置时阻断该爆炸DP。',
     'one_weapon_critical_does_not_destroy_every_mount_in_a_fit', 'RTS-14-09池化弹药按架分配；指挥仪/弹射器/升降机抽选未完；大多数平台弹库位置待核。')
mark('FLOTATION', 'partial', 'F/G浮力计数、降速及沉没门槛接执行。', remaining='各尺寸与复合损伤未全验收。')
mark('SUB-BATTERY', 'tested_source_subset', '蓄电容量损失按电量比例作用、AIP先25%、核潜艇混合行转轮机。',
     'battery_critical_removes_charged_and_depleted_capacity_proportionally mixed_battery_row_becomes_engineering_on_nuclear_submarines', '舰型元数据覆盖与原续航/充电率表未完。')
mark('SUB-CONTROL', 'tested_source_subset', '重度控制舱同时舰桥+CIC，轻度分支。', 'major_control_damage_applies_both_bridge_and_cic', '轻度分支及全部控制效应仍待组合验收。')
mark('SUB-DEFORM', 'partial', '第一次限100米，第二次50米，第三次转耐压壳穿透。', remaining='选择超限潜深的25%/30%风险程序未完。')
mark('SUB-HULL', 'tested_source_subset', '浅于等于100米的50%浮航弃艇窗口与2D10TT沉没；深处立即致沉。',
     'shallow_hull_penetration_has_a_rescue_window_but_is_never_repaired', '受控深度/上浮动作已测，完整救援人员另列。')
mark('SINKING', 'tested', '水面舰/浮航潜艇致沉后D10×10分钟保存倒计时；水下潜艇/弹药殉爆立即沉没。',
     'sinking_is_a_saved_countdown_except_submerged_boats_and_explosions', '战斗失能时停止机动为RTS实现；确认击沉/损失在实际沉没时记账。')
mark('RCS NOISE', 'not_implemented', '本轮未补损伤后的完整特征变化。')
mark('REPAIRS REPAIR-TYPES', 'partial', 'v2取消旧300秒安静自动修好系统；解卡、临时电力恢复单独处理。',
     'no_automatic_sensor_repair_at_five_minutes', '战后6/12/24/48小时修理、火灾复燃、进水补片与全部分系统效果未实现。')
mark('AIR-HIT', 'partial', '基本规则飞机成功命中即击落，不套舰艇关键表。', remaining='完整战后受损回收及可选分级飞机损伤未实现。')
mark('RNG GEOMETRY DICE-LOG', 'tested_source_subset', '损伤命中身份/逐骰/有符号实际来向持久化；同tick不复用流，日志只给受损方。',
     'same_tick_hits_have_distinct_auditable_random_identities hit_rolls_are_private_to_the_victims_side a_replayed_hit_identity_is_not_applied_twice', '损伤/舰炮流已改；其他武器Pk/BOL/友军误射等随机流仍需全面审计。')
mark('ROOT-DATA', 'not_implemented', '旧Python原型损伤表没有改成新的权威表；当前Rust不读取它。', remaining='R-026保留；不能把旧verified:true算作新Rust完成。')
mark('VARIANT', 'tested_source_subset', 'YJ12舰/空、高/掠四配置；YJ91/鱼雷目标DP、Mk54速度射程等来源子集。',
     'annex_variants_and_notation_are_not_inferred_from_damage_bands', '舰射YJ12高空段转末段距离无已核值；全附件/飞机挂载变体未完。')

items=[]
for old in baseline['items']:
    value=rows.get(old['id'])
    if old['optional']:
        value=dict(status='optional_not_enabled', implementation='可选规则单列，未启用。', acceptance=[], remaining='非基础规则完成分母。')
    if value is None:
        raise ValueError('Missing explicit implementation status: '+old['id'])
    items.append(dict(id=old['id'], title=old['title'], optional=old['optional'], rule=old['rule'], source=old['source'],
        data_path='data/rules/damage-runtime-v2.json' if value['status'] not in ['not_implemented','optional_not_enabled'] else None,
        execution_path='crates/harpoon-core/src/rules/damage_runtime.rs', **value))
report=dict(schema_version=1,date='2026-10-22',rule_profile='hv2026-rts-v2',full_rulebook_implemented=False,
    scope='原80审计项的实施跟踪，不是全书分母；tested_*_subset均未全项结案。',
    test_paths=['crates/harpoon-core/tests/rules_acceptance.rs','crates/harpoon-core/src/rules/damage_runtime.rs#source_effect_tests','crates/harpoon-server/src/persistence.rs','crates/harpoon-server/src/agent.rs'],
    sources=baseline['sources'],status_counts=dict(Counter(i['status'] for i in items)),items=items)
(ROOT/'docs/damage-rule-acceptance-20261022.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
md=['# 损伤规则逐项实施验收（2026-10-22）','',report['scope'],'',
    '状态含义：`tested`为本项已接通并有来源案例；`tested_source_subset`/`tested_rts_subset`只表示明确子集已通过；`partial`仍有条件/验收缺项；`blocked_branch`存在明确待裁分支；`not_implemented`未完成；可选项单列。不能把80项换算为全书实现率。','',
    '| ID／项目 | 当前状态 | 已实施 | 验收入口 | 尚缺条件 |','|---|---|---|---|---|']
for i in items:
    md.append('| '+ ' | '.join([i['id']+' '+i['title'],i['status'],i['implementation'], '<br>'.join(i['acceptance']) or '待补',i['remaining'] or '—'])+' |')
(ROOT/'docs/damage-rule-acceptance-20261022.md').write_text('\n'.join(md)+'\n',encoding='utf-8')
print(json.dumps(report['status_counts'],ensure_ascii=False))
