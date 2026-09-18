# Repository Guidelines

## Project Structure & Module Organization

This repository contains `technetium-battle-bot`, a Rust 2024 binary for the “未来战争” competition. It currently has no dependencies, and `src/main.rs` only prints a greeting; the bot and HTTP server are not implemented yet.

- `Cargo.toml`: package metadata and dependencies.
- `src/`: Rust source. Add focused modules here as protocol handling and strategy develop.
- `docs/docs/任务书.md`: game rules and competition requirements.
- `docs/docs/接口文档.md`: HTTP integration and request/response schema.
- `docs/docs/request.txt` and `response.txt`: protocol examples.
- `docs/Demo/CoreGeek.tar.gz`: supplied demo archive.
- `target/`: generated Cargo output, ignored by Git.

There is currently no dedicated test or asset directory.

## 项目文档导航与维护

| 文档 | 用途与更新时机 |
| --- | --- |
| [DESIGN.md](DESIGN.md) | 项目结构、分层 AI、FSM、事件/上报、世界模型、算法与接口契约；架构变化时更新。 |
| [HANDOFF.md](HANDOFF.md) | Agent 快速交接：当前实现状态、检查结果、限制和下一步；每次阶段交接时更新。 |
| [PLAN.md](PLAN.md) | 实施阶段、依赖、待办与验收条件；仅按实际完成和验证结果勾选。 |
| [AGENT_CONTEXT.md](AGENT_CONTEXT.md) | 开发中大模型交互形成的需求、事实、证据和上下文摘要；新增约束或工作结束时更新。 |
| [DECISION.md](DECISION.md) | 重大方案选择、依据、代价及重新评估条件；决策改变时保留历史并追加替代记录。 |

开始工作先读 `HANDOFF.md` 和 `AGENT_CONTEXT.md`，再查阅对应设计与计划。当前框架仅完成设计，文档中的目标目录、接口示意和性能目标不代表已经实现或测量。规则未决项统一维护在 `DESIGN.md`，不要把 Demo 假设写成正式规则。开发上下文与比赛运行时的 LLM/SOP 记忆分开管理。

## Build, Test, and Development Commands

Use a Rust toolchain supporting edition 2024. Run commands from the repository root:

- `cargo build`: compile a debug binary.
- `cargo run`: run the current entry point.
- `cargo build --release`: build the optimized binary in `target/release/`.
- `cargo test`: run Rust tests; none are currently defined.
- `cargo fmt --check`: check formatting with rustfmt.
- `cargo clippy --all-targets -- -D warnings`: lint and reject warnings.

Run `cargo fmt` to apply formatting before submitting changes.

## Coding Style & Naming Conventions

Follow rustfmt defaults, including four-space indentation. Use `snake_case` for modules, functions, and variables; `PascalCase` for types and traits; and `SCREAMING_SNAKE_CASE` for constants. Keep transport, game-state modeling, and strategy logic in separate modules as they are introduced. Preserve documented JSON field names, such as `roundNo` and `roleCommandMap`, when implementing serialization.

## 代码开发要求

以下是必须满足的约束，适用于业务代码、测试代码和项目维护的脚本。修改触及的文件或函数超限时，应在本次变更中完成合理拆分；不得通过压缩排版、隐藏到宏或堆叠闭包规避限制。

| 项目 | 硬性要求与统计口径 |
| --- | --- |
| 文件长度 | 单个代码文件不超过 **500 行**，按格式化后的物理行统计，包含空行、注释和测试代码。 |
| 函数长度 | 单个函数不超过 **50 行**，从函数签名起至结束括号止，包含多行签名、空行和注释；方法及承担独立逻辑的闭包同样遵守。 |
| 分支与循环 | 单个函数内分支和循环合计不超过 **6 个**，嵌套结构累加。每个 `if`、`else if`、`else`、`match` 分支臂、`for`、`while`、`loop` 各计一个；`if let`/`while let` 按对应结构计一次，`match` 本身不重复计数。 |
| 数字常量 | 避免魔法数字。代码中的数字使用有业务含义的命名常量，数字字面量仅出现在常量定义中；覆盖索引、数量、默认值、阈值、超时、协议值及测试期望。可调参数通过配置读取，代码默认值仍定义为常量。 |
| 测试隔离 | 业务实现和测试实现必须放在不同文件，禁止在业务文件内编写测试函数或内联测试模块。测试文件也遵守上述规模与复杂度限制。 |

常量使用 `SCREAMING_SNAKE_CASE`，名称说明含义及必要单位，例如 `MAX_WEAPONS`、`DECISION_BUDGET_MS`；避免 `THREE`、`VALUE_500` 等仅替换字面量的名称。比赛规则常量集中在 `rules/`，策略参数归所属策略配置，局部实现常量放在所属模块或类型内，避免无归属的全局常量集合。

同时遵守以下开发规范：

- 模块、函数保持单一职责；优先通过明确数据类型、职责拆分、提前返回和规则表降低复杂度，不为满足行数制造无意义的转发函数。
- 遵循 DESIGN 的分层依赖与状态所有权；跨层使用类型化接口、事件和上报，不能越层修改状态或绕过动作仲裁。
- 外部输入、文件、网络和协议处理显式返回/处理 `Result`、`Option`；禁止依赖 `unwrap()`、`expect()` 或 `panic!` 处理可预期失败。错误提供上下文，日志不泄露密钥或敏感数据。
- 公共接口说明输入约束、返回值和失败语义；复杂规则注释解释原因及依据。保持最小可见性，不为方便测试随意扩大公共 API。
- 不引入无依据的依赖、无关重构或重复实现；新增依赖说明用途并核对比赛环境兼容性。并发、计算、队列与日志须遵守回合预算和容量限制。
- 代码变更完成后运行 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 及相关场景验证；同时检查文件/函数行数和分支循环数量。rustfmt/Clippy 默认配置不能替代上述自定义约束检查，未验证项必须明确记录。

## Testing Guidelines

Use Rust’s built-in `#[test]` framework. 单元测试放在独立文件，例如业务实现为 `src/world/mod.rs`，测试实现为 `src/world/tests.rs`，业务文件只保留 `#[cfg(test)] mod tests;` 声明。集成测试放在 `tests/`，测试数据放在 `tests/fixtures/`；不在业务文件内写 `#[cfg(test)] mod tests { ... }`。

Name tests after behavior, for example `rejects_out_of_bounds_move`. Cover parsing, command serialization, and game-rule boundaries when adding those features. 验证可观察行为、错误路径与边界，缺陷修复添加能复现该问题的回归测试；测试须确定、独立，不依赖执行顺序或真实外部服务。No coverage threshold is configured.

Treat the supplied protocol examples as reference material: `response.txt` contains repeated keys and syntax issues. Create valid, focused fixtures before using it in automated tests.

## Commit & Pull Request Guidelines

Use short, imperative subjects such as `Add request parsing`. Keep commits focused.

代码变更完成并通过相关检查后，必须及时执行 `git commit` 并推送远端服务器，不将已完成改动长期留在工作区。提交前检查 `git status`、`git diff` 和暂存区差异，只暂存本次任务相关文件，保留用户已有改动；提交说明应准确描述变更目的。

推送到当前工作分支配置的远端分支；首次推送应核对远端和目标分支后设置 upstream。不得跳过 hooks、强制推送或覆盖远端历史。推送失败时保留本地提交，处理可安全解决的问题后重试；仍受阻则明确报告原因、提交号和待完成步骤，不能宣称已经推送成功。

Pull requests should explain the behavior change, link applicable issues or specification sections, and list validation commands and results. Include representative request/response examples for protocol changes. Avoid committing generated build output or machine-specific files.

## 框架变更与文档同步

功能变更涉及模块结构、分层职责、公共接口、FSM 状态/转移、事件/上报、世界模型、规则或运行时约定时，必须在同一次变更中更新相关辅助文档，并与代码一起提交：

- `DESIGN.md`：更新最终架构、接口、状态表和规则约束，避免保留已失效的设计描述。
- `DECISION.md`：记录重大选择、理由、代价和替代关系；保留历史决策。
- `PLAN.md`：同步实现步骤、依赖与验收状态，仅勾选实际完成事项。
- `HANDOFF.md`、`AGENT_CONTEXT.md`：更新实际实现进度、验证结果、已知限制和下一步。
- `AGENTS.md`：开发命令、目录结构或贡献约定发生变化时同步更新。

提交前对照文档导航核查受影响文件；未涉及某项内容无需机械修改对应文档，但不得以“稍后补文档”代替本次必要同步。
