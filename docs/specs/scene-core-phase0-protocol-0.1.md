# Phase 0：建立 scene-core Protocol 0.1 与 Windows Sidecar 基础

> 状态：2026-09-07 基于已确认决策整理；2026-09-14 增量确认音频/分析抽帧后置和职责边界。已有工程骨架，正文 Current State 保留最初核查背景；新增能力尚未实现。用户已确认[素材时间规则](media-time-policy-draft.md)，作为本候选协议的规范性补充；转换实现和 fixture 验证仍待完成。
> 范围：仅 `scene-core`；不修改 `scene-seek`、Web RAG 或远程 GitHub 状态。


## Context

`scene-core` 目前只有媒体核心规划文档，没有 Rust workspace、协议 Schema、测试 fixture、CI 或可执行发布物。首个直接使用者将是 Windows 上的 `scene-seek`，但本仓库只负责平台专用的媒体引擎 bundle；Tauri、AppContainer、Job Object、最终安装包、签名和用户体验属于消费者仓库。

现在需要先冻结一个可实现、可测试、可扩展的候选 `0.1` 协议，并把四个容易形成技术债的边界前置：Scene/Shot 领域 seam、FFmpeg 供应链、输入集合身份和任务缓存推导模型。Windows sidecar 只能在来源、许可、能力和依赖闭包可验证后进入 Phase 1；否则媒体逻辑会在 DTO、版本、路径、安全、缓存和发布形态尚未统一时开始编码。

### Stakeholders

- `scene-core` 实现者：需要无待定设计的协议、目录结构和测试门禁。
- Host Adapter 实现者：需要稳定的进程、staging、取消、事件和 Artifact 校验契约。
- 最终用户：间接受益于可诊断、可取消、失败不污染结果的媒体处理链路。

### Why Now

Phase 1 的 `probe` 和 `extract_preview` 依赖本规格中的消息、Manifest、版本和 bundle 边界。先实现媒体逻辑会导致现有 `1.0` 文档、已批准 `0.1` 方向与消费者假设继续漂移。

## Current State

验证日期：2026-09-07。

| 组件 | 当前事实 | 缺口 |
|---|---|---|
| 仓库 | `README.md` 明确运行时尚未实现 | 无 `Cargo.toml`、源码、测试和 CI |
| 引擎协议 | `engine-contract.md` 仍使用 `protocolVersion: 1.0`、`artifact_ready`、`outputFingerprint` 和 partial Artifact | 与已批准 `0.1`、原子发布和单一 `inputFingerprint` 冲突 |
| 路线图 | M1 同时承诺音频、封面、关键帧和基础帧 | 与“PCM 由真实消费者需求触发”冲突 |
| 职责文档 | 同时描述 library、CLI、消费者安全和平台处理 | 尚未区分公共 sidecar、内部 crate 与 Host 职责 |
| 发布 | 没有平台 bundle、工具来源、checksum、SBOM 或许可清单 | 无法被消费者固定和验证 |
| 领域模型 | Scene/Shot 只在后续路线中出现 | 若不先冻结时间区间、父子关系和 provenance，后续 Evidence 映射会再次改协议 |
| 输入与缓存 | `inputFingerprint` 同时被理解为文件 hash、source identity 和缓存条件 | 缺少输入集合身份、派生键和 Host 缓存状态模型 |
| FFmpeg 管理 | 只假定“固定第三方 LGPL 构建” | 未覆盖构建来源、DLL closure、能力基线、CVE、升级和回滚 |
| Git/CI | 当前目录不是有效 Git 仓库，`gh` Token 无效 | 本规格只生成本地 Issue-ready 文档；仓库初始化后再接 GitHub Actions |

## Conflict and Extensibility Audit

### 必须修正的现有冲突

1. `implementation-plan.md` Phase 0 中的 TypeScript 影子消费者不进入本仓库 Ticket，改为第二消费者阶段的外部门禁。
2. AppContainer、Job Object、NTFS ACL、Tauri、最终签名、Defender 和干净安装 VM 属于 `scene-seek`；本仓库只定义 Host Integration Contract。
3. Core 的 Windows 产物是 sidecar ZIP，不是桌面安装包。Rust 源码跨平台，但 `.exe`、Linux ELF 和 FFmpeg 工具必须分别构建。
4. `version` 和 `doctor` 在 Phase 0 实现；真正执行 `probe/extract_preview` 的 `run` 与端到端 `smoke` 在 Phase 1 实现。
5. 三个 Rust crate 在 Phase 0 保持 workspace-private；公共兼容承诺只覆盖 JSON/JSONL 和 bundle。未来发布 Rust library 必须单独评审 Rust API SemVer。
6. 旧文档中“破坏性变化升 major”只适用于未来稳定协议。`0.x` 使用精确版本，新增或改变字段发布新的 `0.2`，引擎可并行声明支持 `0.1` 和 `0.2`。
7. Scene/Shot 的最小领域模型和 Schema seam 在 Phase 0 冻结，但 `probe`/`extract_preview` 不生成它们；Scene 只能先作为带 provenance 的 candidate，不能由 Core 宣称为业务权威 Scene。
8. `inputFingerprint` 不再表示单文件 hash；它是带版本的逻辑输入集合摘要。单个文件的 `contentHash`、`sourceVersionId` 和任务缓存的 `derivationKey` 分开建模。
9. Core 保持无状态，不保存缓存数据库；协议返回可验证的 `derivationKey`，Host 负责缓存命中、租约、并发去重、Artifact 持久化和权限隔离。
10. FFmpeg 不再作为“下载两个 exe”处理；供应链策略与 Windows bundle 可复现验证拆成独立门禁。第三方构建不能满足来源、许可、能力或依赖闭包要求时，切换到自建 CI 构建。

### 后续扩展不会破坏 0.1 的规则

| 后续能力 | 扩展方式 | 不允许的做法 |
|---|---|---|
| PCM | 新协议版本增加 `extract_audio_pcm` 与 `audioPcm` Artifact | 把 PCM 塞进 `extract_preview` 或返回内存 bytes |
| Linux Worker | 发布独立 Linux bundle，复用同一受支持协议版本 | 假定 Windows/Linux JPEG hash 相同 |
| Scene/Shot | Phase 0 先冻结 `TemporalSegment`/父子关系/provenance；后续用新 operation 输出 shot 或 scene candidate | 把诊断 preview 重新解释成 Scene/Shot，或让 Core 宣称业务 Scene 权威 |
| HTTP/gRPC 服务 | 服务 Adapter 包装现有请求、事件和 Manifest | 把数据库、对象存储或权限字段加入引擎协议 |
| 硬件加速 | 新 capability 与显式 operation profile | 静默改变默认输出和确定性边界 |
| Rust library | 单独发布并建立 Rust API SemVer | 将内部 crate API 当成 0.1 公共承诺 |
| 新 FFmpeg 许可档 | 新 bundle variant 与独立 package manifest | 在同一 bundle 身份下切换 LGPL/GPL 配置 |
| 任务缓存 | Host 以 `(cacheScope, derivationKey)` 保存完整不可变 Artifact 集；命中时新建 Run/provenance | 复用旧 Run identity，或把失败/取消/部分产物写入成功缓存 |

## Proposed Change

### 1. Repository Foundation

建立以下结构：

```text
Cargo.toml
rust-toolchain.toml
crates/
  scene-core-protocol/
  scene-core-engine/
schemas/0.1/
fixtures/protocol/valid/
fixtures/protocol/invalid/
fixtures/protocol/golden-jsonl/
fixtures/media/
packaging/toolchains/x86_64-pc-windows-msvc/
packaging/bundles/windows-x86_64/
docs/specs/
docs/tickets/
.github/workflows/ci.yml
```

- Rust edition 使用 2024，workspace resolver 使用 3。
- `rust-toolchain.toml` 在 T01 开始前固定一个已验证、可复现的具体 stable 版本；Linux 与 Windows 使用同一版本，Phase 0 的 MSRV 等于该版本。禁止使用 `latest` 或未锁定的浮动 toolchain。
- Phase 0 的生产依赖方向固定为 `scene-core-engine -> scene-core-protocol`。
- 两个 crate 均设置 `publish = false`；不得产生 crates.io 发布工作。
- `scene-core-media` 不在 Phase 0 创建；首次实现媒体 operation 时再按独立 Ticket 建立，避免空模块和虚假依赖。
- CI 在 Git 仓库初始化后使用 GitHub Actions：Linux 执行 fmt、clippy、check、test、Schema drift；Windows 11 x64 执行 check、test 和 bundle spike。

### 2. Protocol Versioning

- 协议字段统一命名为 `engineProtocolVersion`。
- 首个值固定为字符串 `0.1`；它与 `engineVersion`、`manifestVersion`、`packageManifestVersion` 独立。
- `0.x` 请求必须精确匹配 `version --json` 的 `supportedProtocolVersions`，不做范围协商。
- 每个版本的请求、事件与 Manifest 都拒绝未知字段。新增字段或枚举值需要新协议版本。
- 引擎可以并行支持多个精确版本；删除旧版本必须发布兼容说明。
- 公共 operation 使用 tagged union。每个 operation 有独立 options 和 completed result，禁止通用自由键值 options 或任意 FFmpeg argv。

### 3. Input Identity

`sourceVersionId`、单文件内容 Hash、逻辑输入身份和任务缓存身份必须分开：

| Identity | Meaning | Owner |
|---|---|---|
| `sourceVersionId` | 业务侧不可变来源版本的关联 ID | Host |
| `contentHash` | 一个实际输入文件的 SHA-256 | Host 计算，Core 执行前复核 |
| `inputFingerprint` | 有序逻辑输入集合的内容身份 | Host 计算，Core 重算并回传 |
| `derivationKey` | 特定输入、操作、输出契约、引擎兼容边界和工具链的可缓存推导身份 | Host 与 Core 独立计算 |

`inputFingerprint` 保留现有字段名以降低跨仓库迁移成本，但其唯一语义固定为：

```text
inputFingerprint = sha256(RFC8785(InputSetDescriptor))

InputSetDescriptor = {
  "inputSetVersion": "1",
  "inputs": [
    {"role": "source_media", "contentHash": "sha256:<hex>", "byteSize": 1048576}
  ]
}
```

- `inputs` 在 canonicalization 前按 `role` 的 UTF-8 字节序排序；同一 role 不得重复。Canonical JSON 使用 RFC 8785，hash 字符串统一小写十六进制。
- `ref`、文件名、主机路径、`sourceVersionId`、request/run/generation/attempt 和修改时间不进入摘要。
- Protocol `0.1` 恰好包含一个 `source_media`；字幕、sidecar metadata 或多角度媒体必须通过新协议版本增加 role。
- Hash 使用快照后的原始字节，不使用路径、mtime 或“快速 hash”。Core 在媒体操作前校验实际 size/hash；不一致返回 `INPUT_CHANGED`。
- 同一内容可以对应不同 `sourceVersionId`；这不改变 `inputFingerprint`，但 Host 的 current-result gate 仍必须匹配当前 source version。

### 4. Control Messages

stdin 是 UTF-8、无 BOM、每行一个 JSON object 的控制流；单行硬上限 1 MiB，拒绝重复 key。

#### StartRequest

```json
{
  "engineProtocolVersion": "0.1",
  "messageType": "start",
  "requestId": "req_01",
  "operation": "probe",
  "sourceVersionId": "sourcev_01",
  "inputs": [
    {
      "role": "source_media",
      "ref": "input/source.media",
      "contentHash": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      "byteSize": 1048576
    }
  ],
  "inputFingerprint": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "operationConfigHash": "sha256:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd",
  "outputContractVersion": "probe-result/1",
  "derivationKey": "sha256:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd",
  "executionContext": {
    "runId": "run_01",
    "generation": 0,
    "attempt": 1,
    "scope": "asset"
  },
  "deadlineMs": 120000,
  "options": {}
}
```

约束：

- `requestId`、`sourceVersionId` 和 `runId` 为 1–128 字符，匹配 `[A-Za-z0-9][A-Za-z0-9._:-]{0,127}`。
- `inputFingerprint` 必须匹配 `sha256:[0-9a-f]{64}`。
- `operationConfigHash` 必须是补齐默认值后的有效 options 的 RFC 8785 SHA-256；`outputContractVersion` 必须是该 operation 在 `0.1` 注册的结果契约版本。
- `derivationKey` 必须使用同一格式，并与第 9 节的 Core 重算结果一致。
- `inputs[0].byteSize` 为 `0..21474836480`，其 `contentHash` 使用同一 SHA-256 格式。
- `inputs` 在 `0.1` 必须恰好包含 `{role: "source_media", ref: "input/source.media"}`。Host 通过 CLI 提供 request staging root，JSON 不包含主机绝对路径。
- Core 必须根据 `inputs` 重算 `inputFingerprint`；摘要不匹配属于 `INVALID_REQUEST`，实际文件 size/hash 不匹配属于 `INPUT_CHANGED`。
- `ref` 与 Manifest 的 `relativeRef` 必须满足安全相对引用形式：不得为绝对路径、Windows 盘符、UNC、ADS、包含 `..` 或 `.` 段、反斜杠或控制字符，单字段上限 1024 字节；`0.1` 的输入 `ref` 只能精确等于 `input/source.media`。
- Host-facing `StartRequest` 的 `executionContext` 必填，四个字段必须齐全。`generation >= 0`，`attempt >= 1`，`scope` 为 `asset | query | evaluation`；仅不经过引擎请求协议的 `version`/`doctor` CLI 不携带它。
- `deadlineMs` 可省略，默认 120000；范围为 `1..600000`。
- `probe` 和 `extract_preview` 在 `0.1` 的调用方 options 都必须是空对象 `{}`；`extract_audio_pcm` 接受可选 `audioStreamIndex`。含未注册键、重复键或非对象形态（如数组）在解析阶段即被拒绝；选项形状与 operation 不匹配属于 `INVALID_REQUEST`。可配置行为通过新协议版本增加，不预留任意键。完整音频线格式见 [extract_audio_pcm 操作规格](extract-audio-pcm.md)。

#### CancelRequest

```json
{
  "engineProtocolVersion": "0.1",
  "messageType": "cancel",
  "requestId": "req_01"
}
```

- 第一行必须且只能是一个 `StartRequest`。
- 后续最多一行，且只能是同 `requestId` 的 `CancelRequest`。
- EOF 表示不再发送控制消息，不表示取消。
- 第二个 start、第二个 cancel 或不同 `requestId` 的 cancel 均为协议错误。

### 5. Event Envelope and State Machine

事件写 stdout，逐行 flush。stderr 不是协议，只允许受控诊断。

```json
{
  "engineProtocolVersion": "0.1",
  "messageType": "event",
  "requestId": "req_01",
  "sourceVersionId": "sourcev_01",
  "inputFingerprint": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "derivationKey": "sha256:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd",
  "operation": "probe",
  "sequence": 1,
  "eventType": "accepted",
  "engine": {
    "engineVersion": "0.1.0-alpha.1",
    "engineCommit": "0123456789abcdef0123456789abcdef01234567",
    "target": "x86_64-pc-windows-msvc",
    "engineCacheCompatibilityId": "scene-core-output-v1",
    "supportedProtocolVersions": ["0.1"],
    "implementedOperations": []
  },
  "executionContext": {
    "runId": "run_01",
    "generation": 0,
    "attempt": 1,
    "scope": "asset"
  },
  "occurredAt": "2026-09-07T00:00:00Z"
}
```

- 状态机为 `Boot -> Accepted -> Running -> Completed | Failed | Cancelled | TimedOut`。
- `accepted.sequence = 1`；后续 sequence 每次加 1，无跳号、重复或回退。
- emitted `accepted` 后必须恰有一个终态，终态后不得输出事件。
- 完全无法解析首行时不写 stdout，写安全 stderr 后 exit 2。
- 能取得 `requestId` 但请求无效时，输出一个 `failed` 事件且 `sequence = 1`，不先输出 `accepted`，随后 exit 2。
- `progress` 包含 `stage`、`completed`、`total`。数值必须非负且同一 stage 内单调；消费者不得根据 stage 文案驱动业务状态。
- `completed` 必须携带 operation-specific `result`；`failed`、`cancelled` 和 `timed_out` 必须携带 `error`，且不得携带权威 Artifact Manifest。
- `sourceVersionId`、`inputFingerprint`、`derivationKey` 和 `executionContext` 必须由所有 Host-facing 事件原样回传；`version`/`doctor` 是独立 CLI 输出。
- 事件的 `engine` 复用第 9 节完整的 `EngineIdentity`（`engineVersion`/`engineCommit`/`target`/`engineCacheCompatibilityId`/`supportedProtocolVersions`/`implementedOperations`）；Manifest、CLI 和事件不得各自定义近似子集。

退出码：

| Code | Required terminal |
|---:|---|
| 0 | `completed` |
| 1 | `failed` with `ENGINE_INTERNAL` |
| 2 | 请求、协议、staging 或 capability 错误对应的 `failed` |
| 3 | 媒体或固定工具错误对应的 `failed` |
| 124 | `timed_out` |
| 130 | `cancelled` |

Host 强杀、进程崩溃、坏 JSON、无终态、身份不符、sequence 异常或 terminal/exit 不一致由 Host 合成为协议失败；Core 文档必须明确这些结果不可发布。

### 6. Errors

```json
{
  "code": "INVALID_REQUEST",
  "message": "请求字段无效。",
  "cause": "deadlineMs 超出允许范围。",
  "nextStep": "使用 1 到 600000 之间的 deadlineMs 后重试。",
  "retryable": false,
  "stage": "validation",
  "limit": 600000,
  "actual": 700000
}
```

`0.1` 错误码固定为：

```text
INVALID_REQUEST
UNSUPPORTED_PROTOCOL
OPERATION_UNAVAILABLE
INPUT_NOT_FOUND
INPUT_CHANGED
UNSUPPORTED_INPUT
CORRUPT_MEDIA
MISSING_VIDEO_STREAM
MISSING_AUDIO_STREAM
RESOURCE_LIMIT
TOOL_UNAVAILABLE
TOOL_FAILED
TIMEOUT
CANCELLED
ENGINE_INTERNAL
```

- `message`、`cause` 和 `nextStep` 必须安全且可行动。
- `stage`、`limit`、`actual` 可选，除此之外不允许任意 details map。
- `retryable` 由错误码唯一决定（见引擎契约的错误矩阵），调用方不得自行设置；错误码、终态事件与退出码的配对是冻结契约。
- 不得输出绝对路径、完整 argv、原始 FFmpeg stderr、媒体内容或凭据。

### 7. Reserved Operation Results

Phase 0 冻结 `probe` 与 `extract_preview` 的标识和结果边界，但不实现操作。`version --json` 必须通过 `implementedOperations` 如实报告当前能力；不得伪报 capability。

0.1 候选扩展新增 `extract_audio_pcm`（[操作规格](extract-audio-pcm.md)）：整轨 PCM WAV `s16le`、保持源采样率与声道、可选 `audioStreamIndex`；结果与预览同形，Artifact 固定为 `audio-pcm` / `output/audio/track.wav` / `audio/wav`，并携带 `presentationTimeMs`（实际首个输出样本的素材时间）与 `audioPcm { sampleRate, channels, sampleCount }`。既有 `probe`/`extract_preview` 线格式不变；被消费者固定后需新协议版本。

#### Probe result

`probe` 的 completed result 包含 `operation: "probe"`、`media` 和 `resourceUsage`，不生成空 Artifact Manifest。`resourceUsage` 在 `0.1` 固定为 `wallTimeMs`（必填非负整数）、`cpuTimeMs` 与 `peakMemoryBytes`（可空非负整数，未知为 null）；Manifest 不包含资源统计。

`media` 至少包含：

```text
container.formatName
container.durationMs?
container.startTimeMs?
container.bitRateBps?
container.fileSizeBytes
streams[]
primaryVideoStreamIndex?
```

每个 stream 包含 index、kind、codecName、durationMs、startTimeMs、timeBase、default disposition 和 attachedPicture；视频流另带 coded/display dimensions、rotation 和平均帧率有理数；音频流另带 sampleRate、channels 和 channelLayout。所有可选归一化值都必须显式为字段或 `null`，不得用省略、`0`、空字符串或 NaN 表示未知。不同 `kind` 的扩展字段互斥：音频流不得携带视频字段，视频流不得携带音频字段，字幕/数据/附件流不得携带两类扩展字段；`attachedPicture` 只允许出现在视频流。尺寸为正整数或 `null`，duration/bitrate 为非负整数或 `null`；container.startTimeMs 在展示原点已知时为 0，否则 null；stream.startTimeMs 为相对该原点的有符号整数或 null。rotation 为可验证的整数角度或 `null`，有理数为 `{num: integer, den: positive integer}` 或 `null`。

#### Extract preview result

时间语义遵循[素材时间规则](media-time-policy-draft.md)：以容器展示起点为唯一原点，内部使用有理数和检查溢出的运算，不分别归零音视频轨。点时间取最近毫秒，半毫秒远离零；覆盖区间起点向下、终点向上取整。公共请求/标注保持非负半开区间；轨道元数据允许负起点。原点未知不猜零，无法可靠确定的位置使用 null。诊断预览允许未知实际帧时间，Host 不得用请求时间替代出镜证据；要求精确映射的后续操作无法满足时间契约时受控失败。后续音频映射按实际输出起点计算，专用字段在音频操作 Spec 中确定。

`extract_preview` 的 completed result 包含相同 `media`、`artifactManifest` 和 `resourceUsage`。

- 诊断预览 profile 固定为 JPEG、最大边 512、不放大、保持比例、应用显示旋转、移除 metadata、软件解码。
- 有有效 duration 时请求 0ms 与 `floor(durationMs / 2)`；时间相同时只保留 opening。
- 无有效 duration 时只请求 opening 0ms；无可解码视频流时失败。
- 单帧只记录 `requestedTimeMs`，可证明时才增加 nullable `presentationTimeMs`；不生成 `startMs/endMs`。预览帧的 `byteSize` 必须为正，`pixelWidth`/`pixelHeight` 必须是已知正整数，`presentationTimeMs` 未知为 `null`。
- Artifact 固定有序，ID/ref 为 `preview-opening` / `output/preview/opening.jpg`，可选 `preview-midpoint` / `output/preview/midpoint.jpg`。

Manifest 示例：

```json
{
  "manifestVersion": "0.1",
  "requestId": "req_01",
  "sourceVersionId": "sourcev_01",
  "inputFingerprint": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "derivationKey": "sha256:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd",
  "operation": "extract_preview",
  "operationConfigHash": "sha256:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd",
  "outputContractVersion": "extract-preview-result/1",
  "engine": {
    "engineVersion": "0.1.0",
    "engineCommit": "0123456789abcdef0123456789abcdef01234567",
    "target": "x86_64-pc-windows-msvc",
    "engineCacheCompatibilityId": "scene-core-output-v1",
    "supportedProtocolVersions": ["0.1"],
    "implementedOperations": []
  },
  "toolchainFingerprint": "sha256:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd",
  "artifacts": [
    {
      "artifactId": "preview-opening",
      "kind": "previewFrame",
      "role": "opening",
      "mediaType": "image/jpeg",
      "relativeRef": "output/preview/opening.jpg",
      "byteSize": 12345,
      "contentHash": "sha256:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd",
      "requestedTimeMs": 0,
      "presentationTimeMs": null,
      "pixelWidth": 512,
      "pixelHeight": 288
    }
  ]
}
```

- `operationConfigHash` 对补齐默认值的 versioned effective profile 按 RFC 8785 canonical JSON 后计算 SHA-256。
- Manifest 不含时间戳和资源统计。
- Artifact hash 只承诺同 target、同 engine bundle、同 operation profile 下稳定。

### 8. Temporal Segment and Scene/Shot Seam

Phase 0 在 `scene-core-protocol` 中定义可生成 Schema 和 fixture 的 `TemporalSegment`，供后续 segmentation operation 复用；它不加入 `probe` 或 `extract_preview` 的 `0.1` result union：

```json
{
  "segmentId": "shot_000001",
  "kind": "shot",
  "startMs": 1200,
  "endMs": 4800,
  "parentSegmentId": null,
  "ordinal": 0,
  "segmentationPolicyRef": "shot-boundary/example@1",
  "provenance": {
    "derivationKey": "sha256:<64 lowercase hex>",
    "engineCacheCompatibilityId": "segment-shot-v1",
    "toolchainFingerprint": "sha256:<64 lowercase hex>"
  }
}
```

- 时间统一使用半开区间 `[startMs, endMs)`，满足 `startMs >= 0`、`endMs > startMs`。
- 同一 `kind` 按 `(startMs, endMs, segmentId)` 确定性排序；`ordinal` 必须与排序位置一致。
- `kind` 首批预留 `shot | sceneCandidate`。Shot 表示可观察的视觉边界；`sceneCandidate` 表示算法建议的语义/编辑分组，不是业务权威 Scene。
- `parentSegmentId` 只能指向同一 Manifest 中覆盖子区间的 segment；不得形成环。Shot 可属于 `sceneCandidate`，但 Host/产品层决定是否接受并映射为 Scene。
- `segmentationPolicyRef` 和 provenance 必填，确保边界可评测、可复现、可升级；policy ref 必须为 `<安全相对引用>@<版本>` 形式。preview frame 不得转换为 TemporalSegment。
- 真正的 `segment_shots` / `propose_scenes` operation、算法、模型、关键帧关系、人工修订和 Evidence 持久化留到后续协议版本。
- Seek 人工人物标注只复用时间区间约定，不复用此切分 DTO；人物、确认状态和交叠出镜区间由 Seek 管理。

### 9. Derivation and Cache Contract

Core 不持久化缓存，但 Phase 0 必须冻结同一任务在 Host 与 Core 两侧可独立计算的描述符。以下 DTO 是跨 StartRequest、completed event、Manifest、`version` 和 `doctor` 复用的公共结构，Schema 只能从这些 DTO 生成：

```text
EngineIdentity = {
  engineVersion,
  engineCommit,
  target,
  engineCacheCompatibilityId,
  supportedProtocolVersions,
  implementedOperations
}

ToolchainIdentity = {
  toolchainFingerprint,
  target,
  ffmpegVersion,
  ffprobeVersion,
  capabilitySetFingerprint
}
```

`EngineIdentity` 与 `ToolchainIdentity` 中的字段顺序和 null 语义固定；Manifest、CLI 和事件不得各自定义一套近似结构。

```text
DerivationDescriptor = {
  derivationDescriptorVersion,
  inputFingerprint,
  operation,
  operationConfigHash,
  outputContractVersion,
  engineCacheCompatibilityId,
  toolchainFingerprint
}

derivationKey = sha256(RFC8785(DerivationDescriptor))
```

canonical JSON 与摘要只接受整数、字符串、布尔、`null`、数组和对象；非整数数字被显式拒绝，协议描述符不包含浮点数。

- `requestId`、`sourceVersionId`、`executionContext`、时间戳、staging ref 和机器身份不得进入 `derivationKey`。
- `operationConfigHash` 覆盖补齐默认值后的有效配置；`outputContractVersion` 标识 operation result/Artifact 的语义版本。
- `engineCacheCompatibilityId` 与 `engineVersion` 分离。仅当修复会改变规范化结果或 Artifact 字节时递增；无输出变化的 CLI/诊断修复不应无效化缓存。
- `toolchainFingerprint` 是平台专用 `ToolchainDescriptor` 的摘要，至少覆盖 target、FFmpeg/FFprobe 版本、构建来源摘要、configure flags 摘要、能力清单摘要和动态库闭包摘要。工具链变化默认产生新 key。
- StartRequest 携带 Host 计算的 `derivationKey`；Core 重算，不匹配时拒绝。合法 `completed` 和 Manifest 原样回传 descriptor 与 key。
- Host 的持久缓存键为 `(cacheScope, derivationKey)`；`cacheScope` 至少隔离租户/用户和权限域，绝不进入 Core 协议或由 Core推断。
- 只有终态 succeeded、exit 0、Manifest/文件/Hash 全部校验通过并原子 finalize 的完整 Artifact 集可缓存。失败、取消、超时、崩溃和 partial/staged 结果不得成为命中源。
- 命中缓存时 Host 创建新的 Run/provenance，关联原缓存条目的 derivation identity；不得复用旧 `requestId`、run/generation/attempt 或跳过 current-result gate。
- 并发请求的租约、single-flight、缓存索引、保留、逐出和磁盘配额属于 Host。Core 只提供 descriptor/key 计算、Schema 和 conformance fixture。

### 10. `version` and `doctor`

Phase 0 实现：

```text
scene-core version --json
scene-core doctor --json [--bundle-root <path>]
```

`version --json` 输出：

```json
{
  "schemaVersion": "1",
  "engineVersion": "0.1.0-alpha.1",
  "engineCommit": "0123456789abcdef0123456789abcdef01234567",
  "target": "x86_64-pc-windows-msvc",
  "engineCacheCompatibilityId": "scene-core-output-v1",
  "toolchainFingerprint": "sha256:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd",
  "supportedProtocolVersions": ["0.1"],
  "implementedOperations": []
}
```

`doctor --json` 输出 `schemaVersion`、整体 `ok | failed`、上述 engine identity、tool identity 和有序 checks。每个 check 包含 `name`、`status`、`code`、安全 `message` 和 `nextStep`。

`doctor` 通过 `--trusted-manifest-sha256 <sha256:...>` 接收包外可信 manifest digest；缺失或与包内 manifest 不符时信任检查失败，绝不以包内 manifest 自证。检查顺序固定，首个失败即停止：package-manifest、manifest-trust-anchor、toolchain-descriptor、bundle-files、engine-executable、engine-version、tool-executables、tool-versions、capabilities、temp-dir。成功退出码 0；失败取首个失败 check 的稳定 code 对应退出码（`TEMP_DIR_UNAVAILABLE` 为 3，其余为 2）。CLI 无法产生正常输出时向 stdout 输出单个 `CliErrorOutput` 对象并使用错误码矩阵的退出码。

必查项：package manifest 可读、所有列出文件 size/hash 匹配、FFmpeg/FFprobe 仅从 bundle 相对路径解析、二进制可启动、报告版本与 package manifest 匹配、目标 codecs/demuxers 可列出、临时目录可创建和删除。Phase 0 不提供正式 `smoke` 命令。

### 11. FFmpeg Supply Chain and License Policy

Phase 0 先建立平台无关的供应链策略，再允许任何平台 bundle 发布：

- `packaging/toolchains/<target>/toolchain.lock.json` 锁定不可变下载 URL、下载 SHA-256、供应方、FFmpeg/FFprobe 精确版本、上游 source tag/commit、source archive hash、configure flags、license profile 和构建日期。
- `capabilities.json` 保存允许的 demuxer、decoder、encoder、filter 和 protocol 基线；CI 对实际 `-buildconf`、`-formats`、`-codecs`、`-filters`、`-protocols` 归一化后比较，缺失和意外新增都失败。
- LGPL profile 禁止 GPL/nonfree flags 和组件，附许可证正文、copyright notices、构建配置、修改说明及可获得对应源码/重链接材料的稳定说明。具体义务在首次发布前由法律/合规负责人确认，Spec 不替代法律意见。
- `ffmpeg`、`ffprobe` 及全部动态库形成闭包；不得依赖 PATH、系统安装或未在 manifest 中列出的非系统 DLL。Windows 使用可重复的 PE import 扫描加干净 VM 运行验证。
- 每个 bundle 输出 SPDX SBOM、toolchain descriptor/fingerprint 和逐文件 hash。Windows/Linux 各自锁定与发布，协议可相同但 fingerprint 和 Artifact 字节不假设一致。
- 依赖升级使用显式 PR：更新 lock、SBOM、能力 diff、许可检查、bundle 测试和变更说明。旧 bundle 与 lock 至少保留到所有消费者完成升级，允许按不可变版本回滚。
- 对 FFmpeg 和随附库建立 CVE 监控与响应等级；受影响版本不得被静默覆盖，修复以新 bundle version 发布。
- 第三方构建只有在来源、hash、配置、许可、源码可得性、能力清单和 DLL closure 全部可验证时可用；任一门禁持续不满足则切换为仓库控制的源码构建 CI。

### 12. Windows Sidecar Bundle Spike

产物名固定为：

```text
scene-core-0.1.0-alpha.1-windows-x86_64.zip
```

内容：

```text
bin/scene-core.exe
bin/ffmpeg.exe
bin/ffprobe.exe
bin/<required LGPL shared DLLs>
schemas/0.1/*
toolchain-descriptor.json
capabilities.json
package-manifest.json
SBOM.spdx.json
THIRD_PARTY_LICENSES/*
```

`package-manifest.json`：

```json
{
  "packageManifestVersion": "1",
  "name": "scene-core",
  "packageVersion": "0.1.0-alpha.1",
  "target": "x86_64-pc-windows-msvc",
  "distributionProfile": "lgpl",
  "toolchainFingerprint": "sha256:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd",
  "toolchainDescriptorRef": "toolchain-descriptor.json",
  "capabilitiesRef": "capabilities.json",
  "engine": {
    "path": "bin/scene-core.exe",
    "version": "0.1.0-alpha.1",
    "commit": "0123456789abcdef0123456789abcdef01234567",
    "engineCacheCompatibilityId": "scene-core-output-v1",
    "supportedProtocolVersions": ["0.1"],
    "implementedOperations": []
  },
  "tools": [
    {
      "name": "ffmpeg",
      "path": "bin/ffmpeg.exe",
      "version": "<pinned exact version>",
      "sourceUrl": "<pinned immutable source>",
      "sourceSha256": "sha256:<64 lowercase hex>",
      "licenseProfile": "LGPL-2.1-or-later"
    },
    {
      "name": "ffprobe",
      "path": "bin/ffprobe.exe",
      "version": "<same pinned exact version>",
      "sourceUrl": "<same pinned immutable source>",
      "sourceSha256": "sha256:<64 lowercase hex>",
      "licenseProfile": "LGPL-2.1-or-later"
    }
  ],
  "files": [
    {
      "path": "bin/scene-core.exe",
      "byteSize": 1,
      "sha256": "sha256:<64 lowercase hex>"
    },
    {
      "path": "bin/ffmpeg.exe",
      "byteSize": 1,
      "sha256": "sha256:<64 lowercase hex>"
    },
    {
      "path": "bin/ffprobe.exe",
      "byteSize": 1,
      "sha256": "sha256:<64 lowercase hex>"
    },
    {
      "path": "capabilities.json",
      "byteSize": 1,
      "sha256": "sha256:<64 lowercase hex>"
    },
    {
      "path": "toolchain-descriptor.json",
      "byteSize": 1,
      "sha256": "sha256:<64 lowercase hex>"
    }
  ],
  "sbomRef": "SBOM.spdx.json",
  "thirdPartyLicensesRef": "THIRD_PARTY_LICENSES"
}
```

- `files` 列出除 `package-manifest.json` 自身以外的全部 bundle 文件（包括 Schema、toolchain descriptor、capabilities、SBOM、许可证和 DLL），并按 path 字节序排序。
- manifest 的 SHA-256 与 ZIP SHA-256 作为 detached checksum 文件生成，不写回 manifest，避免自引用；checksum 文件可随 release metadata 分发，但不进入被 hash 的 ZIP。
- 包内 manifest 只提供逐文件完整性校验，不构成信任根。Host 或发布流程必须从包外可信 release metadata、受信任 digest 或签名验证 manifest digest，再验证 manifest 列出的文件；不依赖付费 Authenticode，也不能把包内自带 checksum 当作唯一信任来源。
- Phase 0 使用固定、不可变 URL 与 SHA-256 的第三方 LGPL shared Windows 构建作为首选 spike。SC-P0-05 必须选定并记录具体版本、构建来源、configure flags、上游源码位置和许可义务；禁止 `latest` URL、GPL 或 nonfree 组件。若门禁无法满足，SC-P0-06 必须以自建来源替代，不能降低验收标准。
- Core Phase 0 不负责 Authenticode、SmartScreen、Tauri 安装包或自动更新。

### 13. Host Integration and Cache Contract

本仓库只定义契约，不实现 Host。详细规范见 [Host 集成与缓存契约](host-cache-contract.md)：

- Host 创建每请求私有且初始为空的 staging root，并把输入放在 `input/source.media`。
- Host 在复制不可变输入快照时计算每项 `contentHash`/`byteSize`、`inputFingerprint` 和 `derivationKey`。
- Host 只能用 `(cacheScope, derivationKey)` 查询已完成、已验证、不可变的 Artifact 集；缓存命中必须创建新的 Run/provenance 并重新执行 current-result gate。
- 未命中时由 Host 持有租约和并发去重；Core 不连接缓存数据库、不管理磁盘配额，也不判断租户或用户权限。
- Host 通过 `scene-core run --staging-root <path>` 启动未来 Phase 1 引擎，并保持 stdin 打开以发送 CancelRequest。
- Host 并发排空 stdout/stderr，限制 JSONL 单行和 stderr 保存量。
- Host 校验 protocol、engine identity、request/source/execution context、sequence、terminal/exit 配对、relative ref、regular file、size 和 hash。
- 只有合法 completed + exit 0 + current-result gate + 全部文件校验通过时可注册 Artifact。
- Windows Host 的 AppContainer、Job Object、ACL、签名和安装包由 `scene-seek` 实现；Linux Host 的 namespace/seccomp/mount 由 Web Worker 实现。
- 现有 `scene-seek` 字幕复用只按 source/provider/version/input fingerprint 查找成功 Run，未覆盖 operation config、output contract 或 toolchain；该实现是后续消费者迁移依据，不是 Core `derivationKey` 的兼容约束。

## Child Issues

| ID | Title | Priority | Human Effort | Dependencies |
|---|---|---:|---:|---|
| SC-P0-01 | 初始化 Rust workspace、toolchain 与 CI | P0 | 1–2d | 无 |
| SC-P0-02 | 实现 Protocol 0.1 DTO、输入身份、Scene/Shot seam 与 derivation key | P0 | 3–4d | SC-P0-01 |
| SC-P0-03 | 生成 JSON Schema、Transcript Validator 与协议 fixture | P0 | 2–3d | SC-P0-02 |
| SC-P0-04 | 实现 `version`、`doctor` 与 package/toolchain manifest 校验 | P0 | 2d | SC-P0-01、SC-P0-03 |
| SC-P0-05 | 固化 FFmpeg 供应链、许可、能力基线与升级回滚策略 | P0 | 2–3d | SC-P0-01 |
| SC-P0-06 | 验证 Windows x64 LGPL bundle 的 DLL closure 与可复现门禁 | P0 | 2–3d | SC-P0-04、SC-P0-05 |
| SC-P0-07 | 固化 Host/cache contract 并修订跨仓库冲突文档 | P0 | 2–3d | SC-P0-02、SC-P0-03、SC-P0-05 |

## Dependency Graph

```text
SC-P0-01 ──┬─> SC-P0-02 ──> SC-P0-03 ──┬─> SC-P0-04 ──┬─> SC-P0-06
            │                         │             │
            └─> SC-P0-05 ─────────────┴─────────────┘
                                      └─> SC-P0-07
```

先固定 workspace 和工具链；协议 DTO 同时承载输入身份、Scene/Shot seam 和 derivation key。Schema、fixture 和 transcript validator 必须建立在同一 DTO 上。FFmpeg 供应链可与协议并行，但 bundle spike 必须等待 package/doctor 和供应链门禁。Host/cache Contract 最后引用最终字段和裁决规则，避免文档再次漂移。

## Acceptance Criteria

1. 根 workspace 在固定 Rust toolchain 上通过 `cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace` 和 `cargo clippy --workspace --all-targets -- -D warnings`。
2. Linux 与 Windows CI 使用同一 lockfile；CI 不下载浮动 `latest` FFmpeg 资产。
3. `schemas/0.1/` 至少包含 control message、engine event、normalized media、Artifact Manifest、TemporalSegment、InputSetDescriptor、DerivationDescriptor、version、doctor、package manifest、toolchain descriptor 和 capabilities 共十二类 Schema，且重新生成后无 diff。
4. JSONL 解析拒绝 BOM、超过 1 MiB 的行、重复 key、未知字段、第二个 start、第二个 cancel 和错误 requestId cancel。
5. validator 覆盖 accepted 从 sequence 1 开始、严格连续、恰好一个终态、终态后无事件及 terminal/exit truth table。
6. Host-facing `StartRequest` 缺失 `executionContext` 或缺少任一字段时请求无效；`version`/`doctor` CLI 按独立 Schema 验证。
7. `inputs[0].ref` 只接受 `input/source.media`；绝对路径、`..`、Windows drive、UNC 和 ADS fixture 全部失败。
8. 至少提交 10 个 valid、20 个 invalid 和 6 个 golden JSONL fixture；fixture runner 报告每个文件的预期结果。
9. `version --json` 输出通过自身 Schema，准确报告 target、commit、支持协议和空 `implementedOperations`；成功或可诊断失败均只向 stdout 输出一个 JSON object，字段和 check 顺序稳定，非零退出码与错误码矩阵一致。
10. `doctor --json` 在完整 bundle 返回 0/ok；缺失、被篡改或版本不符的 engine/tool/DLL 返回非 0/failed，并给出问题、原因和修复动作。
11. Windows CI 生成指定 ZIP；解压后无需 Rust、Python、系统 FFmpeg、PATH 修改或管理员权限即可运行 `version` 和 `doctor`。
12. package manifest 列出全部非自身文件的相对路径、size 和 SHA-256；ZIP、manifest 均生成 detached checksum。
13. bundle 只包含经过记录的 LGPL 构建和所需 DLL，并附 SPDX SBOM、第三方许可、上游源码与构建配置引用；不包含 GPL/nonfree 组件。
14. `engine-contract.md`、`roadmap.md`、`responsibilities.md`、`implementation-plan.md` 和 `cross-repository-contracts.md` 不再声明 `1.0`、`artifact_ready`、`outputFingerprint`、partial publish、Phase 0 TypeScript consumer 或 Core 实现 Tauri/AppContainer；并统一 input identity、derivation key、Scene/Shot seam 和 Host cache ownership。
15. 文档明确内部 crate、公共 sidecar、Host Adapter 和最终平台包的职责边界，并包含 PCM、Linux、Scene/Shot、服务化、硬件加速与 Rust library 的升级路径。
16. Phase 0 不实现 `run` 的媒体处理、FFprobe 媒体归一化、preview 生成、PCM、segmentation operation、消费者 Adapter 或 Tauri；只提供协议、Schema、fixture、`version`、`doctor` 和 bundle/toolchain 验证。

## Testing Plan

| Layer | What | Minimum Count |
|---|---|---:|
| Unit | ID、fingerprint、deadline、execution context、relative ref、错误和 package manifest 校验 | 24 |
| Protocol | duplicate key、unknown field、line size、framing 与 operation union | 12 |
| Transcript | sequence、terminal、cancel、EOF 与 exit truth table | 12 |
| Fixture conformance | valid/invalid/golden 文件遍历与 Schema drift | 3 runners |
| CLI integration | `version`、healthy doctor、missing/tampered/mismatched bundle | 6 |
| Windows package | build ZIP、detached checksum、clean extraction 后 version/doctor | 3 |
| Documentation | 禁止过期术语的 `rg` gate 与链接检查 | 2 |

## Rollback Plan

Phase 0 不修改数据或远程系统。若协议方向错误，删除未发布的 alpha bundle并回退相关 PR；`0.1` 尚未被消费者采用前可以整体替换。任何已被消费者固定的协议或 bundle 不允许原地覆盖，必须发布新 protocol 或 engine prerelease 版本。

## Effort Estimate

| Component | Human Effort |
|---|---:|
| Workspace、toolchain、CI | 1–2d |
| DTO、严格 parser、语义 validator | 2–3d |
| Schema、fixture、transcript validator | 2d |
| Version/doctor/package validation | 2d |
| FFmpeg 供应链、许可与能力基线 | 2–3d |
| Windows LGPL bundle reproducibility spike | 2–3d |
| Host/cache Contract 与文档统一 | 2–3d |
| Total | 13–18d |

外部依赖：建立有效 Git 仓库/远程、修复 GitHub CLI 认证，以及可执行 Windows GitHub Actions。它们不阻止本地 Spec 和 Ticket 生成，但会阻止 CI 验收。

## Files Reference

| File | Change |
|---|---|
| `Cargo.toml` | 新建 workspace 与统一依赖配置 |
| `rust-toolchain.toml` | 固定精确 Rust toolchain |
| `crates/scene-core-protocol/**` | 新建 DTO、严格解析、validator 与 Schema 导出 |
| `crates/scene-core-engine/**` | 新建 `version`、`doctor` CLI |
| `schemas/0.1/**` | 新建生成式 JSON Schema |
| `fixtures/protocol/**` | 新建 valid/invalid/golden fixture |
| `packaging/toolchains/** 和 packaging/bundles/windows-x86_64/**` | 新建 bundle 锁定、打包和许可配置 |
| `.github/workflows/ci.yml` | 新建 Linux/Windows CI |
| `docs/architecture/repositories/scene-core/engine-contract.md` | 替换为 0.1 契约 |
| `docs/architecture/repositories/scene-core/roadmap.md` | 改为消费者驱动阶段，不预定 PCM |
| `docs/architecture/repositories/scene-core/responsibilities.md` | 区分 Core、Host 和平台职责 |
| `docs/architecture/repositories/scene-core/implementation-plan.md` | 移除 Seek 实现和 Core 外部门禁 |
| `docs/architecture/repositories/cross-repository-contracts.md` | 同步候选 0.1 字段与兼容语义 |

## Out of Scope

- `scene-seek`、Web RAG 或其他仓库的任何代码和 Ticket。
- Tauri、AppContainer、Job Object、NTFS ACL、安装包、SmartScreen 和 Authenticode。
- `run` 的媒体操作执行、真实 `probe`、preview、PCM 和正式 `smoke`。
- TypeScript 影子消费者和第二消费者 conformance。
- crates.io、公共 Rust API 兼容承诺。
- Linux 正式 bundle、macOS、容器和服务部署。
- ASR、OCR、VLM、Embedding、Rerank、Generation、数据库、对象存储、队列和权限。

## Definition of Done

1. SC-P0-01 至 SC-P0-07 的验收条件全部通过。
2. 聚焦 Eng Review 确认协议、版本、bundle 与 Host 边界不存在阻断项。
3. Phase 1 可以只依据本规格和生成的 Schema/fixture 开始实现，无需重新决定消息、版本、路径或发布结构。
