# Issue tracker: 本地 Markdown（docs/tickets）

本仓库的 issue 与 spec 以 markdown 文件形式存放在 `docs/tickets/`，不依赖远端或 `gh`/`glab` CLI。

## Conventions

- Epic：`docs/tickets/<EPIC-ID>-EPIC.md`（如 `SC-P0-EPIC.md`），包含目标、范围、明确不做、ticket 表和依赖关系。
- 单 ticket 一文件：`docs/tickets/<EPIC-ID>-<NN>-<slug>.md`（如 `SC-P0-02-protocol-identity-segments.md`），从 `01` 编号，禁止合并成单个 tickets 文件。
- 每个 ticket 使用 `## 目的`、`## 范围`、`## 验收标准`、可选 `## 实现状态` 和 `## 依赖` 章节，并在正文声明依赖。
- 使用 triage 状态时，在文件顶部附近写 `Status:` 行。
- 评论与对话历史追加到文件末尾 `## Comments` 标题下。

## When a skill says "publish to the issue tracker"

在 `docs/tickets/` 下创建或更新文件（必要时创建目录），遵循上述命名与章节约定；新 ticket 默认加入该功能已有 Epic，只有开启新 Epic 时才新建 `<EPIC-ID>-EPIC.md`。

## When a skill says "fetch the relevant ticket"

直接读取给定路径的文件；用户通常传路径或 ticket ID（如 `SC-P0-02`，按前缀匹配唯一文件）。

## Wayfinding operations

供 `/wayfinder` 使用。**map** 是 `docs/tickets/<EFFORT>-map.md`，每个 ticket 一个 **child** 文件。

- **Child ticket**：`docs/tickets/<EFFORT>-<NN>-<slug>.md`，从 `01` 编号，正文写问题。`Type:` 行记录类型（`research`/`prototype`/`grilling`/`task`）；`Status:` 行记录 `claimed`/`resolved`。
- **Blocking**：顶部附近的 `Blocked by: NN, NN` 行；所列文件全部 `resolved` 时解锁。
- **Frontier**：扫描该 effort 下 open、未阻塞且未认领的文件，编号最小者优先。
- **Claim**：开始前设置 `Status: claimed` 并保存。
- **Resolve**：在 `## Answer` 下追加答案，设置 `Status: resolved`，再把上下文指针追加到 map 的决策记录。
