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

### 结项后审查修复（2026-09-16）

对 `be22f9d` 的完整交付做了一轮 Standards/Spec 审查，确认并修复以下缺陷；每条都补了可运行检查（`cargo test --workspace` 全绿）：

| 类别 | 缺陷 | 修复 | 证据 |
|---|---|---|---|
| Spec P1 | staging 输入用 `is_file()` 跟随符号链接越界 | 逐层 `symlink_metadata` 拒绝符号链接（输入与输出路径） | `staging.rs` 三个符号链接用例（输入文件、输入目录、悬空输出） |
| Spec P1 | 非零原点预览定位把 origin 重复相加 | 按素材时间定位（去掉 origin 加项） | `preview_contract::nonzero_origin_previews_seek_by_material_time`；真实 TS 端到端与 `-seek_timestamp 1` 参考帧逐字节一致 |
| Spec P1 | 输入 hash 不受取消/deadline 约束 | 计时与取消线程提前到校验之前，hash 按块观察取消 | `hash::cancellation_stops_hashing`；`accepted`+`cancelled`/`timedOut` 终态 |
| Spec P2 | 身份完整的非法请求缺 `failed` 事件 | 宽松回显改用非严格解析，允许未知字段 | `identity_extras_still_report_a_failed_event` |
| Spec P2 | 重复 CancelRequest 未按协议错误拒绝 | 复用 `ControlStreamValidator` 统一控制流规则 | `control_session_rejects_a_second_cancel_and_a_foreign_request_id` |
| Spec P2 | 1 MiB 行限制在整行分配之后 | 读取阶段按上限截断，超限行按 framing 错误 | `cli::tests::capped_lines_*`、`oversized_first_line_is_rejected_without_buffering_it` |
| Standards P2 | 工具输出读取错误被当作 EOF | `read_capped` 显式返回 I/O 错误并向上映射 | `process::tests::read_capped_reports_truncation_and_read_errors` |
| Standards P2 | 事件写入/flush 错误被忽略 | 交付失败时以非零退出码结束 | `emit_events` 记录写失败并返回 `ENGINE_INTERNAL` |
| Standards P2 | 请求生成脚本整读媒体 | 分块流式 hash | `scripts/make-run-request.py` |
| Spec P1（复核追加） | 只校验最终 `.jpg`，预置 `opening.tmp` 符号链接可越界写入并回收为 Artifact | 临时文件路径同样过 `StagingRoot::contain` | `preview_temporary_symlink_cannot_escape_staging`（去掉校验即复现越界写入） |
| Spec P2（复核追加） | 非法 UTF-8 行被 `from_utf8_lossy` 替换后继续解析 | 非 UTF-8 行按 framing 错误拒绝 | `capped_lines_flag_invalid_utf8` |
| Standards P2（复核追加） | `recv_timeout` 的 Timeout/Disconnected 被伪装成空输出 | 两种情形都按读取错误上报 | `collect_output_reports_a_missing_reader_result` |
| Standards P2（复核追加） | 控制通道读取失败静默 break | 读取失败记录 framing 违规 | 控制线程错误分支 |
| Standards P2（复核追加） | 输入 `metadata` 错误被当成尺寸 0 | I/O 错误按 `INPUT_NOT_FOUND` 上报 | `validate_request` 元数据分支 |
| Standards P2（第三轮） | 控制通道读取失败记录违规后未退出循环，会忙等并反复覆盖错误 | 记录后 `break` | `capped_lines_propagate_read_errors` |
| Spec P2（第三轮） | 超限行仍继续读到换行/EOF，无换行的超长输入可在 deadline 之前无限等待 | 超过上限即刻返回该行（不再读到换行） | `oversized_lines_stop_reading_without_a_newline`（去掉提前返回即失败） |
| P2（第四轮） | 超限/非法 UTF-8/协议违规只记录违规、不退出控制读取循环，持续无换行输入会一直读下去 | 抽出 `read_control_lines`，任一 framing 违规或已存在违规即退出循环 | `control_reader_stops_on_an_endless_oversized_line`、`control_reader_stops_after_an_invalid_utf8_line`、`control_reader_stops_after_a_protocol_violation`（去掉退出即三项失败） |
| 额外（TS 探测） | MPEG-TS 的 `coded_width: 0` 被当成零尺寸流，导致 probe `ENGINE_INTERNAL` | `0` 视为未知；尺寸/采样率/声道只保留正值 | `zero_coded_dimensions_from_mpegts_are_unknown`；真实 TS probe + extract_preview 端到端通过 |

### 第二轮 Eng Review（2026-09-21）

范围：T1–T5 工作树改动（doctor/staging 链接边界、失败 stage、许可证引用闭合、文档状态、Windows staging 测试与 CI step）。结论：**CLEAN**（实现、契约与本地门禁）；唯一未闭合项是 Windows 运行时 CI 执行，属环境门禁而非设计结论。

| 项 | 证据 | 判定 |
|---|---|---|
| T1 doctor 链接/reparse 拒绝 | `cargo test -p scene-core-engine --test doctor_contract`（Linux 8/8）；`listed_symlinked_file_fails_bundle_files` / `listed_reparse_file_fails_bundle_files`；错误映射保持 `BUNDLE_FILE_INVALID` | CLEAN |
| T2 失败 stage 按 operation | preview 工具失败 stage=`preview`、probe=`probe`；timeout/cancel 仍为 `run`（`run.rs:593`、`run.rs:598`）；`run_contract` 断言通过 | CLEAN |
| T3 许可证引用闭合 | `package.rs` 要求 `thirdPartyLicensesRef/` 下至少一个已列文件，相似前缀目录被拒；`build-windows-bundle.ps1` 生成 `THIRD_PARTY_LICENSES/COPYING.LGPLv3` 并计入 `files`，真实 bundle 不受影响 | CLEAN |
| T4 文档状态同步与链接 | P0 Epic、实现计划与验收记录措辞一致，无“Phase 1 未创建”“媒体运行时未实现”残留；`docs/**` 57 条相对链接全部可解析（本轮修复 5 处越级路径） | CLEAN |
| T5 Windows staging 边界 | Linux staging 6/6 通过（ADS/UNC/traversal 词法拒绝 + Unix symlink）；Windows junction/symlink 用例交叉编译通过；CI 新增 `cargo +1.85.1 test -p scene-core-media staging --locked` | CLEAN（运行时待 CI） |

本地门禁：`cargo fmt --all -- --check`、`cargo check --workspace --locked`、`cargo test --workspace --locked`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`bash scripts/check-schema-drift.sh`、`git diff --check` 全部通过；另执行 `cargo check -p scene-core-media -p scene-core-engine --tests --target x86_64-pc-windows-msvc --locked` 覆盖 `#[cfg(windows)]` 测试代码。

非阻断观察（本轮不新增代码）：

1. `collect_files` 对 reparse 拒绝复用“bundle directory could not be walked”文案，错误码正确但诊断不够精确。
2. Windows symlink 用例依赖 runner 允许创建符号链接（junction 用例不依赖特权）；首次 Windows CI 运行若失败，应显式修复或前置能力检查，不能改为静默跳过。
3. `path_contains_link_or_reparse` 对每个已列文件重走祖先路径（O(files×depth)），当前 bundle 规模无影响。

### 仍然开放（不阻塞 Phase 1 结项）

1. ~~真实/公开脱敏样本、预览期望图与播放器点击对照~~ 已完成，见 [manual-verification.md](../acceptance/manual-verification.md) 与 [real-media-acceptance.md](../acceptance/real-media-acceptance.md)。
2. 并发/多请求下的峰值内存与磁盘配额：单请求与 20 GiB 已实测（567 MiB/s、峰值 152 MiB）；Host 侧准入、配额与 single-flight 契约已定义（[host-cache-contract](../specs/host-cache-contract.md) §6.1），Host 实现与实测待 P2。
3. 聚焦 Eng Review：第一轮审查发现已修复并留证；第二轮复核（2026-09-21）结论 CLEAN，见上节与 [media-extension-review.md](../architecture/repositories/scene-core/media-extension-review.md) 报告；Windows 运行时条件测试待 CI 首次推送执行。
4. ~~Windows sidecar 内的 `run` 端到端实测~~ 已在干净 Windows 上以 bundle artifact 跑通 probe/extract_preview 与负向矩阵。

## 完成定义

- 六个 Ticket 验收通过，`version --json` 如实报告 `implementedOperations: ["probe", "extract_preview"]`。
- Windows sidecar 在 bundle 内可执行 `run`（`bundle-smoke.ps1` 在解压 bundle 内跑 probe/extract_preview）；Linux CI 用锁定工具链跑合成媒体合同测试。
- 真实媒体验收记录包含时间偏移、VFR/B 帧、未知原点、损坏媒体与附件图例。（部分：一个真实 CFR 样本基线见 `docs/acceptance/real-media-acceptance.md`；异常类真实样本仍由合成覆盖）
- 失败、取消、超时和 staged 产物不发布、不可缓存（沿用 SC-P0-07 契约）。
