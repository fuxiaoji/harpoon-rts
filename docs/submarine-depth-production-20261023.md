# 第3.2.1章来源潜深程序

权威为Harpoon V 2026-01-01英文PDF28/印刷3-6；先检索中文核心RAG“深度”，再视觉核对原页Submarine Depth Bands与Submarine Speed/Depth Change两表。证据及原件SHA在artifacts/rules-following-20261023/navigation。适用显式hv2026-rts-v4逐艇来源/作者条件，不由型号名称推测水深、潜深或水面最大航速。

来源上限与实水深、VSmall或其他潜艇水深门、原深度带端点、1/2/3/5/8带每180秒及独立三分钟水面/下潜/P-S浅深转换进入navigation-2026.json。生产入口为Command::SetDepth→navigation2026::submarine::order_depth/advance→实际实体深度；同一目标的重复命令不重置正在执行的作业。工作状态、已用秒数与本方来源fit随World保存，仅本方navigation.source_rules可见。

明确RTS改编：普通潜深在原表深度带端点间连续插值，每秒消费实际航速对应的带数额度；小数航速按原表上限分档，源整带端点保持原表时长。水面/下潜和P-S↔浅深是独立180秒工作，下潜完成前真实深度保持0、按水面目标结算，并受明确水面速度限制。深层须先到浅深，再单独命令上浮或P-S；无来源的零速普通潜深、超已核潜深、浅水和跨过下潜程序的订单拒绝或保持待裁。

旧Entity::max_speed里超过60米自动降至六成，是来源未支持的通用近似；它会破坏31节以上8带的真实执行。只对本包明确配置的来源潜深fit绕过该近似，保留真实机动系统和损伤限速。未配置单位及旧规则版沿用原算法。

测试先记录7红；初次修复6绿1红暴露上述旧限速。修复后来源潜深8项通过，见artifacts/rules-following-20261023/validation/following-core-red-green.log；同批其他领域仍有预期红测，整个新包尚未联合验收。

范围仍有限：Very Deep许可/内爆、Emergency Blow的Loud声纳状态、已核型号Crash Dive、柴油/锂电池/AIP、完整先行距离与每枢轴损速、倒车拖曳风险、碰撞/搁浅等继续分项补齐。普通深度程序不替代这些机制，也不称第3章完成。
