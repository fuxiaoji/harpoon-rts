# 来源观察与 LLM/FSM 同步

本轮先注册真实回归，再由根代理串行验证。前两轮的夹具错误保留：UnitObs 不存在 ordered_altitude_m；来源四枚 Mk54 变体必须通过实际 v4 数据配置登记。修正后9项为2绿7真实红，见 artifacts/rules-closure-20261024/validation/fsm-llm-fixture-corrected-red.log。生产仅在该结果后修改。随后11项为9绿2条新增修理接入真实红，见 fsm-main-green-repairs-red.log；修理适配仅在该结果后实现，并在 fsm-llm-joint.log 实际11项全绿。

LLM 对全部命令通过共同 Command::unit_id 检查主行动单位的人控保留，包含重新启用自动化。提示词使用单份 core commands::command_schema，与 MCP 同源发现全部命令及嵌套 action/mission；加入本方 ground，保留现有 mines、aviation 和 underwater own_in_flight:null。提示仅由 Observation 和公共装备库建立，不读取敌方实体真态。

后续发现玩家队列也复用 AI 校验，真实队列2红已保存在 player-ai-origin-red.log。现已通过内部 typed Ai/Player 区分权限：validate_commands 保持 AI 入口，validate_player_commands 为人类队列入口；只有 AI 被人控保留与 begin_repairs 门拦住，玩家仍须通过同一归属、参数与来源合法检查。根代理已接入实际队列，两条真实队列回归在 player-ai-origin-fixed.log 全绿；完整服务端套件仍待最终复验，不能只用 AI 独立用例外推人控接口。

FSM 在 v4 来源航空 profile 下消费本方 aviation.aircombat.aerial_torpedo_release：按 max_altitude_m/max_speed_kts 准备实际高度与速度，等待 ready_at；速度来源未知则保持攻击待命。准备命令不会立即被视为实际就绪，也不会绕过真实 Fire/FCS。own_in_flight 未知保持阻断。v4 潜艇目标必须新鲜、可定位且坐标有限；来源 ASW 任务不再依赖呼号关键词判断角色。未启用该来源 profile 的旧路径有保留行为正例。

本方修理观察与人控开始门两项新增回归已形成真实失败再修复。begin_repairs 宣告战斗结束并开始修理须人类决定，LLM 明确拒绝；AI 只能在既有本方 v4 来源修理 session 中安排 repairs_auto/repairs_manual/repair_due 后续动作，最终修理资格与期限仍由实际核心门判定。本适配层不独立认定 §14.6 全部来源已验。

来源依据与范围见 artifacts/rules-closure-20261024/fsm-llm/README.md 及相邻 airdrop 原页证据。此处不关闭完整 R-054；完整空投螺旋搜索、再攻击与后续适配仍未完成。
