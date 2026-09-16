# SC-P1-01：建立 `scene-core-media` 与固定工具链执行层

## 目的

把 FFmpeg/FFprobe 调用收进独立 crate，用锁定的工具链做受控进程执行，避免媒体逻辑泄漏进协议层。

## 范围

- 新建 `crates/scene-core-media`（`publish = false`），依赖 `scene-core-protocol`；禁止在协议 crate 引入进程或 FFmpeg 概念。
- 为 Linux 测试环境增加 `packaging/toolchains/x86_64-unknown-linux-gnu/toolchain.lock.json`（不可变 URL、SHA-256、LGPL 档位、能力基线），复用 `scripts/verify-toolchain-lock.sh`。
- 受控执行层：只用 bundle/lock 相对路径解析工具、不经过 shell、argv 由代码构造、stdout/stderr 有人头/字节上限、超时与取消可 kill、stderr 不进入协议或用户可见错误。
- 目录解析与 staging containment：输入只允许 `input/source.media`，输出只允许 staging 内相对路径。

## 验收标准

1. Linux CI 下载并校验锁定的 FFmpeg/FFprobe，执行 `-version` 与一次最小 `-i` 探测。
2. 执行层单测覆盖：超时 kill、取消 kill、非零退出、stderr 上限、路径越界拒绝。
3. `cargo tree` 证明依赖方向为 `engine -> media -> protocol`，protocol/media 均无 FFmpeg argv 或路径暴露。

## 实现状态（2026-09-16）

- 新增 `crates/scene-core-media`（`publish = false`），依赖协议 crate；engine 增加 `engine -> media -> protocol` 依赖边。
- Linux 工具链 lock：`packaging/toolchains/x86_64-unknown-linux-gnu/toolchain.lock.json`，BtbN `linux64-lgpl-shared` `n9.0.1-31-g3a7c002718`（62,531,572 字节、SHA-256、源码归档与构建脚本 hash、7 个共享库、LGPL-3.0-or-later 见 ADR-0001）。
- 新增 `scripts/fetch-toolchain.sh <target> <dest>`：校验 size/SHA-256 后解压并输出 bin 目录；Linux CI 用它设置 `SCENE_CORE_FFMPEG_DIR` 并运行 media 合同测试。
- 执行层：无 shell、绝对路径解析（不查 PATH）、stdout/stderr 上限与截断标记、deadline 与取消 kill、读取线程带收尾超时；staging 容器含 symlink 逃逸拒绝。
- 验证：本地用提取的 Linux 工具链跑通 7 个 media 测试（`ffprobe -version`、生成并探测合成媒体、缺文件非零退出、超时/取消、输出截断）；`cargo tree` 确认依赖方向；workspace 共 124 tests，clippy 无 allow。

## 依赖

SC-P0-05、SC-P0-06。
