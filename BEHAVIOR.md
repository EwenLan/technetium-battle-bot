# 状态、角色与决策算法实施规格

> 更新：2026-09-20。状态：完整逐状态目标规格，仅部分算法已落地，尚未实战调优；已实现范围见 [HANDOFF.md](HANDOFF.md)。现有 FSM enum 不表示全部状态行为已完成。
> [DESIGN.md](DESIGN.md) 管理架构与状态拓扑，[INTERFACES.md](INTERFACES.md) 管理类型和调用契约；本文定义每个状态具体调用的算法、守卫、角色规则及确定性选择顺序。未知裁判规则继续由 DESIGN 的 U02–U09 管理；U01 建造区域已确认。

## 1. 实施约定与命名参数

标记含义：**R** 为任务书/接口确认的规则；**P** 为本设计选择的策略参数；**U** 为未确认行为。P 值只是可运行基线，不宣称最优。实现时把数字定义为有含义的常量或版本化配置，不在状态处理器内散写字面量。

| 常量/配置键 | 首版值 | 类别与用途 |
| --- | --- | --- |
| `DAY_ROUNDS / NIGHT_ROUNDS / MATCH_ROUNDS` | 70 / 60 / 1300 | R：昼夜/比赛时长 |
| `MAX_WEAPONS / WEAPON_BUILD_GOLD` | 3 / 25 | R：三种武器合计上限、基础建造成本 |
| `LLM_DAILY_LIMIT / SUMMON_ORDER_DAILY_LIMIT` | 3 / 10 | R：普通 LLM 调用、机器人召唤令每日上限 |
| `SCORE_SCALE` | 1000 | P：效用、风险和置信度采用整数刻度 |
| `RETURN_MARGIN / POSITION_SETUP_ROUNDS` | 3 / 2 回合 | P：返防安全余量、未就位者的计划/站位开销估计 |
| `ENDGAME_WINDOW` | 130 回合 | P：进入最后一日后停止长期投资 |
| `THREAT_HORIZON / ROBOT_STEP_ESTIMATE` | 3 回合 / 1 格每回合 | P：威胁近似窗口和机器人移动估计；速度不是新增官方规则 |
| `BASE_RISK_ENTER / BASE_RISK_EXIT` | 400 / 200 | P：基地威胁进入/退出阈值，均以 SCORE_SCALE 为刻度 |
| `UNIT_RISK_ENTER / UNIT_RISK_EXIT` | 700 / 350 | P：角色保全阈值 |
| `EMERGENCY_CLEAR_ROUNDS` | 3 个连续观测回合 | P：退出紧急态所需稳定窗口 |
| `STALL_LIMIT / LOCAL_REPLAN_LIMIT` | 2 次无进展 / 3 次重规划 | P：对同一目标统计；没有发动作的等待不累计移动失败 |
| `ACTION_RESULT_GRACE / JOB_RESULT_GRACE` | 2 个后续观测回合 | P：未能对账的结果转 Unknown/超时；不代表可直接重发 |
| `BLOCKED_RECHECK_INTERVAL / BLOCKED_MAX_ROUNDS` | 2 / 6 回合 | P：普通阻塞重试与最长占人窗口 |
| `ASSIGNMENT_ACK_GRACE` | 2 回合 | P：战术分配握手等待上限 |
| `CANDIDATES_PER_ROLE / FIRE_TARGET_LIMIT / FIRE_OPTIONS_PER_WEAPON` | 6 / 8 / 4 | P：有界候选集；Idle/不攻击额外保留 |
| `LAYOUT_SITE_LIMIT` | 6 | P：每个缺失武器槽位保留的合法建造位置数；联合布局仍受计算预算限制 |
| `PATH_STEP_COST / PATH_RISK_WEIGHT / PATH_CROWD_WEIGHT` | 1000 / 2 / 100 | P：A* 基础步成本、目标格风险权重、目标格相邻己方角色数量权重 |
| `OPENING_LOADOUT` | Gatling、Railgun、Rocket | P：首版三炮基线，需地图/弹道规则可用；不是最优配兵结论 |
| `GATHER_BATCH_MAX / STONE_RESERVE_TARGET` | 8 / 4 件 | P：一轮采售目标；只为已安排墙体保留最多四块石头 |
| `ECONOMY_SETUP_ROUNDS / SUCCESS_RATE_PRIOR` | 6 回合 / 500 | P：新采售周期内部推进开销估计、无同族历史时的成功率先验；后者以 SCORE_SCALE 为刻度，属于预测 |
| `UNIT_HEAL_THRESHOLD / BASE_UPGRADE_THRESHOLD` | 500 / 500 | P：当前 HP 相对规则最大 HP 的比例门槛 |
| `CHALLENGE_SETUP_ESTIMATE / LLM_STEP_ESTIMATE / TOOL_STEP_ESTIMATE` | 6 / 5 / 5 回合 | P：流程/认知步骤预估，包含 FSM 推进开销；须用实际轨迹校正 |
| `ANSWER_MARGIN / COGNITIVE_RETRY_LIMIT` | 3 回合 / 2 次 | P：答案提交/确认余量；同一求解步骤有限重试 |
| `TREASURE_CONFIDENCE_MIN / TREASURE_ATTEMPT_LIMIT` | 800 / 2 次每场 | P：证据质量门槛和付费尝试上限，失败不能原样重试 |
| `POINT_VALUE / GOLD_VALUE / DEFENSE_VALUE / RISK_PENALTY` | 1000 / 10 / 20 / 2 | P：任务效用系数，定义见 A04 |
| `SWITCH_PENALTY` | 100 | P：任务切换时从效用分子扣除，强制抢占不受其阻止 |
| `ENABLE_OPPONENT_PRESSURE` | false | P：先完成生存和任务闭环，再经对比测试启用 |

其他容量上限（body、日志、队列、上下文、单次寻路扩展数）沿用接口的 Limits，启动时必须配置并验证，不能以无界集合替代。寻路默认最多弹出 `map.width × map.height` 个不同格子，启发式一致时无需反复扩展同一格；到计算预算仍未结束返回 Incomplete。

## 2. 通用转移执行算法

### A01：单回合与单实例推进

1. A 阶段幂等应用新观测；关联上一轮 action/job；保存实际位置、支出、死亡、任务结束等证据，冻结 WorldView。
2. 把事件先写入对应实例 inbox/账本。仅有效 session/owner 的控制消息可执行；旧动作事实仍归档，不随取消删除。
3. B 阶段自下而上处理事实与报告，C 阶段自上而下生成目标、租约申请、计划及动作建议。B/C 每实例最多各一次普通状态切换。
4. 评估当前状态的全部可用边，按下面的优先级和稳定 `TransitionRuleId` 取第一个 guard=true 的边；未知 guard 不视为 true。
5. 若目标与当前态相同，只执行 tick，不重复 on_enter；否则先校验退出约束，产生释放/移交请求，再写新状态并执行入口逻辑。
6. D 阶段整包通过后，草稿处理 CommitReceipt；只为实际提交的动作/作业进入等待态。编码、回调及版本检查均通过才原子提交。
7. 普通状态转移每轮上限遵循 DESIGN 6.0；最多一次紧急重规划。无论预算是否耗尽，硬性死亡、失效与动作合法性都优先抑制新动作。

| 守卫检查顺序 | 处理规则 |
| --- | --- |
| 已发生结果 | 首先把事实与完成证据写入账本；这是更新事实，不受本轮普通转移次数限制 |
| `G01 SessionEnded / G02 WorldInvalid` | 已结束停止新任务；关键世界失效冻结策略并空响应 |
| `G03 OwnerCancelled / G04 ActorDead` | 撤销未来控制；已完成旧成果保留，不给死亡角色新动作 |
| `G05 GoalSatisfied` | 若完成发生在截止/取消前，则按原目标记录成功；终态实例只补归档，不复活 |
| `G06 HardDeadline / G07 PermanentFailure` | 没有此前成功证据时结束该任务/计划 |
| `G08 Emergency / G09 Preemption` | 按任务可恢复性挂起或取消，再安排保全/防御工作 |
| 本状态普通边 | 当前局部条件、结果、资源、路径、等待解除 |
| 可选优化 | 无必要切换则保持当前任务；禁止仅因相同事件重复触发重规划 |

同一个回合内可能“上一动作已完成、当前角色死亡”：先记录完成，角色进 Dead，已完成任务可成功，后续任务不得分配给该角色。尚未发生的预期效果不能抢在死亡/取消前被标记完成。

### A02：守卫函数的精确定义

| Guard ID | 判定规则 | Unknown/false 的处理 |
| --- | --- | --- |
| G01 | 官方结束依据已确认，或最大回合已完成结算；仅收到第 1300 回合请求不满足 | 正常处理最后一轮；不把断流当结束 |
| G02 | 当前 round/map/我方主体信息不可用 | Degraded；缺少单个可选字段只关对应能力 |
| G03 | OwnerPath 任一父代次失效或收到有权取消该 owner 的消息 | 删除未提交候选；旧效果独立对账 |
| G04 | 当前可靠观测确认 actor 死亡；攻击同时检查 controller | 失去视野不是死亡；我方不可确认存活则不给新动作 |
| G05 | GoalPredicate 返回 Satisfied，且证据属于该目标及有效窗口 | Pending 留原态；Unknown 不完成 |
| G06 | 超出 deadline 的合法窗口：inclusive=true 时 `r>at`，false 时 `r≥at`；或裁判超时/结束已确认 | 不知道裁判期限时不捏造 Expired(Challenge)；deadline_kind 分别规定它约束发送还是结果到达 |
| G07 | 目标永久失效且无替代，或同一目标局部重试达到 LOCAL_REPLAN_LIMIT | 暂时资源不足用 Blocked，不能直接永久失败 |
| G08 | A03 的 base_risk ≥ BASE_RISK_ENTER，或关键角色 risk ≥ UNIT_RISK_ENTER，或必要防守配对无法按期完成 | 没有活基地则移除基地风险项，避免对已毁基地持续紧急 |
| G09 | 上级新指令要求抢占，且抢占能解决更高优先级缺口 | 可恢复任务 Suspended，挑战取消并跟踪真实结束 |
| G10 ReturnDue | `next_night_round - current_round ≤ required_ready_eta - current_round + RETURN_MARGIN` | 夜间直接 NightDefend；必要路线未知视为防守缺口 |
| G11 Actionable | 当前 RuleVerdict=Allowed、owner/stamp 有效、所需租约获准、动作槽无冲突 | Unknown/Denied 不发动作，返回具体阻塞原因 |
| G12 ResultSettled | 本 action 的 bool/效果已按 A10 对账，且无矛盾证据 | 等待至 ACTION_RESULT_GRACE，再 OutcomeUnknown，不原样重发消费 |
| G13 WaitReleased | WakeCondition 中指定条件满足，或 deadline 到达、owner 失效、风险升级 | 未满足保持等待，不每 tick 新建同一个请求 |
| G14 ChallengeAdmissible | A08 的任务所有权、期限、位置、风险、返防与预估时间全部通过 | 缺期限时首版不接新的长任务；当前已接任务保留部分答案流程 |
| G15 KnowledgeUsable | 证据引用存在、schema/任务代次匹配、未过期且没有未解决矛盾 | 模型自述 confidence 不能单独满足 |
| G16 Endgame | 剩余可执行回合 ≤ ENDGAME_WINDOW；本场一旦满足保留标记 | Emergency 恢复时也不能回到长期发展政策 |

### A03：时间、风险与返防估计

`distance(a,b)=max(abs(dx),abs(dy))`；到建筑距离取它全部占地格的最小值。以请求处于回合开始时刻 r 为准：本轮 move 结果最早在下一次有效请求确认。`ready_eta(unit, stand)` = 已在合法操作位时 r，否则 `r + path_steps + POSITION_SETUP_ROUNDS`。`required_ready_eta` 是必要控制者最佳可行配对中的最晚 ready_eta。

候选工作准入检查：`r + work_estimate + return_steps + setup + RETURN_MARGIN ≤ next_night_round`；先估到最终工作位置的路线，再估从该位置返防，不能用当前位置返程替代。路径/任务时间 Unknown 不当作零；必须选择短期可控工作或直接返防。

风险采用可复现的保守近似，不声称是裁判精确伤害：

- 对对象 e 及每个可见机器人 j，若 `d(j,e) ≤ attack_range(j) + THREAT_HORIZON × ROBOT_STEP_ESTIMATE`，把其攻击力计入 `potential_dps(e)`；不根据 targetTeam 排除可能撞到我方角色的机器人。
- `risk(e)=min(SCORE_SCALE, ceil(SCORE_SCALE × THREAT_HORIZON × potential_dps(e) / max(current_hp(e), MIN_POSITIVE_HP)))`。
- 机器人眩晕剩余回合未给出时，跨窗口预测不扣去整段攻击；“当前眩晕”作为战术优势记录，不能等同未来三回合无风险。
- 未观察敌人的历史位置只形成额外不确定风险标记；当前范围和路径缓存必须注明哪些值是预测。
- 连续 EMERGENCY_CLEAR_ROUNDS 个**新观测**回合内，基地风险低于退出阈值、关键角色风险低于退出阈值且防守缺口已消除，才退出 Emergency；重复请求不增加稳定计数，`WorldRecovered` 会清零中断前的连续证据并从恢复帧重新累计。

`MIN_POSITIVE_HP` 为分母保护常量；已死对象先被 G04 过滤，不能靠该常量把死者当活人。风险可以保守高估；测得机器人速度/攻击顺序后修改预测模型和参数版本，不改变确认事实。

## 3. 角色分工与能力规则

### 3.1 两名工人和开拓者

从我方 `roleType` 获取角色；工人按 EntityId 排序标记 W1/W2，开拓者标记 P，不写死 `10010/20010`。标签随身份保持到复活，仅作为并列决策的偏好，不代表不同能力或永久职业。

| 角色 | 默认偏好 | 白天行为顺序 | 黄昏/夜间 | 死亡/替补 |
| --- | --- | --- | --- | --- |
| W1：工程优先工人 | 合法建设、升级、维修 | 紧急自救 → 按期防御建设/维修 → 已持券升级 → 已有材料建墙 → 有收益采售 | A07 匹配炮位；按 G10 停止远行；开火/补给/撤退均占一动作 | 释放任务和控制者租约；W2 可接工程任务，不等待 W1 预计复活 |
| W2：经济优先工人 | 采集、运输出售、采购 | 紧急自救 → 必需建设替补/第二施工位 → 已有矿物出售 → A05 采售 → 已批准补给 | 与 W1 同样可操控任意武器，不固定绑定某种炮 | W1 接经济或防御，按实际收益重配；背包不转移给 W1 |
| P：开拓者 | 自进化任务、宝藏、第三操控者 | 紧急自救 → 完成在途挑战 → 可准入高收益挑战 → 证据合格宝藏 → 个人采购/移动/防御准备 | 已批准且可安全守位的挑战可继续；不满足最低防守则放弃并返防；不能边控炮边答题 | 挑战不可由工人接替；已持物品保留到实际复活；工人承担可替代防守 |

硬性工种：仅工人 `collect/build/remove`，仅开拓者 `acceptTask/submitAnswer/summonTreasure`；三人都可 move、买卖、用物品、丢弃与操炮。角色本身没有直接攻击，`Attack` 的 actor 始终是武器。

W1/W2 偏好只在任务等级、期限、效用和行程等指标相同后用于打破平局。若 W2 已拿券且离建筑更近，就由 W2 升级；若首夜必要建设需要两名工人并行，就允许同时施工。任务分配结果优先于标签，禁止双层逻辑各自抢人。

### 3.2 武器、建筑与敌方单位

| 对象 | 本方控制算法 | 不允许的假设 |
| --- | --- | --- |
| 加特林 | A07 分配合法控制者，A09 枚举等级要求数量的落点并检查锥角/弹道 | 不自动射击；不能少填目标绕过等级规则 |
| 电磁炮 | A09 按穿透能量和路径机器人顺序评分 | 不把总能量等同每个目标都获得全伤害 |
| 火箭 | 已知冷却为零才参与 A09；多落点允许叠加 | 不把冷却未知当零；不提前从占用图删除预计击杀对象 |
| 基地 | 用于布局/风险/升级候选；按 2×2 占地建立索引 | 不产生移动/开火命令；已毁基地不能靠药剂/升级券复活 |
| 围墙 | 工人建设/拆除；三人按物品规则修复或升级 | 不替角色分配独立任务 FSM；不能封死唯一出入口 |
| 机器人 | 观测输入、障碍、风险/火力目标，区分清晨清除与击杀 | 不由本方下命令；预测路径不是判题器承诺 |
| 敌方角色/建筑 | 部分可见观测与历史记忆 | 看不见不等于死亡；首版对玩家单位攻击受 U06 验证门槛限制 |

## 4. 可复用决策算法

### A04：候选任务生成、评分与三角色匹配

1. 从当前 StrategicDirective 生成有稳定目标键的候选：`(kind,target,objective_window)`；相同键已有活动任务则更新，不重复新建。
2. 按存活、工种、依赖、期限、物品可达性、规则可用性过滤。目标已经满足则直接记录 SatisfiedBeforeStart，绝不再花钱“完成一次”。非必需的 Q3/Q4 候选仅在 gain 为正时参加分配；Q0/Q1 的强制价值不依赖预估收益。
3. 每人先保留强制保全代表、返防代表和现有可继续任务（同键去重），再按优先类各取最佳代表，最后用效用填充至 CANDIDATES_PER_ROLE；普通经济候选不能挤掉这些保留项。每类代表也用下面的效用/稳定序选择，不能保留无限多个“强制”变体。额外加入 Idle。
4. 枚举三人的候选组合，禁止重复独占目标、同人两任务、不可同时满足的资金/用品、依赖未完成和违反最低防守。多人协作任务必须作为一个组合候选验证，不拆成互不知情的两份。
5. 先最大化各优先类的可满足目标向量，再最大化总效用；同值选择切换更少、总路程更短、符合 W1/W2 默认偏好的配对更多，最后按 `(unit_id, mission_id, target_y, target_x)` 最小的组合。提交整组租约申请，只有获准才派发。

| 任务优先类（从高到低） | 纳入条件 |
| --- | --- |
| Q0 保全 | 立即动作能降低 G08 风险或避免关键角色死亡；不可救的已毁对象不产生无限任务 |
| Q1 必须防御 | G10 返防、首夜缺炮/必要施工、当前必要控制者缺口 |
| Q2 期限内兑现 | 已接挑战的临期提交、即将错过的已确认宝藏窗口；不得违反 Q0/Q1 |
| Q3 常规发展 | 采售、升级、可准入挑战、补给和有价值墙体 |
| Q4 可选探索/施压 | 空闲探索、ENABLE_OPPONENT_PRESSURE 允许的召唤令 |

非硬约束候选的 `gain = POINT_VALUE × expected_points + GOLD_VALUE × net_gold + DEFENSE_VALUE × defense_gain - RISK_PENALTY × risk - switch_cost`，效用为 `gain / max(estimated_rounds, MIN_ACTION_ROUNDS)`。比较分数用有溢出检查的有理数交叉相乘；不依赖浮点近似。`switch_cost` 在确需换任务时为 SWITCH_PENALTY。

`expected_points` 为明确奖励乘成功率估计：同任务族且同 SOP 版本已有样本时用成功次数/已结算尝试次数，否则用 SUCCESS_RATE_PRIOR 并标 Predicted；未知奖励以零参与常规效用，不阻止明确必须的 Q1。金额/积分计算采用定点分子保留精度；最早提交回合未知时不承诺速度奖励。`defense_gain` 以短窗口预期有效防御伤害/避免损失的 HP 估计，未知时为零；首夜缺炮由 Q1 保证，不通过虚构巨大 defense_gain 强行抬分。`net_gold` 扣全部采购/建造支出；风险取工作路线和终点的保守峰值。

每次新观测重新检查硬约束；普通收益变化仅在旧任务可中断且新组合扣除 SWITCH_PENALTY 后更好时换岗。持有挑战会话的 P 不能被普通经济小幅收益抢占。

### A05：采矿、背包与出售

对每个未确认消失、可达且未被可靠停工条件禁止的矿点 m：

1. 求到矿点交互格最短风险路径、到小贩路径及从小贩返防路径；任一必要路径 Unknown/不可达则不作为正常完整采售候选。
2. `n=min(GATHER_BATCH_MAX,个人可用背包格,返防期限可容纳的采集次数)`；已验证矿量不足则进一步限制，矿量未知不能假装还有十份。n 非正则不开始新采集。
3. `cycle_rounds=去矿步数+n+去小贩步数+一次出售动作+必要流程开销`；新周期开销采用 ECONOMY_SETUP_ROUNDS，执行中按尚未完成的状态步骤重估并记录分项；收益使用当前确认矿价乘 n，新闻未来涨价仅作带置信度的次级候选。
4. 按 `(收益/周期 DESC,路线风险 ASC,到矿距离 ASC,矿种原名 ASC,y ASC,x ASC)` 选矿。两个工人可在不同交互格采同矿，不能错误独占整个矿点。
5. 每次采集对账后重算 n 和 G10。背包将满、批次达标、矿点消失/停工、继续采集会误返防时，停止采集并选择出售或直接返防；不能要求必须卖完才回防。

出售按 `(该物品可售总价 DESC,原名 ASC)` 逐种执行，每回合只卖一种指定数量。保留正在执行任务预约的用品和墙体石头；石头保留量是 `min(计划近期墙体缺石量,STONE_RESERVE_TARGET,当前持有量)`，没有墙体计划不机械囤四块。背包“计划会拿到”不算当前可出售数量。

背包满时先出售可售矿物，取消无法装下的新采购。`drop` 只接受显式的腾包任务授权，且物品不属于任何有效预约；按可确认剩余用途价值从低到高选取。用途/售卖能力未知时不能视为垃圾自动丢弃，转 Blocked(Capacity)，让任务层取消低优先采购或改任务。

### A06：开局、建造、升级与补给

**开局**：以基地动态生成的 12 格武器环安排 OPENING_LOADOUT 缺失槽位。对各位置先做连通性过滤：把拟建建筑占地加入障碍后，每座必要武器仍有可达操作位，基地出入口仍有可行通路。按 `(覆盖基地周边可通行攻击入口的格数 DESC,角色总施工步数 ASC,操作位风险 ASC,坐标 ASC)` 为每槽保留 LAYOUT_SITE_LIMIT 个位置，枚举不重叠的槽位组合，对整体布局再做一次连通性/不同操控站位过滤，以同样排序选取。预算耗尽只保留已完整校验的布局。入侵入口未给出时只用基地外侧可通行边界作为代理，不称为真实机器人出生点。

为缺失且准备实施的武器保留 `缺失座数 × WEAPON_BUILD_GOLD`，最多覆盖初始确认金币；两名工人优先并行完成可负担 Q1 施工。可在首夜前完成的第三座随后安排。武器候选只取基地周围 4×4 减 2×2 基地的 12 格；墙候选只取周围 6×6 减内层 4×4 的 20 格，并分别满足金币或个人背包石头约束。

墙体仅在基地 6×6 外环的 20 格中，保持操作位/出口连通；对 A03 纳入威胁的机器人分别求到基地攻击范围的可行路径，比较建墙前后的最短步数增量总和，只计可计算且为正的改善，再按工人距离/坐标打破平局；没有可见威胁/可核验路线时首版不为猜测的入口自动建墙。拆墙仅由任务层在已确认阻断必要通路且拆除能恢复连通时授权，重新比较防御损失；替换炮必须显式授权，并计入降回level1的成本。

升级/补给顺序：

1. 当前角色有 Medicine 且生命比例低于 UNIT_HEAL_THRESHOLD，或 A03 预测下一窗口致死且治疗能改善结果时，生成 Q0 用药候选；不是同回合同时开火。
2. 基地存在、等级可升、持有对应券且生命比例低于 BASE_UPGRADE_THRESHOLD/处于高风险时，优先比较升级回血与其他可救援动作；必须实际能到建筑旁。
3. 常规升级在必要防守建设预留之外支付。候选按 `(短窗口有效火力增量+有风险时可恢复HP)/总金币成本` 排序，比较时仍受 A04 角色时间和预算约束。不能把安全基地满血升级虚构为立即救命收益。
4. 采购必须给最终使用者本人创建任务；按动态商品清单/价格、正数量、空背包格验证。不能设计工人买券再转交 P 的不存在动作。
5. DizzyWeapon/Bomb 用与开火相同的威胁/有效伤害标准比较；能作用于两方机器人，但不能伤害敌方人物/建筑。未知物品用途不自动 use。

### A07：返防、操控者和操作位匹配

1. 取存活武器及其合法邻接站位，不把未知冷却永久排除（冷却影响开火，不影响未来配对）。生成 `(武器,角色,站位,ready_eta,风险,放弃任务成本)` 候选。
2. 首夜默认使全部可用武器有控制者；后续夜晚默认至少控制 `min(两名工人数,存活武器数,存活角色数)` 座。若基地风险触发、两名工人无法覆盖要求，或 P 无可准入高收益任务，则尽可能覆盖全部武器。
3. 枚举至多三炮三人的配对，硬性排除同角色多炮、相同操作格、不可达站位、超出返防期限和非法挑战离场。对需要强制终止挑战的方案单独计放弃代价，由 Q0/Q1 授权。
4. 按 `(按期配对数 DESC,基地威胁方向预期有效火力 DESC,最晚ready_eta ASC,放弃收益 ASC,总步数 ASC,稳定ID序 ASC)` 选择。配对获准后产生 DefendSector 任务和租约。
5. 控制者死亡/武器毁坏时，只重算受影响配对及必要连锁冲突，不把全部角色重新洗牌。控制者需治疗则该轮空炮，其他炮仍可开火。

若必要武器无人可及，则不是发非法远程 attack：上报 AtRisk(DefenseLate)/ResourceNeeded(Controller)，允许减少覆盖、抢占任务或撤退。暂时无炮时角色保全/任务照常进行，不能给基地构造 attack。

### A08：开拓者任务、答案与宝藏准入

挑战候选只来自己方有效任务点，P 存活、无另一个未结束挑战、站位可达。已验证 SOP 估时采用同任务族近期成功步骤的保守上界；没有 SOP 时用 `CHALLENGE_SETUP_ESTIMATE + 预定LLM步骤数×LLM_STEP_ESTIMATE + 工具步骤数×TOOL_STEP_ESTIMATE + ANSWER_MARGIN`。无可界定步骤数则首版不作为有保证完成的任务。

新任务需要估计能够满足可知裁判期限和返防预算，且在点位守位期间不会违反最低防守。未知 timeoutRounds 首版不接新的长任务，等待新规则/配置证据，不擅自把它设为无限。已经进行中的任务期限未知时尽早提交有证据字段，继续观察，不伪造超时。

答案缓存按 `(challenge_generation, schema, canonical_answer_hash)` 去重。字段只有真实工具结果或可核对本地推导支持时才可标“已验证”；可提交部分答案但不得补造未知值。反馈错误而任务仍在进行时，只修正失败/缺失字段，保留更有证据的旧字段；接口没给通过率则不声称知道“最高通过率”。

宝藏候选必须同时有位置、开放窗口、物品多重集的证据；用最弱一项证据质量作为整体置信度（不是 LLM 自报概率），需达 TREASURE_CONFIDENCE_MIN。购买后仍能满足防守预算，行程/开放窗口可达且尝试次数未达上限才准入。每次失败必须有新证据或修正条件才能再试，合法失败用品照常对账。

### A09：火力目标生成、组合与防止过量伤害

1. 先通过 G11 检查本轮可操作的武器/控制者，冷却未知或非零则不生成 attack。
2. 机器人按 `(预计到达基地攻击范围回合 ASC,对我方关键对象的攻击力 DESC,击杀积分/HP DESC,ID ASC)` 排序，保留 FIRE_TARGET_LIMIT 个重点目标；ETA采用 `ceil(max(distance_to_base-attack_range,0)/ROBOT_STEP_ESTIMATE)`，仅作忽略障碍的紧迫程度估计。紧迫威胁指 ETA ≤ THREAT_HORIZON；无存活基地时改以当前被保护角色为对象。所有机器人仍参与射线路径和溅射计算，不能从弹道中删除非重点机器人。
3. **加特林**：候选为范围内重点目标坐标；枚举数量等于等级的组合，任意方向向量点积均非负，且方向不为零。按裁判确认的弹道判定最近机器人，每弹至多 10 伤害。首版不假定重复同坐标合法；合法候选不足时不发送少目标攻击，转 Hold/报告规则或候选限制。
4. **电磁炮**：每个候选单落点按射线排序机器人；沿途累计实际扣能伤害，每个实体消耗 `min(剩余能量,它当前HP)`，到终点/能量耗尽停止。非机器人遮挡及线段穿格依 U06，未确认时该具体射击方案不可宣称已验证。
5. **火箭**：候选落点取重点机器人及其八邻域内、地图和射程合法的格子；去重后按预计覆盖伤害保留 FIRE_TARGET_LIMIT 个，枚举可重复落点的等级长度组合。中心/溅射按规则累计，重叠允许叠加。
6. 每炮按下述比较序保留 FIRE_OPTIONS_PER_WEAPON 个最佳合法方案和不攻击；枚举炮间组合，求每个机器人的联合伤害 `D_i`。每个目标有效伤害为 `min(D_i,HP_i)`、过量为 `max(D_i-HP_i,0)`；比较 `(对被保护对象紧迫威胁的有效伤害 DESC,可确认模型下的预期击杀积分 DESC,总有效伤害 DESC,过量伤害 ASC,稳定落点序 ASC)`。
7. 射线穿透/阻挡按同一回合开始 HP/位置评估，不因另一炮预计击杀就重写占用或提前减少穿透消耗；裁判细节不明时联合击杀标为估计。最终输出仍经整包校验。

实现分开 `target_candidates`、`gatling_options`、`railgun_options`、`rocket_options`、`combine_fire`，不得在一个大 match/多层循环函数中包办。每个枚举步骤显式消耗候选预算，预算用尽只保留完整合法组合。

### A10：动作结果确认与有限等待

| 动作 | 确认效果所需证据 | 证据不足时 |
| --- | --- | --- |
| move | 下一观测中同角色位置等于目标，且无归属矛盾 | 位置未变且回执失败：失败；缺角色/跳帧/矛盾：Unknown，不声称成功 |
| collect/buy/sell/drop | 个人背包对应数量变化；涉及金币还核对总账一致性 | 金币可能同时受任务奖励影响，不把总差额全部归给一人；只确认可唯一归属部分 |
| build/remove | 对应目标格和类型的己方建筑出现/消失，与动作/回执一致 | 因其他事件同时毁坏无法归因时 Unknown，重新检查目标是否仍需执行 |
| use | 用品消耗及对应治疗/升级/眩晕等可观察变化；效果不可见时可记录执行回执而非收益 | 不因没看到增益就立即再买/再用；升级可能同时受伤，不能要求必须观察满血 |
| attack | 可归属的真回执确认本轮操作；HP/消失另作效果记录 | 不把机器人清晨清除当己方击杀，不因回执真宣称指定目标已死 |
| acceptTask/submitAnswer | 当前挑战原文/会话及错误/结束变化，匹配角色和发送回合 | 原文非空不等于新接取成功；提交无错误不等于全对 |
| summonTreasure | 与本次发送相关的结果码及背包消耗 | 码 0/无结果/跳帧为待核对；禁止消费型盲重试 |

每个 ActionId 恰好关联一个本地提交记录；超过 ACTION_RESULT_GRACE 且仍无可靠结果，发布 ActionOutcomeUnknown，释放本轮动作槽但保留未决消费历史。后续根据当前存量重建可用量，不能把“未知”同时当成功增益和失败退款。

### A11：路径、等待、撤退和阻塞恢复

八邻域 A* 的每步成本为 `PATH_STEP_COST + PATH_RISK_WEIGHT×目的格风险 + PATH_CROWD_WEIGHT×目的格相邻己方其他角色数`，所有附加项非负；目的格风险用 A03 将本角色投影到该格计算。启发式为到目标站位集合的最小切比雪夫距离乘 PATH_STEP_COST。弹出顺序 `(f,h,y,x,insertion_sequence)`，邻居固定坐标序，允许规则规定的对角线穿行。动态障碍按当前占位保守处理，不踏入其他角色当前格、不互换。

仅对已经提交 move 且下一观测无进展累计 stall；达到 STALL_LIMIT 使路径版本失效，重新计算交互站位与路线。目标/任务不变时最多 LOCAL_REPLAN_LIMIT 次，真实路径进展才清零。重规划不能靠反复新建 PlanId 绕过计数，计数归 `(mission,target)`。

等待对象必须记录类型化 WakeCondition 及 deadline；包括冷却归零、资源批准、作业结果、窗口开放、格子腾空、目标可用、新证据，字段定义见 INTERFACES 2.4；每 tick 只查询该条件及全局守卫。普通 Blocked 每 BLOCKED_RECHECK_INTERVAL 回合复查，到 BLOCKED_MAX_ROUNDS 仍无进展则释放非必要人员并升级报告；挑战守位不套用“自动离开”的普通等待规则。

撤退枚举当前可行一步及有界安全目的地，按 `(死亡风险 ASC,当前位置到目的地累计风险 ASC,离威胁距离 DESC,到己方可防守区域距离 ASC,y,x)` 选择；没有比当前更好的可行移动时考虑已持药剂/炸弹等合法救援或 Hold，不能为了“撤退”踏入更坏格。逃生意图仍需 owner 授权和整包仲裁。

### A12：事件/上报与去重

世界变化在当前快照与上个有效快照比较后发布；跳帧只能标记区间变化，不能捏造中间事件顺序。完成/失败报告按 `(owner,scope,status,reason,evidence_hash)` 恰好应用一次，进展只有观测发生变化才重发。已提交动作的回执即使 owner 失效也要形成原路径的事实报告，但 `is_current` 失败时不得修改替代任务、计划或个体状态。

资源不足上报必须带缺多少、给谁用、何时需要；父层先尝试已存在同目标前置任务，不重复建采购任务。紧急报告必须附 A03 风险或 G10 迟到证据，不能因为子模块泛化 Failed 就让全队进入 Emergency。

## 5. 世界和战略：逐状态处理规格

表中的算法与守卫引用上文 A/G 编号。每行是一个状态处理器的实现责任；箭头条件按从左到右顺序评估，优先执行 A01 全局守卫。未列出的条件保持原态；不允许靠默认分支跳到其他状态。前缀 W/S/M/T/I/C/J/N/B 分别代表世界、战略、任务、战术、个体、挑战、认知作业、新闻、宝藏，避免同名状态混淆。

### 5.1 世界 W：更新事实，不选择动作

| 状态 | 必须执行的计算/输出 | 切换规则与清理 |
| --- | --- | --- |
| `W.ColdStart` | 校验 round/map/我方实体，规范化阵营、footprint、工种；创建版本和索引；没有合法快照时输出能力禁用原因 | 完整首帧 → Ready；关键字段错误 → Degraded；G01 → Closed。可选字段 Unknown 不阻止 Ready |
| `W.Ready` | A10 对账 → 实体/背包/价格/会话索引更新 → A12 差分事件 → 冻结快照；同 RequestKey 只应用一次 | G01 → Closed；G02 → Degraded；其余保持。新新闻只入原文仓，不当成当前价格事实 |
| `W.Degraded` | 保存最后有效历史但禁止用旧位置发动作；保留可可靠归属的实际回执；不猜补关键字段 | 通过完整重新校验的新帧 → Ready，发 WorldRecovered、失效旧路径/建议；G01 → Closed；否则返回兜底 |
| `W.Closed` | 归档结果及已提交动作，停止新分配/认知调用；同场迟到数据仅作对账 | 本 session 不再退出；经 IF02 确认新比赛才新建 ColdStart，不能因 round 变小自行清空 |

实体投影不是另一套行动调度器：可见且 HP 正为 ObservedAlive；明确死亡证据为 ConfirmedDead；普通敌方角色离开局部视野为 Unobserved；全图可见的敌方基地/墙体从后续快照消失为 Removed，确认已拆除/清晨清理也为 Removed，并记录原因。消失本身不算己方击杀。Unobserved 再观测到恢复 ObservedAlive；ConfirmedDead 仅明确复活可恢复；Removed 的新矿点即使同坐标也建新实例。武器 cooldown Unknown 不能投影为 Ready，矿点消失不能推断谁采走最后一件。

### 5.2 战略 S：每轮产生一份 Directive

正常态选择函数按 `G16 → Endgame；夜间 → NightDefend；G10 → PrepareNight；否则 → DayDevelop` 排序。Emergency 在正常态之外抢占；Endgame 是政策状态，内部仍计算昼夜并遵守建造/攻击限制，不另建一个会冲突的战略 FSM。

| 状态 | 必须执行的计算/输出 | 切换规则与清理 |
| --- | --- | --- |
| `S.Bootstrap` | W.Ready 后创建角色名册、规则能力开关、初始目标；A06 检查首夜建造；不依赖假定角色 ID/初始站位 | G01 → Finished；G08 → Emergency；初始化完成 → 正常态选择函数。W 非 Ready 冻结，不能重复生成初始任务 |
| `S.DayDevelop` | A03/A07 先算返防约束；A06 安排必要建设；A04 分配剩余人员做 A05/A08；保留必要建设资金 | G08 → Emergency；G16 → Endgame；夜间 → NightDefend；G10 → PrepareNight。切换前撤销不满足新期限的工作 |
| `S.PrepareNight` | 冻结新远程采矿/长挑战；按 A07 匹配武器、控制者和站位；短任务只有证明完成后仍准时才准入；有在途挑战按缺口决定守位或取消 | G08 → Emergency；G16 → Endgame；夜间 → NightDefend；白天且防守需求已消失/新白天 → DayDevelop；其余保持，避免 ETA 小变动来回切换 |
| `S.NightDefend` | 每新帧重算 A03、A07、A09；缺控制者先重配；无威胁仍保留所需控制者，允许合法补给；不发布 build | G08 → Emergency；G16 → Endgame；新白天 → DayDevelop。只有夜晚结束/目标撤销才能结束整夜 Defense，不因一次击杀成功 |
| `S.Emergency` | 记录原因集合和进入回合；Q0 保全、Q1 补防；取消不可恢复挑战，挂起可恢复经济；生成有证据的用药/升级/炸弹/撤退需求 | G01 → Finished；A03 退出条件连续满足 EMERGENCY_CLEAR_ROUNDS → 正常态选择函数；新风险清零稳定计数。己方基地已毁时移除该风险原因，仍照顾存活角色 |
| `S.Endgame` | 保留昼夜子政策；只接受预计收益在最后可执行回合前兑现的工作；停止无法回本的新长期 SOP/屯货/工程；已有药/券按当前收益使用，未知剩余奖励不制造收益 | G08 → Emergency（保留 endgame 标记）；G01 → Finished；否则保持。第 MATCH_ROUNDS 回合仍生成合法动作 |
| `S.Finished` | 撤销未来目标、租约和认知作业；归档分数/原因；不新增动作 | 终态；断流、己方基地单独被毁不满足入口依据 |

## 6. 任务层 M：从候选到成果的处理规格

每个 MissionSpec 固定 `goal、target、window、dependency、interruptibility、priority_class`。采集/建设等流程节点见 DESIGN 6.2.1，但节点调度必须使用 A04–A08；Defense 的 goal 是约定防守窗口结束且控制责任已解除，不能用“到达炮位”替代。

| 状态 | 必须执行的计算/输出 | 切换规则与清理 |
| --- | --- | --- |
| `M.Proposed` | A04 去重和能力过滤，计算 gain/rounds/risk/dependencies；目标已完成则记 SatisfiedBeforeStart 并丢弃候选，不再建运行实例 | 前置齐全且有价值 → Ready；暂缺前置保持并建立唯一依赖；永久不可行 → Failed；目标被撤销 → Cancelled；窗口过期 → Expired |
| `M.Ready` | 参加 A04 全队枚举；申请角色、武器、站位及数量资源；依赖/预算发生变化必须重新计算 | 全部必要租约原子批准 → Assigned；目标暂不可用/资源长期短缺 → Blocked；当轮竞争落选保持 Ready，不上报执行失败 |
| `M.Assigned` | 只发一次含完整 owner 的 MissionAssigned；保存握手期限；接收战术发出的 MissionActivated | 对应计划已接收 → Executing；超过 ASSIGNMENT_ACK_GRACE 或分配资源失效 → Blocked；死亡可换人则 Blocked，绑定 P 的挑战死亡则 Failed |
| `M.Executing` | 根据真实 Step/Plan 报告推进 DAG；A10 复验目标，A03 检查余量，A12 合并需求；子计划完成仅解锁下一节点 | G05 → Succeeded；可恢复抢占 → Suspended；不可恢复抢占 → Cancelled；暂缺路径/物资/替补 → Blocked；G06/G07 → Expired/Failed |
| `M.Blocked` | 保存类型化 WakeCondition、blocked_since/recheck_at；A11 复查，合并前置需求；释放无用站位/预算，必要挑战守位单独保留 | G05 → Succeeded；解除且前置齐 → Ready；可恢复抢占 → Suspended；超过 BLOCKED_MAX_ROUNDS 释放普通人员，仍有前置机会可继续无占人等待；无机会/重试耗尽 → Failed；G06 → Expired |
| `M.Suspended` | 保存流程节点/已证实成果和已持物品；取消旧计划权限、释放控制者/站位，消费账本不回滚 | G05 → Succeeded；新政策允许且 A04/A03 仍准入 → Ready（新计划代次）；目标失效 → Failed；G06 → Expired；不能让挑战离开后恢复旧题 |
| `M.Succeeded` | 一次性输出 Mission 完成证据/实际收益，解锁依赖；释放未使用租约，归档实际结果 | 终态；迟到效果只补账，不重复发布成功或奖励 |
| `M.Failed` | 保存 reason、重试次数、部分成果；取消子计划/认知权限，向上报替代需求 | 终态；替代目标必须由父层创建新任务，不自动反复重建同一失败目标 |
| `M.Cancelled` | 保存取消主体和生效回合；使所有后代候选失效、释放未来资源 | 终态；保留已发送动作/挑战会话观测，不能把内部取消写成裁判已结束 |
| `M.Expired` | 按 deadline_kind 记录内部窗口过期或已证实裁判超时；停止新动作，保存部分成果 | 终态；截止前已完成但迟到的证据补记成果，不复活计划；不可假称比赛挑战已超时 |

所有非终态适用 G01/G03 的取消传播；W.Degraded 冻结执行而非批量 Failed。任务死亡处理依可替代性决定，不把所有 UnitDied 一律映射 Failed。Assigned 的握手只用 INTERFACES 中的 MissionActivated，不新增同义事件。

当前不可变 MissionSpec 已固定 GoalPredicate、target、dependencies、required capabilities、MissionDeadline、Q0–Q4 priority、interruptibility 和 RetryPolicy，完整契约变化会更换 assignment。`MissionRegistry::reconcile` 已按 G05/G06 处理建设实体、ChallengeEnded、白昼 PhaseChanged 与 AllOf 证据，保存证据来源回合；Succeeded 解锁依赖，期限未满足时根任务 Expired、后代 Cancelled。EffectObservation/InternalPlanning 拒绝迟到证据；ActionSubmission 仅由匹配目标的实际提交满足 checkpoint，按时提交后可等待效果。实时策略仍只创建无依赖、无 deadline 任务；经济周期证据、能力过滤、自动 DAG/期限生成、Failed/Suspended、租约和完整进展字段仍须按本表补齐。

## 7. 战术 T 与个体 I：计划、动作和恢复

### 7.1 战术状态

| 状态 | 必须执行的计算/输出 | 切换规则与清理 |
| --- | --- | --- |
| `T.Plan` | 选择当前 DAG 节点；A11 枚举合法终点和路径，操炮调用 A07；输出租约申请，等待批准后才能派 Intent | 已到目标邻域 → Position（统一复验站位）；需要路径 → Approach；暂不可行 → Hold；G07 → Failed |
| `T.Approach` | 基于当前快照检查下一格，只发 Move 意图；预测最终站位和 ETA；A11 累计已提交移动的 stall | 观测到终点邻域 → Position；路径变更/stall 达限 → Replan；临时占格 → Hold；G08 且上级授权逃生 → Retreat；不得在移动建议发出时认为已到位 |
| `T.Position` | 复验交互距离、控制者身份、朝向/落点约束；最多提出一次修正移动，更新站位租约 | 实际位置满足交互且租约齐 → Execute；新障碍或站位失效 → Replan；资源待批 → Hold |
| `T.Execute` | 按节点产生一个 Interact/OperateWeapon/保持任务意图；开火用 A09；提交后保留 action_id，不直接完成节点 | 对应动作实际提交 → Evaluate；无目标/冷却/等认知 → Hold；提交前条件失效 → Replan；本地未获选则保持，不创建虚假在途 action |
| `T.Evaluate` | 读 A10 对账和 Step 报告；核对节点目标；采矿检查 A05 批量/返防，防守检查窗口而非击杀 | 节点及本 Plan 目标都满足 → Completed；仍需同节点动作 → Execute；需新路线/节点 → Replan；结果仍未决保持；结果 Unknown/失败 → Replan 或 Hold（按可恢复原因） |
| `T.Replan` | 取消旧未提交 Intent，增加 plan generation；按 `(mission,target)` 保留 A11 重试计数；重新算目标/站位/路径并请求资源 | 找到可行替代且需移动 → Approach；已经在邻域 → Position；暂时不可行 → Hold；达到重试上限/目标永久消失 → Failed；成功移动等真实进展后才清零计数 |
| `T.Hold` | 保存 ResumeReason、WakeCondition、deadline；无动作时不累计 stall；冷却/认知等待继续守位，普通堵路可释放临时格 | 冷却/目标/认知可用且位置有效 → Execute；邻域需要复验 → Position；路径/资源变化需要新规划 → Replan；条件未解除保持；G06/G07 → Failed；授权撤退 → Retreat |
| `T.Retreat` | 用 A11 生成降低风险的合法一步；先核对药剂/救援动作是否占本轮角色槽；向父层上报逃生 ETA 和被中断工作 | 观测到安全站位且 risk < UNIT_RISK_EXIT → Replan（仍有有效任务）或 Cancelled；没有更安全步骤 → Hold 并报告风险；角色死亡 → Failed；新任务接手则旧计划 Cancelled |
| `T.Completed` | 发一次 Plan 级完成报告及目标证据，释放局部租约；父任务自行复验 Mission 目标 | 终态；不能把 Plan 完成直接当父 Mission 完成 |
| `T.Failed` | 发一次带原因、已试路线/次数、阻塞位置的报告；撤销子意图，保留已发送效果 | 终态；可替补的控制者死亡可使父 Mission Blocked 后创建新计划 |
| `T.Cancelled` | 使全部子意图 generation 失效；释放局部未来预约，记录取消来源 | 终态；不撤销已经发生的动作事实 |

执行中的 T 统一在 G03 → Cancelled、G04 → Failed、G05 → Completed；因此纯移动/逃生计划可以观测到位直接完成。G06 表示该计划失效并报告 Deadline，不创建未定义的 T.Expired。跨节点的正常 Replan 不累计“失败重试”，只有无进展/失败引发的重规划累计 A11 计数。Retreat 不能用失效的旧 owner 发动作：父任务已取消时，由新的 Escape 任务/计划持有逃生授权。

### 7.2 个体状态

| 状态 | 必须执行的计算/输出 | 切换规则与清理 |
| --- | --- | --- |
| `I.Idle` | 注册存活可派遣能力；无 Intent 不自主采矿或抢炮；核对在途记录后接收授权意图 | 合法移动/交互/操炮 Intent → Moving/Interacting/OperatingWeapon；确认挑战守位且作业等待 → WaitingResult(CognitiveHold)；G04 → Dead |
| `I.Moving` | 检查路径版本/下一格/邻接/预约，编译单步 move；不在个体层重做全队路径 | D 阶段真提交 → WaitingResult(ActionAck)；已在目标并有新交互/操炮 Intent → 对应执行态；位置失配/风险 → Recovering；取消且无在途 → Idle |
| `I.Interacting` | 对指定技能逐项校验工种、目标、数量、位置、物资；只产生一个 Proposal；答案/献祭版本须与 owner 匹配 | 真提交 → WaitingResult(ActionAck)；纯认知保持意图 → WaitingResult(CognitiveHold)；需移动且获新意图 → Moving；本地淘汰保持并报告，不冒充裁判拒绝 |
| `I.OperatingWeapon` | 校验所分配武器/控制者/站位；采用 T 给定火力方案并再做规则检查；冷却/无目标可本轮无建议 | 真提交攻击 → WaitingResult(ActionAck)，actor 为武器且绑定本人；站位失配 → Recovering；获新合法意图 → 对应状态；不自行切炮 |
| `I.WaitingResult` | ActionAck：按 action_id 做 A10；CognitiveHold：按 job/挑战代次等有效输入并守位；两类上下文不混用 | ActionAck 已确认且意图有效 → 原执行态，否则 Idle；拒绝/Unknown → Recovering；CognitiveHold 结果/新指令 → Interacting 或 Idle；G04 → Dead；取消时保留 pending 对账，禁止旧动作重发 |
| `I.Recovering` | 撤销失效建议、刷新实际位置/背包/能力，报告失配；不自主重购/献祭；等待 T 的新路径/逃生/交互意图 | 存活且新意图有效、无未处理回执 → 对应执行态；无任务且核对完成 → Idle；仍有影响当前动作合法性的未知结果保持；G04 → Dead |
| `I.Dead` | 清理未来控制/站位与建议，发一次 UnitUnavailable；保留背包历史及已提交记录，不自行计算复活完成 | 可靠 UnitRevived → Recovering；预计复活时刻/不再可见均不触发；终场仅归档 |

“仍有未处理回执”不代表冻结角色到比赛结束：ACTION_RESULT_GRACE 后转 Unknown，已发送消费保持账本墓碑；在可靠新快照下可执行无重复消费风险的新意图。死亡/取消/未知结果均不得删除这个墓碑。

## 8. 认知与高收益任务：逐状态处理规格

### 8.1 挑战 C

所有非终态：裁判明确结束 → Learn；G03/G04 或不可避免撤离 → Aborted；结束前已有可靠完成证据先归档。deadline near 定义为剩余合法提交回合 ≤ ANSWER_MARGIN，此时有新版本的可提交证据就优先 Submit，无可提交证据则停止新增长作业；明确无法完成必要步骤才 Aborted。内部时限不是裁判会话结束证据。

| 状态 | 必须执行的计算/输出 | 切换规则与清理 |
| --- | --- | --- |
| `C.ApproachPoint` | A08/G14 准入，绑定 P/己方点位和任务窗口，交由 T 寻路；每新帧重估返防 | 实际合法邻接且无别的实际会话 → AcceptPending；点位暂不可用保持至内部截止；永久失效 → Aborted |
| `C.AcceptPending` | 同 generation 只允许一条在途 accept；未发送前每轮再验 G14；发送后保持邻域并按 A10 查会话关联 | 可靠 ChallengeStarted → ReadTask；明确失败且仍可准入 → ApproachPoint；超过结果宽限仍无法关联 → Aborted，保留会话对账；本地未获选保持 |
| `C.ReadTask` | 固定题文/hash、accepted_round、可用工具、deadline Knowledge；解析输入/输出格式，不能用上一题补空字段 | 最小题意完整 → RetrieveSop；缺字段有限等待到 JOB_RESULT_GRACE，仍缺 → Aborted；未知期限下限制为可快速产生证据的步骤 |
| `C.RetrieveSop` | 按任务族/格式/工具能力精确匹配 SOP 前提，绑定模板版本和本题参数；只复用方法不复用旧答案 | 命中且下一步有合法命令 → ExecuteSandbox；已有本题答案证据 → Submit；否则 Plan |
| `C.Plan` | 本地解析优先；创建一个依赖明确的 NextStep，记录预计时间/所需证据；需模型时申请单个 Job 和通道 | 本地命令可执行 → ExecuteSandbox；有答案证据 → Submit；D 真提交 LLM → AwaitLlm；暂未批准保持；无可行步骤/重试耗尽 → Aborted |
| `C.AwaitLlm` | 保持邻域，等待对应已验证 Job；模型输出只允许 typed NextStep/AnswerCandidate，不能直接成为 RoleCommand | 有证据命令 → ExecuteSandbox；本题证据支持的答案 → Submit；缺证据/可修复错误且未超 COGNITIVE_RETRY_LIMIT → Plan；否则 Aborted 或已有部分答案时 Submit |
| `C.ExecuteSandbox` | 检查任务实际仍在进行、工具/命令范围、长度、输入引用、deadline；同通道不能并发重复发送 | D 真提交 executeCmd → AwaitSandbox；本地落选保持；前提改变可修复 → Plan；工具不可用且无替代 → Aborted/部分答案 Submit |
| `C.AwaitSandbox` | 等本 Job 的结果，工人照常行动；不因空输出重放可能修改文件的命令 | 已归属结果 → InspectResult；作业失败/超时且允许新诊断步骤 → Plan；仅剩提交余量 → Submit（须有证据）；否则 Aborted |
| `C.InspectResult` | 分别处理 exitCode、TIMEOUT、JUDGER_ERROR、TRUNCATED；解析结构并关联输入；保留可信部分，不能因 exitCode=0 就判答案正确 | 足够/截止前部分答案 → Submit；明确下一工具步骤 → ExecuteSandbox；需修正推理 → Plan；无有效后续且重试耗尽 → Aborted |
| `C.Submit` | 格式化本题答案，计算 answer_hash；只有新证据导致版本变化或明确可重试拒绝，才允许再次提交；申请 P 动作槽 | D 真提交 submitAnswer → AwaitFeedback；本地落选保持；会话已结束 → Learn；无合法答案/内部截止 → Aborted |
| `C.AwaitFeedback` | 按发送回合/版本检查 errors、任务原文、结束和奖励；有反馈但未结束不能谎称全对 | 可靠结束 → Learn；明确错误或需要补全且有时间 → Plan；宽限后反馈未知 → Plan 做证据诊断，禁止同答案原样重发；无后续机会 → Aborted |
| `C.Learn` | 本地归档步骤、题型、结果和证据；仅把已验证方法标 Verified，未知得分记录 Unknown；更新版本化 SOP，不占用任务期已结束后的免额 | 归档一次完成 → Done；不自动再发 LLM 总结 |
| `C.Done` | 输出一次挑战流程结果，M 依自己目标核对成功/部分完成；释放守位和认知权限 | 终态；当前 challenge generation 不复用 |
| `C.Aborted` | 记录内部放弃原因、部分答案和费用；取消后代 Job/未来动作；取消不等于裁判会话已经结束 | 终态；world 继续跟踪真实 ChallengeEnded；晚到成果只归档，不能重新发送答案 |

COGNITIVE_RETRY_LIMIT 按 `(challenge_generation, step_kind, input_hash)` 统计失败后的重做次数，不随 JobId 改变清零；只有新增有效证据/明确进入下一步骤才换计数键。没有变化的模型自我改写不算新证据。

### 8.2 作业 J：LLM 与沙盒共用生命周期

| 状态 | 必须执行的计算/输出 | 切换规则与清理 |
| --- | --- | --- |
| `J.Queued` | 检查 owner/任务范围、输入hash、通道/额度、deadline；只生成认知候选，不先扣实际额度 | D 的 CognitiveJobCommitted → Sent；G03 → Cancelled；过期 → TimedOut；无额度保持到截止，本地淘汰不算 Sent |
| `J.Sent` | 固化发送回合、原 stamp、quota claim 和关联规则；通道登记 pending，额度记在途 | 下一 B → AwaitingResult；同时到达的原始结果先入持久 inbox，不能因当前态不是 AwaitingResult 丢弃；取消 → Cancelled |
| `J.AwaitingResult` | 优先归属已有结果，再验 schema/证据/代次；无 job ID 时仅匹配已确认的协议时间关系和唯一在途对象 | 有效 → Validated；明确错误/结构不可用 → Failed；JOB_RESULT_GRACE 后仍无可归属结果 → TimedOut；G03 → Cancelled。符合 deadline.inclusive 的窗口内结果先验证，截止后结果只归档 |
| `J.Validated` | 保存 typed payload、EvidenceRef、输出hash；将结果送指定父流程；预测仓更新排下一 A，不能写冻结快照 | 消费者确认应用且 owner 有效 → Applied；owner 失效 → Cancelled；过期不可用 → TimedOut；接收方尚未推进则保持，不重复应用 |
| `J.Applied` | 标记应用键 `(job_id,consumer,output_hash)`，释放通道预约，归档成本/结果 | 终态；保留发送墓碑用于重复/迟到隔离 |
| `J.Failed` | 保存解析/工具/证据错误，通知父流程选择后续；释放未来资源，已经发送额度不退款 | 终态；重试必须由父流程新建有预算的新 Job |
| `J.TimedOut` | 保存发送/截止/已收到片段；通知父流程；将无法确认是否仍运行的通道标 Quarantined | 终态；迟到结果只归档，只有能排除串用才重新开放通道 |
| `J.Cancelled` | 使输出控制权限失效，保留已发送作业墓碑；未发送预约可释放，已发送继续对账 | 终态；任务取消不保证外部计算停止，也不能立即把下次结果绑定新任务 |

Job 的 ResultGrace 按不同的新观测次数计算，不按重复 HTTP 请求计算；若跳过回合且协议只保证“下一回合”，相关性变 Unknown 并隔离，不能拿最新文本补成确定结果。Challenge 可在自己的 B/C 阶段消费 J 已验证结果；仍必须遵守每实例 A01 预算，不能把整个子流程循环跑完。

### 8.3 新闻 N 与证据质量

证据质量是可解释分级，不是统计概率：当前协议直接给出的相应事实/可复核工具结果为 `EVIDENCE_DIRECT=1000`；两个独立且一致的明确线索为 `EVIDENCE_CORROBORATED=900`；单条明确原文且可确定解析为 `EVIDENCE_EXPLICIT=800`；歧义/推断为 `EVIDENCE_AMBIGUOUS=500`；矛盾或无来源为 `EVIDENCE_INVALID=0`。这些都是 P 参数；重复转载/同源模型复述不算独立来源。矿价仍以当前观察为准，传闻评级不能提升成世界事实。宝藏地点、时间、物品分别打分，候选总体取最小值；任一项缺失/矛盾即不准献祭。

| 状态 | 必须执行的计算/输出 | 切换规则与清理 |
| --- | --- | --- |
| `N.Collect` | 按 `(day,channel,text_hash)` 保存原文/来源与首次看到回合；同条只建一次流程 | 非空新条目 → Extract；重复不新建，旧实例继续；无内容 → Rejected |
| `N.Extract` | 优先固定格式/日期/矿种词表解析；无法确定时申请受每日额度约束的 LLM 作业，保留 evidence spans | 有结构化候选 → Validate；预算/额度不足 → Deferred；不可解析且无后续价值 → Rejected；已发送作业仍未结果则保持，不重发 |
| `N.Validate` | 核对原文引用、实体/坐标范围、相对日期基准、时间先后、物品多重集和矛盾；赋质量等级 | 字段合格且无矛盾 → Publish；缺上下文可补 → Deferred；伪证据/非法字段 → Rejected；不能用模型自报置信度替代检查 |
| `N.Publish` | 产生带 sources/valid_until 的 ValidatedHypothesis；按 news/version 幂等写记忆，下一 A 更新共享预测并发布 HypothesisUpdated | 本条保持归档完成标记，不再输出；修订输入新建版本，不把本条重设 Collect |
| `N.Deferred` | 记录 MissingEvidence/NoQuota/Budget 和唤醒条件、相关窗口；宝藏原文长期保存，失效经济预测停止应用 | 新证据/新日额度且仍有价值 → Extract；窗口失效/确定无价值 → Rejected；否则保持，无每轮盲调 LLM |
| `N.Rejected` | 保存原文与拒绝原因，停止发布；新证据可以创建新版本供后续重新判断 | 本版本终态，禁止改写旧原文使其看似通过验证 |

### 8.4 宝藏 B

尝试计数只在实际提交 summonTreasure 后增加；本地拒绝不算付费尝试。总体证据质量满足门槛只是候选资格，还须通过 A04 资源/时间/防御匹配。地图唯一宝藏的成功/已空证据会撤销全部同场竞争候选。

| 状态 | 必须执行的计算/输出 | 切换规则与清理 |
| --- | --- | --- |
| `B.CollectClues` | 汇总跨日原文/验证线索，按来源去重；保留相互矛盾分支，不先买物品 | 有足以限定地点/窗口/多重集的线索 → Infer；无新证据保持；G01/已空 → Abandoned |
| `B.Infer` | 将有界候选按 `(最低证据质量 DESC,总用品成本 ASC,最早可到时间 ASC,稳定候选ID)` 排序；推断仅生成 Hypothesis | 候选齐全 → Validate；缺关键维度 → CollectClues；所有证据矛盾且无可补来源 → Abandoned |
| `B.Validate` | 校验 G15、质量、窗口、attempt上限、个人背包容量；A08 计算采购+行走+召唤+返防；A04 比较机会成本 | 已准入 → AcquireSupplies；缺证据 → CollectClues；候选错误可替代 → Reassess；无预算/时间/尝试次数 → Abandoned |
| `B.AcquireSupplies` | P 自己购买缺少的多重集数量；先核对已有与在途物品；一次采购后 A10 确认，不能从工人背包借用 | 全部用品已观察持有 → Travel；价格/容量/窗口变化 → Reassess；永久买不到 → Abandoned |
| `B.Travel` | T 规划到候选相邻可交互格；途中检查窗口、风险、已空消息；用品不提前扣减 | 实际到位且窗口已开 → Summon；提前到位 → WaitWindow；路径/时间改变 → Reassess；强制返防 → Abandoned |
| `B.WaitWindow` | P 保持安全合法站位，WakeCondition=WindowOpened；等待成本仍计入防守准入 | 窗口已开且证据/用品仍合格 → Summon；窗口已错过/危险/新增矛盾 → Reassess；没有可靠开窗条件不得靠定时猜测献祭 |
| `B.Summon` | 再验实际背包多重集、位置和 attempt限额；一个 candidate_version 只建一个在途动作；申请 P 动作槽 | D 真提交 → AwaitResult；本地落选保持；条件变化 → Reassess；候选版本不能靠无实质更改绕过去重 |
| `B.AwaitResult` | A10 关联结果与消耗；用品结果不明仍保留 pending；不发送第二次献祭 | 码1 → Completed；码4 → Abandoned；码2/3 → Reassess；码0/无结果保持至宽限后 Reassess(Unknown)，禁止据此认定没消耗 |
| `B.Reassess` | 码2 同时保留错地点/未开放两种解释；码3审查用品/数量证据；Unknown核对当前存量；只允许新证据改变候选 | 新证据形成可行新版本且 attempts 未耗尽 → Validate；没有新证据但窗口仍可等待 → CollectClues；已空/次数耗尽/无可行窗口 → Abandoned |
| `B.Completed` | 归档成功证据及实际收益，取消同场其他宝藏候选，释放未来预约 | 终态；不重复奖励/献祭 |
| `B.Abandoned` | 归档放弃原因与实际用品损失；未用物品仍属于 P；回收未来租约，把 P 交回任务分配 | 终态；内部放弃不伪造宝藏已被别人取走；新信息可形成新候选但继承全场尝试计数 |

## 9. 可追踪的切换过程与数值用例

### 9.1 一次切换必须产生什么

每条允许边注册稳定 `TransitionRuleId={machine,from,trigger_class,guard_id,to}`；相同集合多条边需显式符号后缀，不用源文件行号。handler 分为 `evaluate`（纯计算候选）、`on_exit`（未来资源释放请求）、`on_enter`（新状态上下文）、`tick`（状态内输出），返回值经 IF11/runtime 应用，禁止内部递归调用下个 handler。

轨迹至少记录：TurnStamp、完整 OwnerPath、from/to/rule_id、触发事件或 Tick、guard结果和计算输入、证据、消耗预算、租约申请/释放、ProposalId、关联 ActionId/JobId。被否决的高优先级边也记录有界 reason 摘要。每次输出能回答“为什么选它、为什么没有选另一候选”。

例：建设任务的 `T.Execute → T.Evaluate` 与 `I.Interacting → I.WaitingResult` 只在 D 收到同一个 ActionCommitted 时成立；下一 A 观察到建筑才有 GoalSatisfied。若最终选中集合没有该建设，则两条提交边都不发生。上级可以在 B 已执行一次，再在 C 执行一次；D 只能登记真实提交，不借机继续 Plan/Approach/Execute 连跳。

### 9.2 必须实现为固定输入测试的示例

| 用例 | 输入与计算 | 必须得到的结果 |
| --- | --- | --- |
| 返防等号 | 下个夜晚71，角色还需走4步，setup2，margin3；r=61时 `10 > 4+2+3`，r=62时 `9 = 4+2+3` | r61尚不因 G10 强制返防，r62进入 PrepareNight；若路径更长重新计算，不把阈值写死为某回合 |
| 基地风险 | HP1500，窗口内 potential_dps200，H3，scale1000 | risk400，恰好触发 Emergency；HP相同且dps99时risk198，才低于退出阈值200 |
| 角色风险 | HP220，potential_dps60，H3 | 向上取整 risk819，触发保全；不能整数截断成818改变边界规则 |
| 采售比较 | 测试配置流程开销2；两候选各采4件；A到矿2/到小贩3/出售1，价3；B到矿5/到小贩3/出售1，价5 | A周期12、收益12；B周期15、收益20，B收益率更高；若 B 返防预算不满足，则先硬过滤选择 A，而非继续选高收益 B |
| 金币冲突 | 可靠金币25，两工人分别建议建设25，另有采购10；仅有一个必需建设 Q1，其余 Q3 | 只给 Q1 建设授予25；不超支，不先发 Assign 再通知没钱；另工人可选不消费候选 |
| 开始接题 | r轮 accept 被提交，r+1观察本次 ChallengeStarted | r轮不得用未确认任务豁免额度；r+1后可按作业流程申请任务期 LLM；不能凭旧 phaseTask 非空抢先豁免 |
| 返防受阻 | 连续两次已提交 move 后角色均未变位置；中间没有出动作的一轮 Hold | stall为2，不是3；重算路径/站位。第3次无进展重规划仍失败后上报父任务，不靠新 PlanId 无限重来 |
| 死亡与成果 | 上一回合建设，本轮目标建筑已出现且工人已死 | 保存建设成果；M可 Succeeded，I=Dead，未来动作无该工人；不能因先处理死亡丢掉建设收益 |
| 战略滞回 | 紧急态后3个不同新帧各基地risk<200、关键角色risk<350且无防守缺口；第二帧重复请求一次 | 第3个新帧才退出，重复不增加计数；任一帧达到退出阈值或缺口再现则清零 |
| 最终回合 | 请求round1300，尚未收到最终结算/官方终止依据 | S=Endgame，仍可合法开火/交答案；不能在本轮开始就 Finished |
| 宝藏失败 | 已提交献祭，返回2，背包减少；下一轮无新线索 | 保存实际消耗和“地点或窗口”歧义；不原样再召唤、不恢复用品、不标记地点必错 |
| 冷却与等待 | 火箭cooldown Unknown、个体有操炮任务，但本轮无攻击提交 | T=Hold，I可OperatingWeapon且无建议；不能 I=WaitingResult(ActionAck)，不能假造 WeaponReady |

上述数字是文档算例；测试实现中也应按 AGENTS 命名常量。算法返回的估计应附计算分项，测试不能仅断言最后选中的 ID，否则难以发现期限/成本被漏算。

## 10. 实现划分与验收覆盖

具体依赖工作包见 [PLAN.md](PLAN.md) 第 4 节。各 FSM 拆独立状态处理器/规则组；共用 guard、风险、路径和效用函数，禁止把本表翻译成一个包含数十 match 分支的大函数。实现需满足 AGENTS 的 500/50/6 约束，测试独立文件。

| 验收 ID | 必须证明的行为 |
| --- | --- |
| BH01 | 78 个命名状态都有 handler/上下文、合法入口、tick/输出、退出/终态策略；每条允许边都有 guard 正/反例，非法边不能执行 |
| BH02 | A01 优先级、B/C/D 转移配额、证据保留、取消/死亡与旧成果并存；真提交才进入结果等待 |
| BH03 | A03 返防等号、未来工作终点返程、未知路径拒绝新长任务、最终回合仍行动 |
| BH04 | 风险向上取整、无活基地移除风险项、严格小于退出阈值、连续新帧滞回 |
| BH05 | 工人身份排序稳定；工种硬过滤；W1/W2只作并列偏好；P的背包不能借用工人物品 |
| BH06 | A04 优先级向量先于效用；资金/角色/武器/格子联合可行；批准后分配；候选上限不裁掉强制保全/返防 |
| BH07 | A05 按完整周期计算采售收益；保留石头仅对应已计划墙；两工人可不同站位采同矿 |
| BH08 | A06 首夜缺口、2×2/4×4/6×6 建造区、地图边界、武器上限、出口连通、未授权覆盖禁止、持券优先用实际持有人 |
| BH09 | A07 控制者/站位唯一、死亡局部重配、挑战放弃成本、补药不能同轮开火、无炮不能生成基地攻击 |
| BH10 | A09 加特林数量/点积/最近目标、电磁穿透、火箭叠加/冷却；联合击杀不提前删障碍，受预算约束 |
| BH11 | A10 对账成功/失败/Unknown、并发收益归因不明、迟到效果、消费未知不退款且不冻结所有未来非消费动作 |
| BH12 | A11 路径平局稳定、允许合法对角线、禁止争格/互换；stall只计真实失败；重规划计数不随PlanId重置 |
| BH13 | M/T/I作用域分离，Defense不因单次开火完成；Blocked释放普通人员、挑战等待不自动离开；无益撤退不移动 |
| BH14 | C接取确认/豁免、SOP前提、部分答案、未变化答案不重发、工具输出不等于正确答案、退出不假定会话已结束 |
| BH15 | J在Sent期间收到结果不丢失、按新帧超时、结果归属/墓碑、旧owner不能应用、未知通道隔离 |
| BH16 | N证据分级、独立来源去重、日期换算、当前价格优先、延迟发布预测、无额度延期 |
| BH17 | B物品多重集、准入/窗口、每场尝试计数、结果0–4、Unknown消费、无新证据禁止重试 |
| BH18 | 同输入/配置/固定工作预算生成相同任务配对、落点、TransitionRuleId轨迹；实时截止可降级且不输出半成品 |

BH 是待编写规格，不能标记为已经通过。策略参数调整须保存 PolicyConfig 版本，使用同回放样本换边比较合法动作率、返防准时率、存活回合、得分与时间消耗；不会只凭单局高分变更默认策略。DESIGN 的 FSM01–FSM16、INTERFACES 的 API01–API15 继续验证整体轨迹和模块接缝，三套规格职责互补。
