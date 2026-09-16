# SC-P0-01：初始化 Rust workspace、toolchain 与 CI

## 目的

建立后续 Ticket 共用的最小 Rust 工程骨架和跨平台检查入口。三个 crate 保持 workspace-private，不承诺 crates.io Rust API。

## 范围

- 新建 `Cargo.toml` workspace、`rust-toolchain.toml` 和两个 crate：`scene-core-protocol`、`scene-core-engine`。Phase 0 不创建空的 `scene-core-media`；Phase 1 首次实现媒体 operation 时再单独立项。
- 固定 Rust edition 2024、resolver 3、具体精确 stable toolchain；记录 Linux/Windows 可用的 MSRV，禁止浮动 `latest`。
- 建立 `.github/workflows/ci.yml`：Linux 执行 `cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 和 Schema drift；Windows 11 x64 执行 check/test 及 bundle 入口。
- 预留 `schemas/0.1/`、`fixtures/`、`packaging/toolchains/`、`packaging/bundles/` 目录，不实现媒体操作。

## 验收标准

1. 空 workspace 在固定 toolchain 上通过上述本地命令。
2. 依赖方向为 `engine -> protocol`，所有 Phase 0 crate `publish = false`。
3. Linux/Windows 使用同一 lockfile；CI 不依赖浮动分支或 `latest` 资产。
4. CI 失败时能定位到具体命令和目录，且 Windows job 不要求本机安装 Rust 之外的运行时依赖。

## 非目标

不添加 FFmpeg 业务封装、不实现 `run`、不构建安装包、不修改远程分支。

## 依赖

无；这是本 Epic 的起始任务。
