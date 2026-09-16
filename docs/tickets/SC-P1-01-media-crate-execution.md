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

## 依赖

SC-P0-05、SC-P0-06。
