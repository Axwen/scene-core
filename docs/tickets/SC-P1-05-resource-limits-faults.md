# SC-P1-05：资源限制与故障注入

## 目的

证明异常路径不伪报成功、不发布部分产物，并给出稳定错误码。

## 范围

- deadline 超时：kill 工具进程、输出 `timed_out` + `TIMEOUT`、exit 124、清理由 Host 隔离。
- 取消：`cancelled` + `CANCELLED`、exit 130，取消后不再有事件。
- 磁盘不足/输出超限：`RESOURCE_LIMIT`、exit 3 或 2（按矩阵），不发布半成品。
- 工具缺失/不可启动：`TOOL_UNAVAILABLE`；工具非零退出：`TOOL_FAILED`（不泄漏原始 stderr）。
- 故障注入通过可替换的执行层与临时小容量目录完成。

## 验收标准

1. 每种故障有进程级测试，断言终态、错误码、退出码与 staging 无已发布产物。
2. 超时/取消后工具进程确实被回收（无孤儿）。
3. 失败 run 不产生可缓存集合，符合 SC-P0-07 契约。

## 实现状态（2026-09-16）

- 资源类失败映射：工具输出含 `no space left` 或 `permission denied` 时归类 `RESOURCE_LIMIT`（probe 与 preview 两路），错误文本仍用引擎安全消息，不泄漏原始 stderr。
- 进程级故障测试（带锁定工具链）：
  - `deadlineMs=1` → `timed_out`、退出 124，staging 无 output；
  - 输出目录只读 → `failed` + `RESOURCE_LIMIT`、退出 3，且 `output/preview/opening.jpg` 不存在（部分产物隔离）；
  - `SCENE_CORE_FFMPEG_DIR` 指向空目录 → 先 `accepted` 再 `TOOL_UNAVAILABLE`、退出 2；
  - 取消/超时在单元层由假后端确定性覆盖（真实进程取消受竞态影响，已由 `RunControl` 单测锁定）。
- 端到端故障测试当场抓到一个真实缺陷：`implemented_operations()` 仍只报告 `probe`，导致 `extract_preview` 被 validate 拒绝（SC-P1-04 遗漏）。已修为 `[probe, extract_preview]`。
- workspace 共 154 tests，clippy 无 allow；连续两次全量运行稳定通过。

### Windows 栈溢出根因（2026-09-16 已修复）

- 现象：`SCENE_CORE_FFMPEG_DIR` 指向空目录时 Windows 子进程以 `0xC00000FD` 退出。
- 根因：`hash_file` 用 `[0_u8; 1024 * 1024]`（1 MiB）**栈缓冲**；Windows 主线程默认栈同为 1 MiB，在 staging 校验阶段即溢出；Linux 8 MiB 未暴露。本地用 `ulimit -s 1024` 复现。
- 修复：缓冲改为堆分配；新增 256 KiB 栈线程的回归测试；该进程测试重新在 Windows 启用。
- 顺带修复：测试并发共享 staging 目录名导致的竞态（改用原子计数唯一命名），连续三次全量运行稳定。

### 产物 finalize 与迟到取消（2026-09-21 复核修复）

- 预览与音频产物先全部写入临时文件、全部校验通过后统一 finalize（预览批量 rename，音频把 rename 放到哈希与连续性校验之后）；任何失败出口由临时文件守卫清理，rename 批失败回滚已发布路径。
- session 在终态为非成功（迟到取消/超时/协议违规）时删除后端已发布的产物，补上“失败/取消/超时无产物”的最后一个窗口。
- 确定性测试：`a_late_cancel_discards_the_finalized_artifact`（假后端 + 预置产物）、`finalize_publishes_the_whole_set_or_rolls_back`、`a_temporary_is_removed_unless_kept`。

未完成：子进程树回收的深度检查（当前工具为单进程）、磁盘配额/大文件预算（SC-P1-06 性能基线）。

### 子进程树回收评估（2026-09-21，仍暂缓）

- 现状：`process::run` 只 kill 直接子进程；若后代进程继承 stdout/stderr 管道，读取线程会滞留到后代退出。`collect_output` 在子进程退出后 2 s 超时返回错误，不阻塞调用方，且 Core 为一次性进程，风险有界。
- 候选 `command-group` 5.0.1（MSRV 1.68、Apache-2.0 OR MIT）：Unix 走 `nix`、Windows 走 `winapi` + `CREATE_SUSPENDED`，会新增两个平台依赖并改变子进程启动路径。当前 FFmpeg 为单进程，收益不足，暂不引入；出现多进程工具（如包装脚本）时再评估并补充跨平台回收测试。

## Host 并发与配额边界（P2，不在本 ticket 实现）

Protocol 0.1 的 Core 是单请求进程：本 ticket 的 deadline/取消/输出上限只约束当前请求，不引入进程内并发或跨请求配额状态。Host 要同时运行多个 Core 请求前，必须按 [Host 集成与缓存契约](../specs/host-cache-contract.md) §6.1 与[跨仓库契约](../architecture/repositories/cross-repository-contracts.md) §3.4 定义并测试：全局/每 `cacheScope` 最大活动请求数、内存与磁盘配额预留、admission reject、同派生键 single-flight、失败回滚清理，以及继续沿用不可缓存语义（失败/取消/超时/partial 永不命中）。这些验收属于 Host 实施，Core 侧不新增并发或配额实现。

## 依赖

SC-P1-03、SC-P1-04。
