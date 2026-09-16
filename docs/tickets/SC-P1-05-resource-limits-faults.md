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

未完成：子进程树回收的深度检查（当前工具为单进程）、磁盘配额/大文件预算（SC-P1-06 性能基线）。

## 依赖

SC-P1-03、SC-P1-04。
