# Agent 快速交接

> 更新：2026-09-19。状态：Rust 2024 比赛机器人首个可运行切片已建成；完整五层 FSM、认知作业与官方联调未完成。目标规格见 [DESIGN.md](DESIGN.md)、[BEHAVIOR.md](BEHAVIOR.md)、[INTERFACES.md](INTERFACES.md)，阶段验收见 [PLAN.md](PLAN.md)。

## 当前代码与入口

`bash run.sh <port>` 锁定依赖构建 release 二进制并启动 HTTP 服务。服务监听 `0.0.0.0`，接受 POST 请求，`runtime::Session` 串行处理回合；`protocol` 解析/编码 JSON，`world` 记录快照差分和有限记忆，`ai` 选择动作，`command::Arbiter` 统一校验并预约角色、目标格与建造金币。`tests/` 存放独立回归测试和有效协议 fixture。运行依赖为 `serde`、`serde_json`。

当前策略能根据昼夜、返防时间和近似基地风险选择阶段；工人按有效价格采矿/出售，必要时用已有药剂，在配置合法建造掩码后尝试三类武器建设；开拓者走向可接任务点、接题、发送一次 `prompt` 并只在紧随其后的回合提交 `llmResp`；夜间为已有炮选择可用控制者、移动到邻位或对可见机器人开火。A* 使用八邻域与有界扩展。世界记忆区分普通敌人失去视野、已确认死亡及全图可见建筑移除；新闻只保存有界原文，不做语义推理。

自动建造默认关闭。只有提供经过确认的 `BOT_BUILD_MASK_PATH=/absolute/path/mask.json` 才启用武器建造；格式为 `{ "weapons": { "challenger": [{"x": 4, "y": 4}], "defender": [...] } }`。`tests/fixtures/build_mask.json` 是合成测试数据，不是官方建造坐标。地图掩码、官方 HTTP 路由/运行环境与部分裁判语义仍属 DESIGN 第 12 节未决项。

## 实际范围与未完成项

已存在世界、战略、任务、战术、个体状态枚举，但主要只有战略阶段选择和少量工人状态记录；不得将 enum 当作完成的状态机。任务层当前直接调用战术辅助，缺少 `StrategicDirective`、任务 DAG/租约、完整 `OwnerPath`、事件 inbox 与上报路由。个体缺动作回执/效果对账；世界缺证据化预测与完整实体生命周期。仲裁目前允许 `move/collect/sell/build/attack/acceptTask/submitAnswer/use Medicine` 的受限子集，其他动作虽可编码但不会被批准。夜间火力是基础贪心，无穿透/溅射结算模型和联合配对。

认知仅覆盖单次 prompt/紧邻下一回合结果，尚无每日额度、作业代次、迟到结果墓碑、沙盒、SOP；新闻、宝藏、墙体、采购/升级/拆除与完整回放尚未实现。不能宣称适合正式比赛或达到目标性能。协议空响应、构建目标与启动方式还需官方环境验证。

## 验证与下一步

本地已用合成日间/夜间 fixture 验证 release 服务能返回 HTTP 200 JSON，并检查缓存、重复 key、世界差分、寻路、建造掩码/资金与武器控制冲突。开发时运行：

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release --locked
bash run.sh 45731
curl --noproxy '*' -sS -X POST --data-binary @tests/fixtures/day.json http://127.0.0.1:45731/
```

继续按 PLAN 补齐 P0 的正式规则与环境证据，再完成 P1 的正式事务/异常服务测试、P2 的世界/FSM/事件上报、P3 的动作与经济闭环、P4 防守、P5 认知、P6 新闻宝藏、P7 全场回放。优先解决官方建造掩码和运行说明；未解决时保守禁用相关动作，持续开发可验证的模块。每次代码变更按 [AGENTS.md](AGENTS.md) 限制拆分、测试、同步文档并及时提交推送。

工作区基线存在用户已有 `.gitignore` 修改及未跟踪的 `.DS_Store`、`docs/` 原始资料；不要清理或混入提交。提交前只暂存本阶段源码、测试、Cargo 文件、脚本与已同步文档。
