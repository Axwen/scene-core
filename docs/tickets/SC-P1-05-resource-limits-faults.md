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

## 依赖

SC-P1-03、SC-P1-04。
