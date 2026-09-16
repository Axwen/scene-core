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

## 结项状态（2026-09-16）

| Ticket | 状态 | 证据 |
|---|---|---|
| SC-P1-01 media crate 与执行层 | 完成 | 无 shell/绝对路径、输出上限、deadline/取消 kill、staging symlink 拒绝；Linux 工具链 lock + fetch 脚本 |
| SC-P1-02 FFprobe 归一化 | 完成 | 时间规则向量 + 锁定工具链合同 + golden；封面图收紧为非 primary |
| SC-P1-03 run 状态机 | 完成 | accepted/progress/终态、取消/超时、退出码矩阵；真实工具链端到端 probe |
| SC-P1-04 预览与 Manifest | 完成 | 512 边长不放大、原子 finalize、Manifest DTO 校验、implementedOperations=[probe,extract_preview] |
| SC-P1-05 故障注入 | 完成 | deadline/只读输出/空工具链；并修复 1 MiB 栈缓冲导致的 Windows 崩溃 |
| SC-P1-06 验收与性能 | 完成（增量一/二/三） | rotation/封面/B 帧/VFR 样本、预览 golden、release P50/P95 与引擎峰值 RSS |

聚合：main 最近 run 全绿、失败 run 清零；`version --json` 如实报告 `implementedOperations: [probe, extract_preview]`。

### 仍然开放（不阻塞 Phase 1 结项）

1. 真实/公开脱敏样本、预览期望图与播放器点击对照（需要真实素材与人工操作）。
2. 并发/多请求下的峰值内存与磁盘配额（单请求 20 GiB 已实测：567 MiB/s、峰值 152 MiB）。
3. 聚焦 Eng Review：建议由用户按既有审查流程基于本表证据执行。
4. Windows sidecar 内的 `run` 端到端实测（当前 bundle 只冒烟 version/doctor）。

## 完成定义

- 六个 Ticket 验收通过，`version --json` 如实报告 `implementedOperations: ["probe", "extract_preview"]`。
- Windows sidecar 在 bundle 内可执行 `run`（`bundle-smoke.ps1` 在解压 bundle 内跑 probe/extract_preview）；Linux CI 用锁定工具链跑合成媒体合同测试。
- 真实媒体验收记录包含时间偏移、VFR/B 帧、未知原点、损坏媒体与附件图例。（部分：一个真实 CFR 样本基线见 `docs/acceptance/real-media-acceptance.md`；异常类真实样本仍由合成覆盖）
- 失败、取消、超时和 staged 产物不发布、不可缓存（沿用 SC-P0-07 契约）。
