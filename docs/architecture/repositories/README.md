# scene-core 架构资料

> 本目录由 Web RAG 规划包于 2026-09-05 迁移。它只包含媒体核心规划和跨仓库协议快照，不表示 Rust 媒体运行时已经实现。

- [scene-core 媒体核心规划包](scene-core/README.md)
- [三仓库跨仓库公共契约](cross-repository-contracts.md)

公共语义事实源仍在 Web RAG 仓库的 `packages/contracts` 与 `packages/rag-core`；本仓库负责媒体处理边界，不负责业务数据库、队列、权限或 AI SDK。

## 当前 Spec 与 Tickets

- [Phase 0 Protocol 0.1 Spec](../../specs/scene-core-phase0-protocol-0.1.md)
- [Phase 0 Epic](../../tickets/SC-P0-EPIC.md)
