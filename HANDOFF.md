# Agent 快速交接

> 更新：2026-09-21。状态：Rust 2024 比赛机器人首个可运行切片已建成；完整五层 FSM、认知作业与官方联调未完成。目标规格见 [DESIGN.md](DESIGN.md)、[BEHAVIOR.md](BEHAVIOR.md)、[INTERFACES.md](INTERFACES.md)，阶段验收见 [PLAN.md](PLAN.md)。

## 当前代码与入口

`bash run.sh <port>` 锁定依赖构建 release 二进制并启动 HTTP 服务。服务监听 `0.0.0.0`，接受 POST 请求，`runtime::Session` 串行处理回合；`protocol` 解析/编码 JSON，并校验全部已观测实体、任务点和基地完整 footprint 均在地图内；`world` 记录快照差分和有限记忆，`ai` 选择动作，`command::Arbiter` 统一校验并预约角色、目标格与建造金币。`tests/` 存放独立回归测试和有效协议 fixture。运行依赖为 `serde`、`serde_json`。

当前策略能根据昼夜、返防时间和近似基地风险选择阶段；工人按有效价格采矿/出售，必要时用已有药剂，并在观测基地动态生成的武器环内尝试三类武器建设；开拓者走向可接任务点（同名两格任务点合并寻路/接取邻域）、接题、发送一次 `prompt` 并只在紧随其后的回合提交 `llmResp`；夜间为已有炮选择可用控制者、移动到邻位或对可见机器人开火。A* 使用八邻域与有界扩展。世界记忆区分普通敌人失去视野、已确认死亡及全图可见建筑移除；新闻只保存有界原文，不做语义推理。

用户确认基地占 2×2：以接口给出的左上角为锚点，外扩一格的 4×4 减去基地得到 12 格武器环；外扩两格的 6×6 减去 4×4 得到 20 格围墙环。`rules/build.rs` 按观测基地生成并裁剪到地图边界，因此换边不需要坐标表。武器自动建设已启用；围墙动作会校验外环和工人石头，但自动围墙布局/出口连通规划仍未实现。

## 实际范围与未完成项

世界的 `ColdStart/Ready/Degraded` 已接入会话：地图尺寸或阵营在同一会话突变时保留最后有效快照、返回空响应，并在一致新帧到来后恢复；生命周期和差分事件进入稳定 ID 的有界日志。`EventInbox` 为四个层级读者维护独立 cursor，支持 poll/ack、截断检测和陈旧 receipt 拒绝；战略读者已接入决策草稿，失败草稿不会推进游标。当前没有信封过滤、实例路由和关键截断恢复，其他三层读者也尚未接入调度。

`domain::owner` 已实现类型化 mission/plan/intent ID、generation、确定性 `OwnerAllocator`、`OwnerPath` 与 `ActiveOwners`。不可变 MissionSpec 已包含 kind、稳定目标键、类型化 goal、依赖、所需能力、deadline、Q0–Q4 优先级、可中断性和重试策略；建设目标同时记录坐标与建筑类型。任务层的 `MissionRegistry` 按 MissionId 保存私有记录及只读 MissionView，直接读取 spec 依赖；前置成功会将依赖任务从 Proposed 唤醒到 Ready，取消会传播到依赖子树。完整 spec 相同时复用 mission/plan，每个动作提案创建新 intent；契约变化时替换 assignment。`command::Arbiter` 在接收提案和编译响应前各校验一次完整父链，等待和 Step 报告复用提案 owner。

任务对账在每次决策开始时运行：建设目标从存活己方建筑观测取证，挑战和守备目标分别从 `ChallengeEnded`、白昼 `PhaseChanged` 事件取证，`AllOf` 合并并去重证据；证据保存实体/事件 ID 与观测回合。满足目标的任务进入 Succeeded 并解锁前置已齐的任务；超过 EffectObservation/InternalPlanning 窗口的任务进入 Expired，依赖后代取消并同步失效 owner。ActionSubmission 只接受与目标匹配的 Sell/Build/SubmitAnswer/Attack 作为 checkpoint，按时提交后允许后续效果对账。迟到效果证据不会复活已过期任务。

任务候选在 owner 分配前执行统一能力准入。工人能力为 Gather/Sell/Build/OperateWeapon，开拓者为 SolveChallenge/OperateWeapon；required_capabilities 必须全部满足。角色不存在、确认死亡、生命未知或缺少具体能力分别返回类型化 CapabilityRejection，本地拒绝不会创建任务、消耗 ID 或替换现有工作。攻击按 controller 作为任务 assignee 检查，武器仍是协议 actor。

GatherAndSell 已有跨步 checkpoint。实际提交 collect 时保存目标矿种、该工人提交前数量和预计效果回合；下一连续帧必须同时出现该工人的 InventoryChanged 且指定矿物增加，才保存采集进展。后续提交 sell 时保存指定矿物预计剩余量和提交前金币；只有本任务已有采集证据、矿物按量减少、同帧 InventoryChanged，且 GoldChanged 从保存的金币值正向增加，任务才 Succeeded。MissionView 可读取 progress_evidence；动作合法回执本身不算效果。并发支出若掩盖金币净增长会保守保持 Pending，等待后续工作处理更完整的共享账本归因。

实时经济和建设任务使用当日最后回合作为 ActionSubmission deadline；挑战取该边界与 `首次候选回合 + timeoutRounds - ANSWER_MARGIN_ROUNDS` 的较早者；防守使用下一白昼首回合的 EffectObservation deadline。自动相对期限在首次 assignment 冻结，后续步骤复用，已过期 spec 在 owner 分配前拒绝。U10 的挑战 timeout 正式起算点尚未确认，当前从候选首次准入开始计时，可能比裁判更保守。

当前自动策略创建的任务仍不带依赖；MissionSpec 尚无资源租约，其他任务也没有完整 progress/lease。挑战的固定目标尚未区分具体挑战代次。简化 `ActionProposal` 只有 actor、owner、action，未包含正式契约的 ProposalId、TurnStamp、claims、预期效果和原子组。战略已有阶段选择，个体已有提交动作的简化等待/回执处理；其余 enum 不代表完成的状态机。仲裁目前允许 `move/collect/sell/build/attack/acceptTask/submitAnswer/use Medicine` 的受限子集。夜间火力是基础贪心，无穿透/溅射结算模型和联合配对。

认知仅覆盖单次 prompt/紧邻下一回合结果，尚无每日额度、作业代次、迟到结果墓碑、沙盒、SOP；新闻、宝藏、墙体、采购/升级/拆除与完整回放尚未实现。不能宣称适合正式比赛或达到目标性能。协议空响应、构建目标与启动方式还需官方环境验证。

## 验证与下一步

本地已用合成日间/夜间 fixture 验证 release 服务能返回 HTTP 200 JSON，并检查缓存、重复 key、世界差分、寻路、2×2/4×4/6×6 建造区、资金与武器控制冲突。开发时运行：

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release --locked
bash run.sh 45731
curl --noproxy '*' -sS -X POST --data-binary @tests/fixtures/day.json http://127.0.0.1:45731/
```

最新阶段已通过 rustfmt、Clippy warnings-as-errors、84 个 Rust 测试（17 个单元测试、67 个集成测试）、locked release 构建和本机 HTTP 200 JSON 冒烟。任务验证覆盖自动 deadline 窗口及冻结、显式期限替换、过期准入拒绝、经济跨步进展与错误归因反例、角色能力准入、目标证据、依赖传播、草稿回滚及旧回执隔离；该结果不代表完整 P2 或官方判题器联调完成。

继续按 PLAN 补齐 P0 的其余正式规则与运行环境证据，再完成 P1 的正式事务/异常服务测试、P2 的世界/FSM/事件上报、P3 的动作与经济闭环、P4 防守、P5 认知、P6 新闻宝藏、P7 全场回放。建造区域 U01 已解决；围墙布局仍须先验证基地出口与操炮通路。每次代码变更按 [AGENTS.md](AGENTS.md) 限制拆分、测试、同步文档并及时提交推送。

工作区基线存在用户已有 `.gitignore` 修改及未跟踪的 `.DS_Store`、`docs/` 原始资料；不要清理或混入提交。提交前只暂存本阶段源码、测试、Cargo 文件、脚本与已同步文档。
