# `scene-core` 落地计划

> 状态：2026-09-16；已有 Rust workspace 和 CI 配置骨架；Protocol 0.1 DTO、严格解析、输入/派生身份、事件状态机和已确认时间规则的纯转换已在 `crates/scene-core-protocol` 实现并通过 workspace 检查，Schema、fixture、`version`/`doctor` 与媒体运行时尚未实现。工程骨架的历史本地检查通过不代表跨平台验收完成。新增媒体扩展方向已获用户确认，详见 [增量工程审查](media-extension-review.md)。

## 结论

`scene-core` 首版作为 `scene-seek` 背后的候选共享引擎。Core 交付平台专用 sidecar bundle 和 Host Contract，不交付 Tauri 或最终桌面安装包。先完成可独立验证的协议与工具链基础，再进入媒体执行。

## Phase 0A：协议与工程骨架

1. 建立 Rust workspace、精确固定的 toolchain、Schema、fixture、fixture manifest 和 transcript validator。
2. 冻结 Protocol `0.1`、严格 JSONL 状态机、Manifest、错误码/退出码矩阵和输入安全边界。
3. 用公共 `EngineIdentity`、`ToolchainIdentity`、`DerivationDescriptor` DTO 统一 StartRequest、事件、Manifest、`version` 和 `doctor` 的字段语义。
4. 将 `sourceVersionId`、文件 `contentHash`、逻辑 `inputFingerprint` 和缓存 `derivationKey` 分离。
5. 提前冻结 `TemporalSegment` 的 Scene/Shot seam：半开区间、单父级严格包含、无环、policy ref、provenance；不实现 segmentation operation。
6. 实现 `version` 和 `doctor` 的稳定 JSON 输出契约；Phase 0 的 `implementedOperations` 必须为空。

Phase 0A 完成后，Phase 1 可以开始媒体 operation 的实现；但正式 Windows 消费者发布和分发必须等待 Phase 0B。

## Phase 0B：供应链与 Windows bundle

1. 建立 FFmpeg lock、许可证/源码材料、能力基线、SBOM、CVE、升级和回滚门禁。
2. 固定外部信任锚：包内 manifest 只做完整性校验，Host/发布流程从包外可信 digest、release metadata 或签名验证 manifest。
3. 验证 Windows x64 LGPL sidecar 的 PE import/DLL closure、逐文件 hash、detached checksum 和干净环境 `version`/`doctor`。
4. 不依赖付费 Authenticode；第三方构建无法满足来源、许可、能力或闭包门禁时，切换到仓库控制的源码构建。

## Phase 1：Engine Core

实现 JSONL 状态机、取消/超时、FFprobe 归一化、诊断 preview、Manifest 和真实/合成媒体测试。只使用 Phase 0A 已冻结的 DTO、Schema、fixture 和 bundle contract。输入与 Artifact hash 必须流式处理，并在实现阶段建立大文件 P95 吞吐、CPU 和磁盘预算。

首版只执行 `probe` 与固定 `extract_preview`。音频提取、分析用抽帧在 Phase 1 后由真实消费者需求分别触发，不阻塞首版。现在澄清公共时间语义，具体扩展字段、格式和资源预算在各自 Spec 中确定；不预建空 crate、通用分析流水线或自由 FFmpeg 参数接口。

## Phase 2：Windows Host 与发布包

在 `scene-seek` Tauri/Rust Host 实现输入快照、AppContainer、Job Object、stdout/stderr 排空、Artifact 二次校验、缓存租约和 `smoke`。Core 不实现这些平台职责。

## Phase 3：`scene-seek` 产品闭环

完成导入、预览、进度、取消、错误、Artifact 注册和 staging 清理。Scene/Shot 只在有真实评测集和消费者需求后通过新 operation/version 接入。

## What already exists

- `docs/specs/scene-core-phase0-protocol-0.1.md`：Phase 0 的完整协议、身份、Manifest、供应链和 Host/cache 设计。
- `docs/tickets/SC-P0-01` 至 `SC-P0-07`：Issue-ready 的纵向任务拆分与依赖关系。
- `engine-contract.md`：Protocol 0.1 消费侧摘要，Rust DTO 为事实源。
- `cross-repository-contracts.md`：Web RAG、scene-core、scene-seek 的边界和迁移约束。
- `crates/scene-core-protocol` 已实现 Protocol 0.1 严格 DTO/解析、canonical JSON 摘要、输入与派生身份、`TemporalSegment` 校验和已确认时间规则的纯转换，并有 110 个通过用例（含事件状态机与 fixture conformance）；`schemas/0.1/`、`fixtures/protocol/**`、`version`/`doctor` 和媒体运行时仍未实现。本次增量审查提供 plan-level 证据，不声称新增功能或 Windows 验证通过。

## NOT in scope

真实 `run` 媒体执行（Phase 0）、ASR、OCR、VLM、Embedding、Rerank、检索、Evidence、Citation、数据库、队列、对象存储、完整 Tauri/React UI、完整 shot/scene segmentation、HTTP/gRPC、daemon、自动更新、macOS、GPU 加速、公共 Rust library API、消费者 Adapter、安装包和远程 GitHub 操作。

## Failure Modes and Guardrails

| Failure mode | Guardrail | Required evidence |
|---|---|---|
| Host 声明的输入与 staging 实际内容不同 | Core 复核 size/hash，失败为 `INPUT_CHANGED` | 大文件、篡改和并发替换 fixture |
| 不同实现计算出不同缓存键 | 公共 DTO + RFC 8785 + golden vectors | Host/Core 双实现 conformance |
| 失败结果污染缓存或索引 | completed + exit 0 + Manifest/hash/current-result gate + 原子 finalize | 取消、超时、崩溃、staged 产物负向测试 |
| 伪造或篡改 bundle manifest | 包外可信 digest/签名先验证 manifest，再验证文件 | 缺文件、篡改文件、未列 DLL、错误 trust anchor |
| 协议事件无法恢复或错误终止 | 严格 sequence、单终态、terminal/exit matrix | golden transcript 与坏 transcript |
| 路径逃逸或 reparse point 绕过 staging | Host/Core 双重 relative/regular-file/reparse 校验 | Windows symlink、junction、ADS、UNC fixture |
| 工具能力或许可证 profile 漂移 | lock、能力基线、SBOM、LGPL 门禁 | capability diff、许可和 bundle CI |
| 20 GiB 输入导致双重 hash 拖慢任务 | Host/Core 仍保留防篡改双校验，但使用流式 hash；Phase 1 设 P95 预算 | 大文件吞吐/CPU/磁盘基线 |

## Test Coverage Review

测试使用 Rust 内置 `cargo test --workspace`；协议 DTO/校验/时间转换的 Phase 0 用例已落地，下列媒体与 CLI 路径待实现：

```text
Consumer/Host
  -> verify external manifest trust anchor
  -> scene-core version/doctor --json
  -> create private staging + input/source.media
  -> [Phase 1] send StartRequest
       -> framing/BOM/size/duplicate-key parser
       -> strict DTO + semantic validation
       -> inputFingerprint + derivationKey recomputation
       -> staging containment + regular-file/hash validation
       -> operation dispatch
            -> accepted(seq=1)
            -> progress* (stage monotonic)
            -> completed(result + Manifest)
             OR failed(error)
             OR cancelled(error)
             OR timed_out(error)
       -> terminal/exit validation
       -> Host current-result gate + atomic finalize
       -> register/cache only complete Artifact set

Standalone Phase 0 checks
  version -> EngineIdentity + ToolchainIdentity -> one JSON stdout object / stable exit
  doctor  -> manifest -> external trust input -> file hashes -> DLL closure
          -> tool launch -> capability baseline -> temp-dir I/O -> ordered checks
          -> one JSON stdout object / stable exit

Cross-language conformance
  fixture-manifest -> Schema -> Rust DTO/parser -> transcript validator
                   -> expected accept/reject + unique rule + error code
```

覆盖清单：协议 framing/未知字段/重复 key、身份 hash、DerivationDescriptor、TemporalSegment 父子约束、事件状态机、错误码和退出码、Manifest 相对路径与 hash、CLI JSON、包外信任锚、工具链能力、取消/超时/部分产物、Windows reparse/DLL closure、Schema drift 和 golden canonical JSON。Host 的租约、single-flight、权限和缓存数据库在 `scene-seek`/Web Worker 侧测试，不在本仓库伪实现。

## Performance Review

- 输入 `contentHash` 在 Host 与 Core 各计算一次，最大 20 GiB 时可能产生约 40 GiB 顺序读取；Phase 0 不删除 Core 复核这一安全边界，Phase 1 必须用流式读取、固定 buffer 和基准门禁验证成本。
- `inputFingerprint` 与 `derivationKey` 只 hash 小型 canonical descriptor，瓶颈不是摘要算法；应避免把媒体字节载入内存。
- Artifact/Manifest 逐文件 hash 采用流式方式；Manifest 不包含媒体内容、时间戳或 ZIP 自身，保持 O(bundle 文件数) 的 doctor 成本。
- FFmpeg I/O、临时磁盘和进程启动成本属于 Phase 1/2 基线；不在 Phase 0 伪造跨平台性能承诺。
- 缓存键不包含 request/run identity，可减少重复推导；但缓存并发、租约、逐出和配额仍由 Host 实现并单独压测。

## Parallelization Strategy

| Step | Modules touched | Depends on |
|---|---|---|
| A. Workspace/toolchain/CI | workspace、CI | — |
| B. Protocol DTO/identity/validator | protocol crate、schemas source | A |
| C. FFmpeg lock/SBOM/license | packaging/toolchains | A |
| D. Schema/fixture/transcript | schemas、fixtures、validator | B |
| E. Version/doctor/manifest | engine CLI、packaging manifest | A、D |
| F. Windows bundle spike | packaging/bundles、Windows CI | C、E |
| G. Host/cache/document contract | docs、cross-repository contract | B、D、C |

Lane A：A → B → D → E；Lane B：A → C；Lane C：A → B/D/C 后执行 G。F 必须等待 C 和 E。先并行启动 Lane A 与 Lane B；合并协议/供应链基础后执行 E、G，再执行 F。Lane B 与 Lane F 都触碰 packaging，但阶段依赖已将其排成顺序，避免 manifest/lock 冲突。

## Implementation Tasks

Synthesized from this review's findings. 按批准的推荐方案落地；checkbox 在实现时勾选。

- [ ] **T1 (P1, human: ~1d / CC: ~20min)** — Protocol DTO — 建立 `EngineIdentity`、`ToolchainIdentity`、`DerivationDescriptor` 单一事实源，并统一 `engineCacheCompatibilityId`。
  - Surfaced by: Code Quality Review — 多处 envelope/Manifest/CLI 近似复制和 cache compatibility 命名漂移。
  - Files: `crates/scene-core-protocol/**`, `schemas/0.1/**`, `docs/specs/scene-core-phase0-protocol-0.1.md`
  - Verify: DTO 生成 Schema 无 diff；Host/Core descriptor golden vector 相同。
  - Status（2026-09-16）：Rust DTO 与 Host/Core 相同 key 的 golden 测试已实现；Schema 无 diff 证据待 SC-P0-03。
- [ ] **T2 (P1, human: ~0.5d / CC: ~10min)** — Error contract — 固化错误码、retryable、terminal event、exit code 的 Error Code Matrix。
  - Surfaced by: Code Quality Review — 错误语义分散，消费者可能把同一失败按不同方式重试或发布。
  - Files: `docs/specs/scene-core-phase0-protocol-0.1.md`, `docs/architecture/repositories/scene-core/engine-contract.md`, `fixtures/protocol/**`
  - Verify: 每个错误码有唯一终态/退出码 fixture；terminal/exit truth table 全通过。
  - Status（2026-09-16）：`ErrorCode` 矩阵、terminal/exit 映射、payload 校验和 `EventStreamValidator` 已实现；fixture 证据待 SC-P0-03。
- [ ] **T3 (P1, human: ~0.5d / CC: ~10min)** — Normalized media schema — 闭合 duration、尺寸、rotation、rational 和 `null` 语义。
  - Surfaced by: Code Quality Review — 缺失值、0 和不可表示数值可能被不同消费者解释。
  - Files: `crates/scene-core-protocol/**`, `schemas/0.1/**`, `fixtures/protocol/**`
  - Verify: null/zero/negative/denominator-zero/NaN 负向 fixture。
  - Status（2026-09-16）：归一化媒体 DTO 与校验已实现；负向 fixture 待 SC-P0-03。
- [ ] **T4 (P1, human: ~0.5d / CC: ~10min)** — Fixture conformance — 增加 fixture manifest，把每个 fixture 映射到规则、Schema、预期结果和错误码。
  - Surfaced by: Test Review — 仅按 fixture 数量验收无法证明规则覆盖。
  - Files: `fixtures/protocol/fixture-manifest.json`, `fixtures/**`, `.github/workflows/ci.yml`
  - Verify: runner 输出逐文件结果；manifest 中每条规则至少有正/负或 golden 证据。
- [ ] **T5 (P1, human: ~0.5d / CC: ~10min)** — CLI and package trust — 冻结 `version`/`doctor` 单 JSON stdout 契约，并验证包外 manifest trust anchor。
  - Surfaced by: Code Quality/Architecture Review — 可启动不等于 bundle 被可信验证。
  - Files: `crates/scene-core-engine/**`, `packaging/**`, `docs/tickets/SC-P0-04-*`, `docs/tickets/SC-P0-06-*`
  - Verify: healthy/tampered/missing/untrusted manifest cases with stable JSON and exit code。
- [ ] **T6 (P1, human: ~0.25d / CC: ~5min)** — Foundation cleanup — T01 前固定具体 Rust toolchain，Phase 0 删除空 `scene-core-media` crate。
  - Surfaced by: Architecture/Code Quality Review — 浮动 toolchain 与空模块会制造不可复现构建和无意义依赖。
  - Files: `rust-toolchain.toml`, `Cargo.toml`, `docs/specs/**`, `docs/tickets/SC-P0-01-*`
  - Verify: Linux/Windows 使用同一精确 toolchain；workspace 依赖图只有 `engine -> protocol`。
- [ ] **T7 (P1, human: ~1d / CC: ~15min)** — Large-input performance gate — 为双重流式 hash、Artifact hash 和 FFmpeg I/O 建立 P95 吞吐/CPU/磁盘基线。
  - Surfaced by: Performance Review — 20 GiB 输入可能带来约 40 GiB 顺序读取。
  - Files: `crates/scene-core-protocol/**`, `crates/scene-core-media/**`（Phase 1）, `tests/benchmarks/**`, `docs/architecture/**`
  - Verify: 大文件基准报告；不得通过取消 Core hash 复核来“优化”。
- [ ] **T8 (P1, human: ~1d / CC: ~15min)** — Path and reparse security tests — 为 relative path、regular file、symlink/junction/reparse/ADS/UNC 建立 Windows 负向测试。
  - Surfaced by: Architecture/Test Review — 仅字符串校验不足以阻止 Windows 文件系统逃逸。
  - Files: `crates/scene-core-protocol/**`, `crates/scene-core-engine/**`, `fixtures/protocol/**`, `.github/workflows/ci.yml`
  - Verify: Linux lexical tests 与 Windows filesystem integration tests 均覆盖拒绝路径。

## Review Status

- Step 0：范围接受为 Phase 0A/0B 拆分，不扩大到消费者实现。
- Architecture Review：已完成；采用 executionContext 必填、DTO/Schema 单一事实源、包外信任锚、双重路径校验、可复核可复现、精确 toolchain、删除空 media crate、最小 TemporalSegment seam。
- Code Quality Review：已完成；采用共享 DTO、错误矩阵、Schema null 语义、fixture manifest、稳定 CLI 契约和 foundation cleanup。
- Test Review：已完成；当前无测试框架，已输出 plan-level coverage diagram 和 test-plan artifact。
- Performance Review：已完成；保留 Core hash 防护，增加流式处理和 Phase 1 P95 基线，不在 Phase 0 假设性能承诺。

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
|--------|---------|-----|------|--------|----------|
| CEO Review | `/plan-ceo-review` | Scope & strategy | 1 | CLEAR | scope accepted; no open product decision |
| Codex Review | `/codex review` | Independent 2nd opinion | 0 | — | — |
| Eng Review | `/plan-eng-review` | Architecture & tests (required) | 1 | CLEAR | 15 findings/gaps, all folded into plan |
| Design Review | `/plan-design-review` | UI/UX gaps | 0 | NOT NEEDED | no UI scope |
| DX Review | `/plan-devex-review` | Developer experience gaps | 0 | DEFERRED | run after Windows Host/download flow exists |

**VERDICT:** CEO + ENG CLEARED — ready to implement Phase 0A; Phase 0B remains a required release gate. No UI review is needed for this plan.

用户已确认增量审查的时间规则，详见 [media-extension-review.md](media-extension-review.md)；SC-P0-02 的 DTO、纯转换和校验已实现并通过 `cargo test --workspace`，SC-P0-03 的 Schema/fixture 与真实媒体验收仍未完成。

NO UNRESOLVED DECISIONS
