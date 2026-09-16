# Domain Docs

工程类技能在探索代码库时应如何消费本仓库的 domain 文档。

## Before exploring, read these

- 仓库根目录的 **`CONTEXT.md`**，或
- 若存在根目录的 **`CONTEXT-MAP.md`**：它指向每个 context 的 `CONTEXT.md`，按主题读取相关文件。
- **`docs/adr/`**：读取与当前工作区域相关的 ADR；多 context 仓库还应检查 `crates/<context>/docs/adr/`。

以上文件不存在时**静默继续**：不提示缺失，也不建议预先创建；`/domain-modeling` 会在术语或决策真正确定时按需创建。

## File structure

单 context 仓库（大多数仓库）：

```text
/
├── CONTEXT.md
├── docs/adr/
│   └── 0001-....md
└── crates/
```

多 context 仓库（根目录存在 `CONTEXT-MAP.md`）：

```text
/
├── CONTEXT-MAP.md
├── docs/adr/                     ← 系统级决策
└── crates/
    ├── scene-core-protocol/
    │   ├── CONTEXT.md
    │   └── docs/adr/             ← context 级决策
    └── scene-core-engine/
        ├── CONTEXT.md
        └── docs/adr/
```

## Use the glossary's vocabulary

产出中命名领域概念时（issue 标题、重构建议、假设、测试名），使用 `CONTEXT.md` 定义的术语，不要漂移到词汇表明确避免的同义词。

若所需概念不在词汇表中：要么你在发明项目不使用的语言（重新考虑），要么存在真实缺口（记下并交给 `/domain-modeling`）。

## Flag ADR conflicts

若产出与既有 ADR 冲突，显式提出而不是静默覆盖：

> _与 ADR-0007（event-sourced orders）冲突，但值得重新讨论，因为……_
