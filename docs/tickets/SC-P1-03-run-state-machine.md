# SC-P1-03：`run` JSONL 状态机、取消/超时与退出码矩阵

## 目的

实现 Host-facing 的 `scene-core run --staging-root <path>`：stdin 控制、stdout 事件、受控终态与退出码，复用已冻结的 DTO 与状态机。

## 范围

- 首行只接受一个 StartRequest；至多一行同 `requestId` 的 CancelRequest；EOF 不表示取消。
- 事件 `accepted`(seq 1) → running/progress → 唯一终态；`sourceVersionId`/`inputFingerprint`/`derivationKey`/`executionContext` 原样回传；复用 `EventStreamValidator`/`ControlStreamValidator`。
- 校验：协议版本、`operationConfigHash`、`outputContractVersion`、`derivationKey`（engine identity）、staging size/hash（`INPUT_CHANGED`）、deadline 1–600000。
- 终态与退出码：0 completed、1 ENGINE_INTERNAL、2 请求/协议/staging、3 媒体/工具、124 timed_out、130 cancelled；取消/超时后不再输出事件。
- 媒体操作经 media trait 注入，状态机与媒体实现解耦，便于假后端测试。

## 验收标准

1. 进程级集成测试覆盖：正常 probe 生命周期、取消、超时、非法请求（failed seq 1 + exit 2）、第二个 start/cancel。
2. terminal/exit 真值表与错误码矩阵一致；staged 产物不发布。
3. `--staging-root` 只接受受控内的 `input/source.media`，绝对路径/`..`/ADS 被拒绝。

## 依赖

SC-P1-01。
