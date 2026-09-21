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

未完成：子进程树回收的深度检查（当前工具为单进程）、磁盘配额/大文件预算（SC-P1-06 性能基线）。

## Host 并发与配额边界（P2，不在本 ticket 实现）

Protocol 0.1 的 Core 是单请求进程：本 ticket 的 deadline/取消/输出上限只约束当前请求，不引入进程内并发或跨请求配额状态。Host 要同时运行多个 Core 请求前，必须按 [Host 集成与缓存契约](../specs/host-cache-contract.md) §6.1 与[跨仓库契约](../architecture/repositories/cross-repository-contracts.md) §3.4 定义并测试：全局/每 `cacheScope` 最大活动请求数、内存与磁盘配额预留、admission reject、同派生键 single-flight、失败回滚清理，以及继续沿用不可缓存语义（失败/取消/超时/partial 永不命中）。这些验收属于 Host 实施，Core 侧不新增并发或配额实现。

## 依赖

SC-P1-03、SC-P1-04。
