# SC-P1：Engine Core（probe 与固定诊断预览）

## 目标

在已冻结的 Protocol 0.1 DTO、Schema 和 fixture 之上实现媒体运行时：`run` 的 JSONL 状态机、FFprobe 归一化、固定诊断预览、Artifact Manifest、取消/超时/资源限制和真实/合成媒体验收。媒体执行进入独立的 `scene-core-media` crate，协议层不引入 FFmpeg 概念。

## 范围

- 新建 `scene-core-media`：固定工具链的受控进程执行、ffprobe 归一化、诊断预览和艺术产物 hash。
- `scene-core run --staging-root <path>`：Start/Cancel 控制流、事件状态机、deadline/取消和退出码矩阵。
- 只实现 `probe` 与固定 `extract_preview`；不扩展诊断预览语义。
- 合成媒体生成与真实/脱敏样本验收；大文件流式 hash 与资源基线。

## 明确不做

音频提取、分析抽帧、Scene/Shot 检测与算法、缓存数据库、HTTP/gRPC 服务、Tauri/AppContainer、Linux 正式 bundle、公共 Rust API 承诺。

## Tickets

| ID | 任务 | 依赖 |
|---|---|---|
| [SC-P1-01](SC-P1-01-media-crate-execution.md) | 建立 `scene-core-media` 与固定工具链执行层（含 Linux 测试工具链 lock） | SC-P0-05、SC-P0-06 |
| [SC-P1-02](SC-P1-02-ffprobe-normalization.md) | FFprobe 归一化与展示时间转换（复用 `ExactTime`） | SC-P1-01 |
| [SC-P1-03](SC-P1-03-run-state-machine.md) | `run` JSONL 状态机、取消/超时与退出码矩阵 | SC-P1-01 |
| [SC-P1-04](SC-P1-04-extract-preview-manifest.md) | 固定诊断预览与 Artifact Manifest（流式 hash、原子 finalize） | SC-P1-02、SC-P1-03 |
| [SC-P1-05](SC-P1-05-resource-limits-faults.md) | 资源限制与故障注入（磁盘不足、取消、超时、部分产物隔离） | SC-P1-03、SC-P1-04 |
| [SC-P1-06](SC-P1-06-media-acceptance-perf.md) | 真实/合成媒体验收与性能基线（P95、峰值内存、输出体积） | SC-P1-02、SC-P1-04、SC-P1-05 |

## 完成定义

- 六个 Ticket 验收通过，`version --json` 如实报告 `implementedOperations: ["probe", "extract_preview"]`。
- Windows sidecar 在 bundle 内可执行 `run`；Linux CI 用锁定工具链跑合成媒体合同测试。
- 真实媒体验收记录包含时间偏移、VFR/B 帧、未知原点、损坏媒体与附件图例。
- 失败、取消、超时和 staged 产物不发布、不可缓存（沿用 SC-P0-07 契约）。
