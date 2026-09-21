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

## 实现状态（2026-09-16）

- `scene-core run --staging-root <path>`：stdin 首行 StartRequest、后续至多一个同 requestId 的 CancelRequest；`ControlStreamValidator`/`EventStreamValidator` 语义；EOF 不取消。
- 验证顺序：严格解析 → （解析失败时 lenient 提取身份并输出 sequence 1 的 `failed`）→ fingerprint/derivationKey 重算 → operation 可用性 → staging containment → 实际 size/hash（`INPUT_CHANGED`/`INPUT_NOT_FOUND`）。
- 事件：`accepted`(1) → `progress`(2, stage `probe`) → 终态(3)；身份字段原样回传；取消/超时通过共享 `RunControl` 传给媒体进程，超时 124、取消 130、失败按错误码矩阵（1/2/3）。
- 媒体后端以 `MediaBackend` trait 注入：真实 `ProbeBackend` 用锁定工具链与流式 hash；测试用假后端覆盖成功/取消/超时/violation/工具不可用。
- 无工具链时仍先 `accepted` 再 `TOOL_UNAVAILABLE` 失败；`SCENE_CORE_FFMPEG_DIR` + `SCENE_CORE_TOOLCHAIN_FINGERPRINT` 为开发/CI 注入点（bundle 内自动读取 descriptor）。
- 测试：7 个 run 合同测试（含真实工具链的端到端 probe：生成合成媒体 → staging → 进程 → 事件校验），workspace 共 146 tests，clippy 无 allow。

历史备注：本 ticket 初次实现时尚未包含 `extract_preview` 的 `run` 分发和磁盘不足注入；两项已分别由 SC-P1-04、SC-P1-05 完成，当前结项以 SC-P1-EPIC 汇总表为准。

## 依赖

SC-P1-01。
