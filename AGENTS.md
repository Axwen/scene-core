# Repository Guidelines

## 项目结构与模块组织

`scene-core` 是 Rust 媒体核心仓库，媒体运行时尚未实现。根目录的 `README.md` 说明定位；架构资料位于 `docs/architecture/repositories/`：`scene-core/` 保存职责、引擎协议、路线图和非目标，`cross-repository-contracts.md` 记录与 Web RAG、`scene-seek` 的公共契约。Rust workspace 已落地：`crates/scene-core-protocol/` 承载协议 DTO、严格解析、校验和已确认时间规则转换（含行为测试），`crates/scene-core-engine/` 目前仅为 CLI 入口；`schemas/0.1/`、`fixtures/protocol/` 由 SC-P0-03 交付，媒体资源与真实媒体验证仍待实现。新增实现时保持 Rust 源码、协议 Schema、fixture 和测试目录职责清晰，并同步更新相关文档。

## 构建、测试与本地开发

Rust workspace 已落地（edition 2024、resolver 3、固定 toolchain 1.85.1），提交前至少通过这些检查，CI 使用 `--locked`：

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

依赖变更后先更新 `Cargo.lock` 再提交。Schema 生成尚未实现，`scripts/check-schema-drift.sh` 目前只是 preflight（SC-P0-03 负责替换为生成式 drift 检查）。协议设计变更可先检查文档链接和示例：

```bash
grep -R "^#" docs/architecture/repositories
```

## 编码风格与命名约定

遵循 Rust 默认风格：4 空格缩进、`snake_case` 函数/变量、`PascalCase` 类型、`SCREAMING_SNAKE_CASE` 常量；提交前运行 `cargo fmt`。JSON/JSONL 协议字段保持现有 camelCase（如 `requestId`、`sourceVersionId`），不要把任意 FFmpeg argv 暴露为公共接口。时间区间必须满足 `startMs >= 0` 且 `endMs > startMs`。

## 测试指南

测试应围绕协议公共行为和不变量，而不是实现细节：覆盖非法请求、事件序号重复或回退、确定性 Manifest、损坏媒体、缺少轨道、取消、超时、资源限制及部分产物隔离。测试文件按行为命名，例如 `engine_contract.rs`；共享合成 fixture 放在测试专用目录，不提交敏感或受版权限制的媒体。

## 提交与 Pull Request

当前没有可用 Git 历史可供归纳；采用 Conventional Commits，例如 `feat: add manifest validation`、`fix: reject duplicate event sequence`、`docs: clarify engine contract`。PR 应说明目的、协议或行为变化、验证命令及兼容性影响；涉及 JSON/JSONL 示例时附前后对比，并链接相关 issue。未经评审不要提交密钥、主机绝对路径、原始 FFmpeg stderr 或业务权限信息。

## Agent skills

### Issue tracker

工作 issue 以 markdown 文件存放在 `docs/tickets/`（Epic + 单 ticket 文件）。详见 `docs/agents/issue-tracker.md`。

### Domain docs

单 context：仓库根目录的 `CONTEXT.md` 与 `docs/adr/`。详见 `docs/agents/domain.md`。

