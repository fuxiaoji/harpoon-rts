# 2026-10-30 开发进度快照

本目录保存上级本地资料仓的协作约束、计划、完成记录、歧义及核心验证清单，便于 GitHub 查阅。这里只是同步副本；本地工作仍以上级资料仓为准。

- 全书、全部可选规则、全部真实装备附件均未完成；无可信完成百分比。
- integration-92 核心批记录 exit_code=0，输入558项；这不是整套工程验收。原始81主台账及后续在建状态按原样保存，不倒签为92全书验收。
- 本次上传前实际执行 cargo check --workspace -j 1：核心库可检查，服务端编译失败。agent.rs 的通信命令缺 SendVisualSignal 分支；反潜命令缺 ProjectorPairPattern、ProjectorPattern、AirDepthChargePattern、AirDepthChargeDepthReport、AirDepthChargeSelectDepth 分支。未为上传改变游戏逻辑。
- 当前分支为进行中版本，不作为可发布版本，也未合并到 main。
- 原始手册、附件PDF、构建缓存、本地战报及大型证据不随本次新增上传；清单只供来源和版本追溯。
- 科研已经分仓；旧 research/realtime 内容仅作历史。
