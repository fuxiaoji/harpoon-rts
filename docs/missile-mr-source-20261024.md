# R-059：真实导弹MR未知与独立防空来源

本次修复撤回六个中国反舰弹目标profile中没有明确来源的`maneuver_rating:0`。六条参数改为`null`并标明R-059待裁；signature、terminal maneuvers、sprint等独立来源字段保留。SAM入口读取MR时返回明确的来源待裁错误，先于许可、扣弹和命中骰，不再用非机动布尔值掩盖缺参，也不会让null进入数值`expect`而崩溃。炮AA和CIWS只读取它们需要的独立来源字段，可以继续结算。

来源结论来自三核心RAG、英文2026全本和原页，以及中美2025附件与本地订正的独立复核。英文PDF81/8-1 §8.1.1步骤9写减bogey MR；PDF83/8-3官方SM1ER对Kh-22N算例按2.0−2+0.5=0.5结算，没有MR扣项。后者证明该具体官方例的净贡献为零，没有把其他型号MR定义为零。PDF88/8-8与PDF102/9-1、105/9-4的明确MR0语句对象是飞机；PDF89/8-9导弹整个PD非机动说明在AA炮表上下文。中国附件D-1/D-2没有目标MR列。来源缺口是本项目严格自动裁决的待裁条件，不是声称原规则禁止对导弹射击。

原始独立审查和原页证据保留在根目录`artifacts/rules-antiship-20261024/mr-peer/independent-review.md`。本轮没有补通用中性MR解释，也没有改变导引头、ECM、命中表或独立终端机动−1。

## 作者假想目标

为保持防守链算法测试，可以在`ScenarioContext.aaw_2026`中声明独立的假想来袭目标。键必须以`scenario_assumption_`开头且有非空后缀；不能用真实附件ID。所有profile字段必须显式提供，MR可以是明确数值或null；`source`必须精确为`scenario_assumption`。

```json
{
  "aaw_2026": {
    "incoming_profiles": {
      "scenario_assumption_aaw_missile": {
        "signature": 3,
        "maneuver_rating": 0,
        "maneuvering": false,
        "terminal_maneuvers": false,
        "high_diving": false,
        "sprint_vehicle": false,
        "source": "scenario_assumption"
      }
    }
  }
}
```

这只声明防守算法的作者输入，不提供真实型号数据或ASCM发射许可。来源MR未知的真实ID不能通过作者map覆盖。profile保存在完整World的context中；作者雷达getter只提供signature，不提供物理size。旧规则配置下该getter不启用。

`world_target`返回拥有自身数据的Value，统一提供真实附件、已登记精确弹体或作者目标的独立字段；SAM在后续专门要求MR。真实六型profile仍存在，因此不会因MR未知在共享入口一并挡住枪炮。

## 测试与范围

根代理先运行`missile_mr_2026`：首次仅有夹具导入错误，不计源红；纠正后正常编译的四项真实断言全部失败，日志为`artifacts/rules-antiship-20261024/mr-source-red-corrected.log`。它们覆盖六型来源值、真实分类目标的SAM准入、无扣弹/假骰结果，以及同对象近防炮的独立路径。生产修复在该真实红之后进行。

新target共六项，另外覆盖完整作者字段、禁止覆盖真实ID、加载验证、保存恢复、明确作者MR和独立终端修正。旧`aaw_2026`保留25项：官方Kh-22N算法例继续存在；三个SAM正例和director负例改用显式假想目标。真实YJ-83的枪炮案例仍使用真实profile且明确断言MR为null。共同夹具改为受控来袭实体，不调用已禁止的v4真实ASCM裸发射；生产发射门没有放宽。

根代理随后在`artifacts/rules-antiship-20261024/core-joint.log`完成精确定向验收：本专题6/6、旧AAW25/25、反舰10/10及入口4/4，共45个case执行通过。这里的45只是这些定向target的case数，不是规则数、整库验证或全书完成率。子代理未运行Cargo；完整World、旧档与联合工作区另由根代理验收。

原来源红日志和候选SHA保留。后续若为本轮另立的观察隐私修复改动同一`aaw2026.rs`，应保存新专题的独立快照，不把新代码追记为原R-059定向验证版本。真实导弹SAM严格MR解释继续待裁，六型没有因作者算法正例而记成来源验证完成。
