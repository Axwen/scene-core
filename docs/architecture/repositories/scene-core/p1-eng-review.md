# P1 实现聚焦工程审查（第二轮 Eng Review）

日期：2026-09-21。范围：`main` @ `08327b0`（含 PR #28）的 P1 实现（protocol / media / engine 与 CI 门禁）。方法：按 plan-eng-review 结构执行，gstack 脚本环境不完整（`~/.config/opencode/skills/gstack/bin` 缺失、`rg` shim 指向不可用的 mise），进入降级模式：三个只读审查代理分别覆盖 engine、media、protocol+tests，所有发现先按“引用触发代码行”门槛过滤，P1 发现由主审逐条复现或读码确认。

本文不是发布承诺，也不替代 Host/真实媒体验收；只记录实现层缺陷与验证缺口。

## 结论与决策记录

**第二轮 Eng Review：`DONE_WITH_CONCERNS`**（第一轮为 `DONE_WITH_CONCERNS`，T1–T5 的修复本身已 CLEAN，但完整实现审查新发现 3 个 P1、16 个 P2）。P1 均影响已交付行为：

1. **合法媒体的 `extract_preview` 会整体失败**：容器时长中点晚于视频轨道结尾时，midpoint 帧生成“成功但无文件”，引擎按 `TOOL_FAILED` 终止并丢弃已生成的 opening（已用锁定工具链复现）。
2. **缺少工具链指纹时 `run` 不产出协议事件**：stdout 直接输出 `CliErrorOutput`，Host 的 `EventStreamValidator` 看不到 `accepted/failed`（读码确认）。
3. **`completed` 事件的 Manifest 不校验身份回传**：`requestId`/`sourceVersionId`/`engine` 可与事件信封不一致而通过校验（读码确认）。

## Step 0 / Scope Challenge

- 现有实现已覆盖目标；本轮不新增能力、不新建服务或抽象，只审查现有代码路径。
- 最小变更集：按发现定级修复；Host 并发配额（T6）与性能门禁（T7）已关闭，不并入本轮。
- 复杂度检查未触发：无新增文件/组件；`doctor`/staging/preview 均为既有模块。
- 搜索检查：Aside/WebSearch 不可用（环境降级），按既有进程执行与校验模式判断，未引入新库或新架构。
- TODOS 交叉引用：T1–T7 已完成；本审查发现成为新的待决工作项，见 `TODOS.md`。

## Architecture Review

```text
Host ──JSONL──> engine(cli/run) ──> media(process/staging) ──> ffmpeg/ffprobe
                    │                      │
                    └── protocol DTO/validator（身份、事件、Manifest、package）
```

边界总体成立：协议层无 FFmpeg 概念，media 只执行与归一化，engine 负责状态机与错误映射。发现：

- **[P1] (confidence 9/10) `cli.rs:172` — 工具链指纹解析失败时 `run` 不进入事件协议。**
  `resolve_toolchain_fingerprint()?` 直接把错误交给 `main`，stdout 得到 `CliErrorOutput` JSON（`main.rs:5-7`），而不是 `accepted` + `failed(TOOL_UNAVAILABLE)`。影响：无 `SCENE_CORE_TOOLCHAIN_FINGERPRINT` 且无 bundle descriptor 的开发/CI 调用、以及缺 descriptor 的 bundle，Host 侧整个事件流校验失败且无法按重试语义处理。修：解析失败时仍走 `run_session` + `UnavailableBackend`，让失败以协议事件表达。
- **[P2] (confidence 9/10) `package.rs:194` — 许可证目录闭合可被“指向已必需目录”绕过。**
  `license_prefix` 前缀匹配对 `thirdPartyLicensesRef: "bin"` 成立（`bin/scene-core.exe` 已必需），T3 的“至少一个许可证文件”变成空约束。修：拒绝与其他必需路径/引用构成前缀的 license ref，或把该 ref 冻结为 `THIRD_PARTY_LICENSES`。
- **[P2] (confidence 8/10) `doctor.rs:287` + `identity.rs:58` — descriptor 的 DLL 闭包未与 `files`/磁盘交叉校验。**
  `read_toolchain_identity` 丢弃 `sharedLibraries`，`check_bundle_files` 只比对磁盘与 manifest；descriptor 声明与 manifest 哈希不一致（或 DLL 完全缺失）时 doctor 仍 ok，SC-P0-06 的“动态库闭包”门禁未落实。修：要求每个 `sharedLibraries[].path` 在 `manifest.files` 中存在且 `sha256` 相同。
- **[P2] (confidence 6/10) `staging.rs:78-102` — 校验与打开之间存在 TOCTOU 窗口。**
  `contain` 在子进程启动前做词法检查，`-y` 写入的是普通路径；同用户进程可在窗口内替换父目录为链接。修：每请求 root 权限收紧（0700）+ 子进程结束后重新校验父路径；残余窗口写入威胁模型文档。

## Code Quality Review

- **[P1] (confidence 9/10) `preview.rs:93-102` + `run.rs:221-231` — midpoint 超出视频结尾即整单失败。**
  引擎用容器 `duration_ms/2` 选点；`-ss` 晚于最后一帧时 ffmpeg 退出 0 但不写文件（已复现：1 s 视频 + 60 s 音频，`-ss 30` 无输出），`!output.is_file()` → `ToolFailed`，已生成的 opening 一并丢弃。修：midpoint 不得超过 primary video 的 `durationMs`；无帧时不生成该 Artifact 而不是失败。
- **[P2] (confidence 8/10) `run.rs:97` / `control.rs:272,314` — CancelRequest 不做协议版本校验。**
  `parse_control_line` 只 `parse`，`validate()`（含 `engineProtocolVersion.is_supported()`）从未调用；`0.9` 的 cancel 被接受并触发取消/退出 130。修：校验后按 framing 违规处理。
- **[P2] (confidence 7/10) `run.rs:283,580` + `probe.rs:17` / `preview.rs:12` — 工具内建 120 s 超时被报告为请求 `TIMEOUT`/124。**
  只有工具自身 120 s 上限能命中该分支；`deadlineMs` 未到的用户收到“已达到 deadline，请加大 deadlineMs”的错误 nextStep，重试必然复现。修：仅在 `control.is_timed_out()` 时映射 `Timeout`，否则归 `RESOURCE_LIMIT`。
- **[P2] (confidence 8/10) `toolchain.rs:40-49` — 空/相对 `SCENE_CORE_FFMPEG_DIR` 绕过“绝对路径、从不查 PATH”的保证。**
  `PathBuf::from("").join("ffmpeg")` 变成无斜杠的 `"ffmpeg"`，`Command` 会查 PATH。修：`from_bin_dir` 拒绝非绝对路径。
- **[P2] (confidence 7/10) `process.rs:132` — `try_wait` 失败直接返回，子进程不 kill/reap、读线程泄漏。**
  与 SC-P1-05“超时/取消回收工具进程”冲突。修：错误路径先 `kill` 再 `wait`。
- **[P2] (confidence 8/10) `preview.rs:13-14` — `force_divisible_by=2` 把 1 px 边压成 0。**
  1x1 视频 filtergraph 报 `Rescaled dimensions 0x0 are invalid` → `ToolFailed`。修：目标尺寸下限 `max(2, …)`。
- **[P2] (confidence 5/10) `run.rs:239-243` — staging 越界/权限失败折叠为可重试 `ENGINE_INTERNAL`。**
  Host 按 `retryable` 重试确定性失败；现有测试只断言 `exit_code != 0`。修：containment 违规改为不可重试错误，输出目录 I/O 失败归 `RESOURCE_LIMIT`。
- **[P2] (confidence 9/10) `event.rs:228-233` — progress 数值无单调与上限校验。**
  spec §5 要求同 stage 内单调、`completed ≤ total`；`EventStreamValidator` 只跟踪 identity/sequence/accepted/terminal，构造 `5/10 → 2/10` 或 `99/2` 均通过。修：状态机记录 `(stage, completed, total)` 并拒绝回退与越界。
- **[P2] (confidence 9/10) `values.rs:664,673` — 发布 Schema 接受 validator 拒绝的值。**
  `RelativeRef` Schema 只有 `minLength/maxLength`（`/etc/passwd`、`C:/x` 可过）；`MediaType` 的 `[a-z0-9.+-_]` 中 `+-_` 是字符范围，`IMAGE/JPEG` 可过（已用 Python 正则验证）。修：为 Schema 补与 validator 等价的 pattern/not 约束，并固化等价性用例。
- **[P2] (confidence 7/10) `identity.rs:105` + `package.rs:69-73` — LGPL 门禁是精确黑名单，`licenseProfile` 不校验档位。**
  `--enable-libx264`（GPL-only 组件）与 `licenseProfile: "GPL-3.0-only"` 在 LGPL 档位下均可通过。修：按记录基线校验 configure flags，并拒绝非 LGPL 的 license 表达式。
- **[P2] (confidence 8/10) `manifest.rs:181` — opening/midpoint 的 `requestedTimeMs` 未固定。**
  0.1 预览 profile 固定 opening=0、midpoint=`floor(duration/2)>0`；`validate_preview_slot` 未约束，`requestedTimeMs=12345` 的 opening 可通过。修：按 slot 校验取值。
- **[P2] (confidence 8/10) `fixtures/protocol/golden-jsonl/*.jsonl` — golden transcript 与引擎实际输出不一致。**
  引擎固定发 `accepted → progress → completed`，golden 文件缺 progress 且 total 不同；fixture runner 只做接受性校验，消费者据 golden 开发会得到错误的“黄金”流。修：按 `run_session` 实际输出重生成 golden，或补引擎输出对比测试。
- **[P2] (confidence 7/10) `cli.rs:189` — `DoctorOutput::validate` 不校验十项检查的冻结顺序与名称。**
  重排/改名/追加检查都能通过 DTO 校验，消费者无法依赖报告结构。修：要求 `checks` 是十项注册名称的有序前缀。

## Test Review

现有 23 个测试目标全绿，但覆盖集中在协议与正常路径。按分支/错误路径审计出的主要缺口（不逐行展开）：

```text
engine/run       ├── [GAP] INPUT_CHANGED / INPUT_NOT_FOUND（validate_request 359-368、388-410）
                 ├── [GAP] 取消/超时发生在媒体工作前（438-453）、完成后（519-534）
                 ├── [GAP] hash 被取消中断（cli.rs:219-231）
                 └── [GAP] extract_preview 真实后端成功路径与 midpoint（204-277）
engine/cli       ├── [GAP] 指纹解析失败（172）与 run 参数错误（113-124）
                 └── [GAP] 控制读取错误/非法 UTF-8/EOF（269-289）、emit 失败（366-377）
engine/doctor    ├── [GAP] engine-version 不匹配、descriptor 读取失败（102-153）
                 ├── [GAP] capabilities digest/结构/空列表（426-476）、temp-dir 失败（480-495）
                 └── [GAP] RealCommandRunner 失败路径（runner.rs:24-28）
media/process    ├── [GAP] ProcessError::Wait（132）、stderr 截断、run 使用原始程序路径
media/probe      ├── [GAP] ResourceLimit、分数时长/i128 溢出、fs-size 回退、时长溢出
                 └── [GAP] SC-P1-02 未实现的“时间戳复位/展示回退 → UNSUPPORTED_INPUT”
media/preview    ├── [GAP] 取消/超时/资源限制映射、NoVideoStream 构造点
                 └── [GAP] seek 晚于视频结尾、单帧、奇数/1px 尺寸、旋转后的预览
media/staging    ├── [GAP] StagingError::Io、root 外候选（47-48、87）
media/hash       ├── [GAP] 哈希中途取消、读错误、空文件
protocol/event   ├── [GAP] Progress 单调/上限、Manifest↔信封身份回传
protocol/schema  └── [GAP] Schema 与 validator 等价性（RelativeRef/MediaType/deadlineMs）
```

新增（相对第一轮）应固化的回归用例：midpoint 超出视频结尾（M1）、取消版本校验、许可证 ref 指向 `bin/`、`MediaType` 大写 schema 差异。

## Failure Modes

| 场景 | 当前行为 | 要求 | 证据 |
|---|---|---|---|
| 视频短于容器一半（带长音轨） | `extract_preview` 整体 `TOOL_FAILED`，opening 丢弃 | 只跳过 midpoint，opening 仍发布 | 已复现 |
| 缺工具链指纹 | stdout 非事件 JSON，Host 协议失败 | `accepted` → `failed(TOOL_UNAVAILABLE)` | 读码 |
| 事件 Manifest 身份不一致 | 校验通过 | 拒绝 | 读码 |
| Progress 回退或 `completed>total` | 校验通过 | 拒绝 | 读码 |
| `licenseRef: "bin"` | 校验通过（空闭合） | 拒绝 | 读码 |
| `deadlineMs=600000` 但工具耗时 >120 s | 报 `TIMEOUT`/124，建议加大 deadline | 区分工具上限与请求超时 | 代理证据 |
| doctor 读取被替换的超大文件 | 全量读入内存，可能 OOM | 先比 size，流式哈希 | 读码 |
| 多视频轨文件 | 预览可能取自非 primary 轨 | 显式 `-map` primary 索引 | 代理证据 |

## Performance Review

- `doctor.rs:299` 对每个已列文件 `fs::read` 全量分配；bundle 被篡改为超大文件时 doctor 可能 OOM，而不是报 `BUNDLE_FILE_INVALID`。修：先比 `metadata.len()`，再复用 `hash_file` 流式校验。
- 其余路径符合设计：工具输出有上限、控制行 1 MiB 上限、hash 1 MiB 堆缓冲、流式无整段媒体入内存。
- 无 N+1/数据库路径；性能门禁（T7）已覆盖 probe/preview/hash 的回归冒烟。

## Suppressed findings

- TOCTOU（`staging.rs:78-102`，confidence 6/10）保留在正文，但按威胁模型需与 Host 权限设计一起决策；本轮不建议单独改。
- `doctor.rs:86` 硬编码 descriptor 文件名（confidence 6/10）：manifest 可声明其它 ref 并通过校验；与 DLL 闭包发现同属“descriptor 契约未闭合”，合并处理。

## 修复状态（2026-09-21，分支 `sc-p1-eng-review-fixes`）

用户决定修复全部 P1+P2。逐条修复与回归证据：

| 发现 | 修复 | 回归证据 |
|---|---|---|
| P1 preview midpoint 越界整单失败 | `run.rs` 用 `min(container, video)` 夹取中点，midpoint 无帧/工具失败时只跳过该 Artifact；`preview.rs` 新增 `NoFrame` 区分“无帧”与工具失败 | `preview_midpoint_stays_inside_a_short_video_track`（MP4 夹取到 500 ms）、`preview_skips_a_midpoint_without_a_video_duration`（MKV 无轨道时长，completed 且仅 opening）、`seeks_past_the_last_frame_report_no_frame` |
| P1 缺指纹不产出协议事件 | `cli.rs` 解析失败仍走 `run_session` + `UnavailableBackend`，产出 `accepted → failed(TOOL_UNAVAILABLE)` | `missing_fingerprint_still_reports_accepted_then_failed` |
| P1 Manifest 身份未回传 | `event.rs` 对 `extract_preview` 结果比对 request/source/fingerprint/derivation/engine | `completed_manifest_must_echo_the_event_identity`（4 种篡改） |
| P2 progress 无单调/上限 | `event.rs` 校验 `completed ≤ total`，状态机拒绝同 stage 回退 | `progress_values_must_not_regress_or_exceed_total` |
| P2 cancel 不校验版本 | `control.rs` `ControlStreamValidator` 拒绝不支持的 `engineProtocolVersion` | `cancel_with_an_unsupported_version_is_rejected` |
| P2 工具 120 s 上限误报 TIMEOUT | 两个 classifier 仅在 `control.is_timed_out()` 时映射 `Timeout`，否则 `RESOURCE_LIMIT` | `tool_timeouts_only_report_deadline_when_the_deadline_fired` |
| P2 非绝对 bin_dir 绕过 PATH 保证 | `toolchain.rs` 拒绝非绝对路径（新增 `NotAbsolute`） | `relative_bin_dirs_are_rejected` |
| P2 `try_wait` 失败不回收子进程 | `process.rs` 错误路径先 kill + wait 再返回 | 无法稳定构造 wait 失败，代码路径审查 |
| P2 1 px 视频预览失败 | `preview.rs` 缩放下限 `max(2, …)` | `one_pixel_frames_are_previewed` |
| P2 staging 失败折叠为可重试 Internal | 新增 `MediaFailure::StagingViolation`（`INVALID_REQUEST`，不可重试）；输出目录 I/O 归 `RESOURCE_LIMIT` | `preview_temporary_symlink_cannot_escape_staging` 断言 code/retryable |
| P2 许可证 ref 可指向 `bin/` | `package.rs` 拒绝作为其他必需路径父目录的 license ref | `license_directory_must_not_be_a_required_parent` |
| P2 Schema 与 validator 不等价 | `RelativeRef` 补等价 pattern（ECMA-262 lookahead），`MediaType` 修正字符范围并限长 128，`deadlineMs` 补 1..=600000 | `wire_schemas_encode_the_validator_constraints` + schema 重生成 |
| P2 LGPL 门禁不完整 | `identity.rs` 增加 GPL-only 组件开关清单；`package.rs` 要求工具 license 表达式为 `LGPL-*` | `descriptor_rejects_gpl_flags_and_empty_closure`、`tool_license_profile_must_be_lgpl` |
| P2 requestedTimeMs 未固定 | `manifest.rs` opening 必须 0、midpoint 必须 > 0 | `preview_requested_times_are_pinned_to_their_slots` |
| P2 golden JSONL 与引擎输出不一致 | 按引擎实际输出重生成 probe/preview success 与 resource-limit 事件（accepted → progress(0/total) → terminal） | `fixture_conformance` 通过 |
| P2 DoctorOutput 顺序未校验 | `cli.rs` 增加 `DOCTOR_CHECK_NAMES`，校验十项有序前缀 | `doctor_checks_must_follow_the_frozen_order` |
| P2 doctor 硬编码 descriptor 文件名 | `doctor.rs` 读取 `manifest.toolchainDescriptorRef` | `toolchain_descriptor_ref_is_honored` |
| P2 DLL 闭包未交叉校验 | `doctor.rs` 要求 descriptor 的每个库在 manifest 中同 hash | `descriptor_library_hashes_must_match_the_manifest`、`descriptor_libraries_must_be_listed_in_the_manifest` |
| P2 doctor 全量读入文件 | `doctor.rs` 先比 `metadata.len()` 再流式 `hash_file` | `missing_and_tampered_files_fail_bundle_files` |

本地门禁（2026-09-21，带锁定工具链）：`fmt`、`check --workspace`、`test --workspace`（23 targets 全绿）、`clippy -D warnings`、`check-schema-drift.sh`、`git diff --check`、Windows 目标 `cargo check --workspace --tests` 与性能门禁（probe p95 10.5 ms / preview p95 25.8 ms / hash 3319 MiB/s）全部通过。

## NOT in scope

- Host 侧并发/配额实现（`host-cache-contract.md` §6.1 已定义）、音频/抽帧能力、真实媒体缺口（见 `real-media-acceptance.md`）、许可复审与对外分发。

## GSTACK REVIEW REPORT

| Review | Runs | Status | Findings |
|---|---:|---|---|
| Eng 聚焦审查（第二轮，2026-09-21） | 2 | DONE_WITH_CONCERNS | 3×P1、16×P2；覆盖缺口集中于 engine 错误路径与 media 边界 |
| 修复复核（2026-09-21，`sc-p1-eng-review-fixes`） | 3 | CLEAN | 3×P1、16×P2 全部修复并补回归；无未决项 |
| 独立模型复核 | 0 | NOT RUN | 环境降级，未运行第二模型 |

**VERDICT:** 第二轮审查发现已全部修复并留证；T1–T5 修复本身维持 CLEAN。合并后以 CI 三检查为准。

NO UNRESOLVED DECISIONS
