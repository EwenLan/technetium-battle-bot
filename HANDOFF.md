# Agent 快速交接

> 更新：2026-09-20。状态：Rust 2024 比赛机器人首个可运行切片已建成；完整五层 FSM、认知作业与官方联调未完成。目标规格见 [DESIGN.md](DESIGN.md)、[BEHAVIOR.md](BEHAVIOR.md)、[INTERFACES.md](INTERFACES.md)，阶段验收见 [PLAN.md](PLAN.md)。

## 当前代码与入口

`bash run.sh <port>` 锁定依赖构建 release 二进制并启动 HTTP 服务。服务监听 `0.0.0.0`，接受 POST 请求，`runtime::Session` 串行处理回合；`protocol` 解析/编码 JSON，`world` 记录快照差分和有限记忆，`ai` 选择动作，`command::Arbiter` 统一校验并预约角色、目标格与建造金币。`tests/` 存放独立回归测试和有效协议 fixture。运行依赖为 `serde`、`serde_json`。

当前策略能根据昼夜、返防时间和近似基地风险选择阶段；工人按有效价格采矿/出售，必要时用已有药剂，并在观测基地动态生成的武器环内尝试三类武器建设；开拓者走向可接任务点（同名两格任务点合并寻路/接取邻域）、接题、发送一次 `prompt` 并只在紧随其后的回合提交 `llmResp`；夜间为已有炮选择可用控制者、移动到邻位或对可见机器人开火。A* 使用八邻域与有界扩展。世界记忆区分普通敌人失去视野、已确认死亡及全图可见建筑移除；新闻只保存有界原文，不做语义推理。

用户确认基地占 2×2：以接口给出的左上角为锚点，外扩一格的 4×4 减去基地得到 12 格武器环；外扩两格的 6×6 减去 4×4 得到 20 格围墙环。`rules/build.rs` 按观测基地生成并裁剪到地图边界，因此换边不需要坐标表。武器自动建设已启用；围墙动作会校验外环和工人石头，但自动围墙布局/出口连通规划仍未实现。

## 实际范围与未完成项

已存在世界、战略、任务、战术、个体状态枚举，但主要只有战略阶段选择和已提交动作的简化等待/回执处理；不得将 enum 当作完成的状态机。仲裁选择的动作才会登记为 `WaitingResult`，下一新回合按 `lastRoundRoleActionResults` 生成 Step 级合法、拒绝或未知报告；拒绝/未知上报给任务/战术的简化状态。合法回执不等于实际效果。任务层当前直接调用战术辅助，缺少 `StrategicDirective`、任务 DAG/租约、完整 `OwnerPath`、事件 inbox 与上报路由。个体缺效果证据对账；世界缺证据化预测与完整实体生命周期。仲裁目前允许 `move/collect/sell/build/attack/acceptTask/submitAnswer/use Medicine` 的受限子集，其他动作虽可编码但不会被批准。夜间火力是基础贪心，无穿透/溅射结算模型和联合配对。

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

继续按 PLAN 补齐 P0 的其余正式规则与运行环境证据，再完成 P1 的正式事务/异常服务测试、P2 的世界/FSM/事件上报、P3 的动作与经济闭环、P4 防守、P5 认知、P6 新闻宝藏、P7 全场回放。建造区域 U01 已解决；围墙布局仍须先验证基地出口与操炮通路。每次代码变更按 [AGENTS.md](AGENTS.md) 限制拆分、测试、同步文档并及时提交推送。

工作区基线存在用户已有 `.gitignore` 修改及未跟踪的 `.DS_Store`、`docs/` 原始资料；不要清理或混入提交。提交前只暂存本阶段源码、测试、Cargo 文件、脚本与已同步文档。
