# Protocol 0.1 Schema 全量参考

> 由 `schemas/0.1/*.json` 渲染（`scripts/schema-reference.py`）。Schema 本身由 Rust DTO 生成，`scripts/check-schema-drift.sh` 阻止漂移。
> 快照：`41d9a5a`（2026-09-16）。JSON Schema Draft 2020-12；所有对象默认拒绝未知字段。
> `类型` 列中 `int | null` 表示未知值必须显式为 null；`format=uint64` 表示非负整数。

## `control-message.json` — Host→engine 控制消息（StartRequest / CancelRequest 的 oneOf）

**ControlMessageSchema** — anyOf: `StartRequest`, `CancelRequest`

### 共享定义（`$defs`）

**CancelMessageType** — string: enum `cancel`

**CancelRequest**

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `engineProtocolVersion` | → `ProtocolVersion` | 是 |
| `messageType` | → `CancelMessageType` | 是 |
| `requestId` | → `Identifier` | 是 |

未知字段拒绝（`additionalProperties: false`）。

**ExecutionContext**

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `attempt` | int<br>format=uint64<br>min=0 | 是 |
| `generation` | int<br>format=uint64<br>min=0 | 是 |
| `runId` | → `Identifier` | 是 |
| `scope` | → `ExecutionScope` | 是 |

未知字段拒绝（`additionalProperties: false`）。

**ExecutionScope** — Request origin scope. `cacheScope` never enters the engine protocol. — string: enum `asset`, `query`, `evaluation`

**Identifier** — string: pattern=`^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$`

**InputDescriptor** — One staged logical input.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `byteSize` | int<br>format=uint64<br>min=0 | 是 |
| `contentHash` | → `Sha256Digest` | 是 |
| `ref` | → `InputRef` | 是 |
| `role` | → `InputRole` | 是 |

未知字段拒绝（`additionalProperties: false`）。

**InputRef** — string: const `input/source.media`

**InputRole** — string: const `source_media`

**Operation** — Operations frozen by Protocol 0.1. — string: enum `probe`, `extract_preview`

**OperationOptions** — Caller-provided operation options.

Protocol 0.1 registers no configurable options: the only accepted wire
shape is `{}`. Unknown keys, duplicate keys and non-object shapes are
rejected while parsing, so no free-form options map can enter the protocol. — object (no fields)

**OutputContractVersion** — string: pattern=`^[a-z0-9-]+/[0-9]+$`

**ProtocolVersion** — string: const `0.1`

**Sha256Digest** — string: pattern=`^sha256:[0-9a-f]{64}$`

**StartMessageType** — string: enum `start`

**StartRequest** — Host-facing start request. `executionContext` is required; `version` and
`doctor` never use this DTO.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `deadlineMs` | int | null<br>format=uint64<br>min=0 | 否 |
| `derivationKey` | → `Sha256Digest` | 是 |
| `engineProtocolVersion` | → `ProtocolVersion` | 是 |
| `executionContext` | → `ExecutionContext` | 是 |
| `inputFingerprint` | → `Sha256Digest` | 是 |
| `inputs` | array<br>items: → `InputDescriptor` | 是 |
| `messageType` | → `StartMessageType` | 是 |
| `operation` | → `Operation` | 是 |
| `operationConfigHash` | → `Sha256Digest` | 是 |
| `options` | → `OperationOptions` | 是 |
| `outputContractVersion` | → `OutputContractVersion` | 是 |
| `requestId` | → `Identifier` | 是 |
| `sourceVersionId` | → `Identifier` | 是 |

未知字段拒绝（`additionalProperties: false`）。

## `engine-event.json` — engine→Host 事件 Envelope（含 operation result union）

**EventEnvelope** — One engine event. Optional body fields are omitted from the wire form when
they do not apply to the event type.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `completed` | int | null<br>format=uint64<br>min=0 | 否 |
| `derivationKey` | → `Sha256Digest` | 是 |
| `engine` | → `EngineIdentity` | 是 |
| `engineProtocolVersion` | → `ProtocolVersion` | 是 |
| `error` | → `ProtocolError` | null | 否 |
| `eventType` | → `EventType` | 是 |
| `executionContext` | → `ExecutionContext` | 是 |
| `inputFingerprint` | → `Sha256Digest` | 是 |
| `messageType` | → `EventMessageType` | 是 |
| `occurredAt` | → `UtcTimestamp` | 是 |
| `operation` | → `Operation` | 是 |
| `requestId` | → `Identifier` | 是 |
| `result` | → `OperationResult` | null | 否 |
| `sequence` | int<br>format=uint64<br>min=0 | 是 |
| `sourceVersionId` | → `Identifier` | 是 |
| `stage` | → `StageName` | null | 否 |
| `total` | int | null<br>format=uint64<br>min=0 | 否 |

未知字段拒绝（`additionalProperties: false`）。

### 共享定义（`$defs`）

**Artifact**

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `artifactId` | → `Identifier` | 是 |
| `byteSize` | int<br>format=uint64<br>min=0 | 是 |
| `contentHash` | → `Sha256Digest` | 是 |
| `kind` | → `ArtifactKind` | 是 |
| `mediaType` | → `MediaType` | 是 |
| `pixelHeight` | int | null<br>format=uint32<br>min=0 | 否 |
| `pixelWidth` | int | null<br>format=uint32<br>min=0 | 否 |
| `presentationTimeMs` | int | null<br>format=uint64<br>min=0 | 否 |
| `relativeRef` | → `RelativeRef` | 是 |
| `requestedTimeMs` | int<br>format=uint64<br>min=0 | 是 |
| `role` | → `ArtifactRole` | 是 |

未知字段拒绝（`additionalProperties: false`）。

**ArtifactKind** — Artifact kinds frozen by Protocol 0.1. — string: enum `previewFrame`

**ArtifactManifest** — Manifest of one immutable Artifact set. Contains no timestamp or resource
statistics; `probe` never produces one.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `artifacts` | array<br>items: → `Artifact` | 是 |
| `derivationKey` | → `Sha256Digest` | 是 |
| `engine` | → `EngineIdentity` | 是 |
| `inputFingerprint` | → `Sha256Digest` | 是 |
| `manifestVersion` | → `ProtocolVersion` | 是 |
| `operation` | → `Operation` | 是 |
| `operationConfigHash` | → `Sha256Digest` | 是 |
| `outputContractVersion` | → `OutputContractVersion` | 是 |
| `requestId` | → `Identifier` | 是 |
| `sourceVersionId` | → `Identifier` | 是 |
| `toolchainFingerprint` | → `Sha256Digest` | 是 |

未知字段拒绝（`additionalProperties: false`）。

**ArtifactRole** — Fixed diagnostic preview roles. — string: enum `opening`, `midpoint`

**CacheCompatibilityId** — string: pattern=`^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`

**CommitHash** — string: pattern=`^[0-9a-f]{40}$`

**ContainerInfo**

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `bitRateBps` | int | null<br>format=uint64<br>min=0 | 否 |
| `durationMs` | int | null<br>format=uint64<br>min=0 | 否 |
| `fileSizeBytes` | int<br>format=uint64<br>min=0 | 是 |
| `formatName` | string | 是 |
| `startTimeMs` | int | null<br>format=uint64<br>min=0<br>max=0 | 否 |

未知字段拒绝（`additionalProperties: false`）。

**EngineIdentity** — Shared identity of the engine build. Reused by events, Manifests, `version`
and `doctor`; no consumer may define an approximate variant.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `engineCacheCompatibilityId` | → `CacheCompatibilityId` | 是 |
| `engineCommit` | → `CommitHash` | 是 |
| `engineVersion` | string | 是 |
| `implementedOperations` | array<br>items: → `Operation` | 是 |
| `supportedProtocolVersions` | array<br>items: → `ProtocolVersion` | 是 |
| `target` | string | 是 |

未知字段拒绝（`additionalProperties: false`）。

**ErrorCode** — Frozen Protocol 0.1 error codes. — string: enum `INVALID_REQUEST`, `UNSUPPORTED_PROTOCOL`, `OPERATION_UNAVAILABLE`, `INPUT_NOT_FOUND`, `INPUT_CHANGED`, `UNSUPPORTED_INPUT`, `CORRUPT_MEDIA`, `MISSING_VIDEO_STREAM`, `RESOURCE_LIMIT`, `TOOL_UNAVAILABLE`, `TOOL_FAILED`, `TIMEOUT`, `CANCELLED`, `ENGINE_INTERNAL`

**EventMessageType** — string: enum `event`

**EventType** — string: enum `accepted`, `running`, `progress`, `completed`, `failed`, `cancelled`, `timed_out`

**ExecutionContext**

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `attempt` | int<br>format=uint64<br>min=0 | 是 |
| `generation` | int<br>format=uint64<br>min=0 | 是 |
| `runId` | → `Identifier` | 是 |
| `scope` | → `ExecutionScope` | 是 |

未知字段拒绝（`additionalProperties: false`）。

**ExecutionScope** — Request origin scope. `cacheScope` never enters the engine protocol. — string: enum `asset`, `query`, `evaluation`

**Identifier** — string: pattern=`^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$`

**MediaStream**

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `attachedPicture` | bool | 是 |
| `averageFrameRate` | → `Rational` | null | 否 |
| `channelLayout` | string | null | 否 |
| `channels` | int | null<br>format=uint32<br>min=0 | 否 |
| `codecName` | string | 是 |
| `codedHeight` | int | null<br>format=uint32<br>min=0 | 否 |
| `codedWidth` | int | null<br>format=uint32<br>min=0 | 否 |
| `defaultDisposition` | bool | 是 |
| `displayHeight` | int | null<br>format=uint32<br>min=0 | 否 |
| `displayWidth` | int | null<br>format=uint32<br>min=0 | 否 |
| `durationMs` | int | null<br>format=uint64<br>min=0 | 否 |
| `index` | int<br>format=uint32<br>min=0 | 是 |
| `kind` | → `StreamKind` | 是 |
| `rotationDegrees` | int | null<br>format=int32 | 否 |
| `sampleRate` | int | null<br>format=uint32<br>min=0 | 否 |
| `startTimeMs` | int | null<br>format=int64 | 否 |
| `timeBase` | → `Rational` | null | 否 |

未知字段拒绝（`additionalProperties: false`）。

**MediaType** — string: pattern=`^[a-z0-9.+-_]+/[a-z0-9.+-_]+$`

**NormalizedMedia**

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `container` | → `ContainerInfo` | 是 |
| `primaryVideoStreamIndex` | int | null<br>format=uint32<br>min=0 | 否 |
| `streams` | array<br>items: → `MediaStream` | 是 |

未知字段拒绝（`additionalProperties: false`）。

**Operation** — Operations frozen by Protocol 0.1. — string: enum `probe`, `extract_preview`

**OperationResult** — Tagged union of completed results, tagged by `operation`. — oneOf: `{"additionalProperties": false, "properties": {"media": {"$ref": "#/$defs/NormalizedMedia"}, "operation": {"const": "probe", "type": "string"}, "resourceUsage": {"$ref": "#/$defs/ResourceUsage"}}, "required": ["operation", "media", "resourceUsage"], "type": "object"}`, `{"additionalProperties": false, "properties": {"artifactManifest": {"$ref": "#/$defs/ArtifactManifest"}, "media": {"$ref": "#/$defs/NormalizedMedia"}, "operation": {"const": "extract_preview", "type": "string"}, "resourceUsage": {"$ref": "#/$defs/ResourceUsage"}}, "required": ["operation", "media", "artifactManifest", "resourceUsage"], "type": "object"}`

**OutputContractVersion** — string: pattern=`^[a-z0-9-]+/[0-9]+$`

**ProtocolError** — Wire error payload. Only the fixed contract fields are allowed.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `actual` | int | null<br>format=int64 | 否 |
| `cause` | string | null | 否 |
| `code` | → `ErrorCode` | 是 |
| `limit` | int | null<br>format=int64 | 否 |
| `message` | string | 是 |
| `nextStep` | string | null | 否 |
| `retryable` | bool | 是 |
| `stage` | string | null | 否 |

未知字段拒绝（`additionalProperties: false`）。

**ProtocolVersion** — string: const `0.1`

**Rational** — Wire rational number, e.g. a time base. The denominator must be positive.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `den` | int<br>format=int64 | 是 |
| `num` | int<br>format=int64 | 是 |

未知字段拒绝（`additionalProperties: false`）。

**RelativeRef** — string: minLen=1; maxLen=1024

**ResourceUsage** — Resource accounting attached to a completed result. Phase 0 freezes the
field boundary; Phase 1 fills the values.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `cpuTimeMs` | int | null<br>format=uint64<br>min=0 | 否 |
| `peakMemoryBytes` | int | null<br>format=uint64<br>min=0 | 否 |
| `wallTimeMs` | int<br>format=uint64<br>min=0 | 是 |

未知字段拒绝（`additionalProperties: false`）。

**Sha256Digest** — string: pattern=`^sha256:[0-9a-f]{64}$`

**StageName** — string: pattern=`^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`

**StreamKind** — Stream kinds recognized by the normalized media DTO. — string: enum `video`, `audio`, `subtitle`, `data`, `attachment`

**UtcTimestamp** — string: format=date-time

## `normalized-media.json` — 归一化媒体元数据（probe / extract_preview 共用）

**NormalizedMedia**

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `container` | → `ContainerInfo` | 是 |
| `primaryVideoStreamIndex` | int | null<br>format=uint32<br>min=0 | 否 |
| `streams` | array<br>items: → `MediaStream` | 是 |

未知字段拒绝（`additionalProperties: false`）。

### 共享定义（`$defs`）

**ContainerInfo**

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `bitRateBps` | int | null<br>format=uint64<br>min=0 | 否 |
| `durationMs` | int | null<br>format=uint64<br>min=0 | 否 |
| `fileSizeBytes` | int<br>format=uint64<br>min=0 | 是 |
| `formatName` | string | 是 |
| `startTimeMs` | int | null<br>format=uint64<br>min=0<br>max=0 | 否 |

未知字段拒绝（`additionalProperties: false`）。

**MediaStream**

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `attachedPicture` | bool | 是 |
| `averageFrameRate` | → `Rational` | null | 否 |
| `channelLayout` | string | null | 否 |
| `channels` | int | null<br>format=uint32<br>min=0 | 否 |
| `codecName` | string | 是 |
| `codedHeight` | int | null<br>format=uint32<br>min=0 | 否 |
| `codedWidth` | int | null<br>format=uint32<br>min=0 | 否 |
| `defaultDisposition` | bool | 是 |
| `displayHeight` | int | null<br>format=uint32<br>min=0 | 否 |
| `displayWidth` | int | null<br>format=uint32<br>min=0 | 否 |
| `durationMs` | int | null<br>format=uint64<br>min=0 | 否 |
| `index` | int<br>format=uint32<br>min=0 | 是 |
| `kind` | → `StreamKind` | 是 |
| `rotationDegrees` | int | null<br>format=int32 | 否 |
| `sampleRate` | int | null<br>format=uint32<br>min=0 | 否 |
| `startTimeMs` | int | null<br>format=int64 | 否 |
| `timeBase` | → `Rational` | null | 否 |

未知字段拒绝（`additionalProperties: false`）。

**Rational** — Wire rational number, e.g. a time base. The denominator must be positive.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `den` | int<br>format=int64 | 是 |
| `num` | int<br>format=int64 | 是 |

未知字段拒绝（`additionalProperties: false`）。

**StreamKind** — Stream kinds recognized by the normalized media DTO. — string: enum `video`, `audio`, `subtitle`, `data`, `attachment`

## `artifact-manifest.json` — Artifact Manifest（extract_preview）

**ArtifactManifest** — Manifest of one immutable Artifact set. Contains no timestamp or resource
statistics; `probe` never produces one.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `artifacts` | array<br>items: → `Artifact` | 是 |
| `derivationKey` | → `Sha256Digest` | 是 |
| `engine` | → `EngineIdentity` | 是 |
| `inputFingerprint` | → `Sha256Digest` | 是 |
| `manifestVersion` | → `ProtocolVersion` | 是 |
| `operation` | → `Operation` | 是 |
| `operationConfigHash` | → `Sha256Digest` | 是 |
| `outputContractVersion` | → `OutputContractVersion` | 是 |
| `requestId` | → `Identifier` | 是 |
| `sourceVersionId` | → `Identifier` | 是 |
| `toolchainFingerprint` | → `Sha256Digest` | 是 |

未知字段拒绝（`additionalProperties: false`）。

### 共享定义（`$defs`）

**Artifact**

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `artifactId` | → `Identifier` | 是 |
| `byteSize` | int<br>format=uint64<br>min=0 | 是 |
| `contentHash` | → `Sha256Digest` | 是 |
| `kind` | → `ArtifactKind` | 是 |
| `mediaType` | → `MediaType` | 是 |
| `pixelHeight` | int | null<br>format=uint32<br>min=0 | 否 |
| `pixelWidth` | int | null<br>format=uint32<br>min=0 | 否 |
| `presentationTimeMs` | int | null<br>format=uint64<br>min=0 | 否 |
| `relativeRef` | → `RelativeRef` | 是 |
| `requestedTimeMs` | int<br>format=uint64<br>min=0 | 是 |
| `role` | → `ArtifactRole` | 是 |

未知字段拒绝（`additionalProperties: false`）。

**ArtifactKind** — Artifact kinds frozen by Protocol 0.1. — string: enum `previewFrame`

**ArtifactRole** — Fixed diagnostic preview roles. — string: enum `opening`, `midpoint`

**CacheCompatibilityId** — string: pattern=`^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`

**CommitHash** — string: pattern=`^[0-9a-f]{40}$`

**EngineIdentity** — Shared identity of the engine build. Reused by events, Manifests, `version`
and `doctor`; no consumer may define an approximate variant.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `engineCacheCompatibilityId` | → `CacheCompatibilityId` | 是 |
| `engineCommit` | → `CommitHash` | 是 |
| `engineVersion` | string | 是 |
| `implementedOperations` | array<br>items: → `Operation` | 是 |
| `supportedProtocolVersions` | array<br>items: → `ProtocolVersion` | 是 |
| `target` | string | 是 |

未知字段拒绝（`additionalProperties: false`）。

**Identifier** — string: pattern=`^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$`

**MediaType** — string: pattern=`^[a-z0-9.+-_]+/[a-z0-9.+-_]+$`

**Operation** — Operations frozen by Protocol 0.1. — string: enum `probe`, `extract_preview`

**OutputContractVersion** — string: pattern=`^[a-z0-9-]+/[0-9]+$`

**ProtocolVersion** — string: const `0.1`

**RelativeRef** — string: minLen=1; maxLen=1024

**Sha256Digest** — string: pattern=`^sha256:[0-9a-f]{64}$`

## `temporal-segment.json` — Scene/Shot seam：候选时间分段

**TemporalSegment** — Half-open `[startMs, endMs)` candidate interval. Public times are
non-negative; parents must cover their children.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `endMs` | int<br>format=uint64<br>min=0 | 是 |
| `kind` | → `TemporalSegmentKind` | 是 |
| `ordinal` | int<br>format=uint64<br>min=0 | 是 |
| `parentSegmentId` | → `Identifier` | null | 否 |
| `provenance` | → `SegmentProvenance` | 是 |
| `segmentId` | → `Identifier` | 是 |
| `segmentationPolicyRef` | → `PolicyRef` | 是 |
| `startMs` | int<br>format=uint64<br>min=0 | 是 |

未知字段拒绝（`additionalProperties: false`）。

### 共享定义（`$defs`）

**CacheCompatibilityId** — string: pattern=`^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`

**Identifier** — string: pattern=`^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$`

**PolicyRef** — string: pattern=`^[^@]+@[A-Za-z0-9._-]+$`; maxLen=200

**SegmentProvenance** — Required reproducibility evidence for a segmentation policy.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `derivationKey` | → `Sha256Digest` | 是 |
| `engineCacheCompatibilityId` | → `CacheCompatibilityId` | 是 |
| `toolchainFingerprint` | → `Sha256Digest` | 是 |

未知字段拒绝（`additionalProperties: false`）。

**Sha256Digest** — string: pattern=`^sha256:[0-9a-f]{64}$`

**TemporalSegmentKind** — Segment kinds frozen by Protocol 0.1. — string: enum `shot`, `sceneCandidate`

## `input-set-descriptor.json` — 逻辑输入集合描述符（inputFingerprint 的 canonical 源）

**InputSetDescriptor** — Versioned logical input set. `0.1` allows exactly one `source_media`.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `inputSetVersion` | → `DescriptorVersion` | 是 |
| `inputs` | array<br>items: → `InputDescriptor` | 是 |

未知字段拒绝（`additionalProperties: false`）。

### 共享定义（`$defs`）

**DescriptorVersion** — Frozen descriptor schema revision. Protocol 0.1 registers only `"1"` for
both `inputSetVersion` and `derivationDescriptorVersion`. — string: enum `1`

**InputDescriptor** — One staged logical input.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `byteSize` | int<br>format=uint64<br>min=0 | 是 |
| `contentHash` | → `Sha256Digest` | 是 |
| `ref` | → `InputRef` | 是 |
| `role` | → `InputRole` | 是 |

未知字段拒绝（`additionalProperties: false`）。

**InputRef** — string: const `input/source.media`

**InputRole** — string: const `source_media`

**Sha256Digest** — string: pattern=`^sha256:[0-9a-f]{64}$`

## `derivation-descriptor.json` — 缓存推导描述符（derivationKey 的 canonical 源）

**DerivationDescriptor** — Cache derivation input. Excludes request/run/source/execution identity so
the same logical media work derives the same key on Host and Core.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `derivationDescriptorVersion` | → `DescriptorVersion` | 是 |
| `engineCacheCompatibilityId` | → `CacheCompatibilityId` | 是 |
| `inputFingerprint` | → `Sha256Digest` | 是 |
| `operation` | → `Operation` | 是 |
| `operationConfigHash` | → `Sha256Digest` | 是 |
| `outputContractVersion` | → `OutputContractVersion` | 是 |
| `toolchainFingerprint` | → `Sha256Digest` | 是 |

未知字段拒绝（`additionalProperties: false`）。

### 共享定义（`$defs`）

**CacheCompatibilityId** — string: pattern=`^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`

**DescriptorVersion** — Frozen descriptor schema revision. Protocol 0.1 registers only `"1"` for
both `inputSetVersion` and `derivationDescriptorVersion`. — string: enum `1`

**Operation** — Operations frozen by Protocol 0.1. — string: enum `probe`, `extract_preview`

**OutputContractVersion** — string: pattern=`^[a-z0-9-]+/[0-9]+$`

**Sha256Digest** — string: pattern=`^sha256:[0-9a-f]{64}$`

## `engine-identity.json` — 共享引擎身份（事件 / Manifest / version / doctor 复用）

**EngineIdentity** — Shared identity of the engine build. Reused by events, Manifests, `version`
and `doctor`; no consumer may define an approximate variant.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `engineCacheCompatibilityId` | → `CacheCompatibilityId` | 是 |
| `engineCommit` | → `CommitHash` | 是 |
| `engineVersion` | string | 是 |
| `implementedOperations` | array<br>items: → `Operation` | 是 |
| `supportedProtocolVersions` | array<br>items: → `ProtocolVersion` | 是 |
| `target` | string | 是 |

未知字段拒绝（`additionalProperties: false`）。

### 共享定义（`$defs`）

**CacheCompatibilityId** — string: pattern=`^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`

**CommitHash** — string: pattern=`^[0-9a-f]{40}$`

**Operation** — Operations frozen by Protocol 0.1. — string: enum `probe`, `extract_preview`

**ProtocolVersion** — string: const `0.1`

## `toolchain-identity.json` — 共享工具链身份（version / doctor / Manifest 指纹来源）

**ToolchainIdentity** — Shared fixed-toolchain identity. The fingerprint covers target, FFmpeg and
FFprobe versions, build source, configure flags, capabilities and the
dynamic library closure.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `capabilitySetFingerprint` | → `Sha256Digest` | 是 |
| `ffmpegVersion` | string | 是 |
| `ffprobeVersion` | string | 是 |
| `target` | string | 是 |
| `toolchainFingerprint` | → `Sha256Digest` | 是 |

未知字段拒绝（`additionalProperties: false`）。

### 共享定义（`$defs`）

**Sha256Digest** — string: pattern=`^sha256:[0-9a-f]{64}$`

## `version.json` — `version --json` 输出

**VersionOutput** — `version --json` output. Mirrors the shared `EngineIdentity` plus the
toolchain fingerprint and a CLI schema version.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `engineCacheCompatibilityId` | → `CacheCompatibilityId` | 是 |
| `engineCommit` | → `CommitHash` | 是 |
| `engineVersion` | string | 是 |
| `implementedOperations` | array<br>items: → `Operation` | 是 |
| `schemaVersion` | → `DescriptorVersion` | 是 |
| `supportedProtocolVersions` | array<br>items: → `ProtocolVersion` | 是 |
| `target` | string | 是 |
| `toolchainFingerprint` | → `Sha256Digest` | 是 |

未知字段拒绝（`additionalProperties: false`）。

### 共享定义（`$defs`）

**CacheCompatibilityId** — string: pattern=`^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`

**CommitHash** — string: pattern=`^[0-9a-f]{40}$`

**DescriptorVersion** — Frozen descriptor schema revision. Protocol 0.1 registers only `"1"` for
both `inputSetVersion` and `derivationDescriptorVersion`. — string: enum `1`

**Operation** — Operations frozen by Protocol 0.1. — string: enum `probe`, `extract_preview`

**ProtocolVersion** — string: const `0.1`

**Sha256Digest** — string: pattern=`^sha256:[0-9a-f]{64}$`

## `doctor.json` — `doctor --json` 输出（有序 checks）

**DoctorOutput** — `doctor --json` output with ordered, stable checks.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `checks` | array<br>items: → `DoctorCheck` | 是 |
| `engine` | → `EngineIdentity` | 是 |
| `schemaVersion` | → `DescriptorVersion` | 是 |
| `status` | → `DoctorStatus` | 是 |
| `toolchain` | → `ToolchainIdentity` | null | 否 |

未知字段拒绝（`additionalProperties: false`）。

### 共享定义（`$defs`）

**CacheCompatibilityId** — string: pattern=`^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`

**CommitHash** — string: pattern=`^[0-9a-f]{40}$`

**DescriptorVersion** — Frozen descriptor schema revision. Protocol 0.1 registers only `"1"` for
both `inputSetVersion` and `derivationDescriptorVersion`. — string: enum `1`

**DoctorCheck**

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `code` | → `DoctorCheckCode` | null | 否 |
| `message` | string | 是 |
| `name` | → `DoctorCheckName` | 是 |
| `nextStep` | string | null | 否 |
| `status` | → `DoctorStatus` | 是 |

未知字段拒绝（`additionalProperties: false`）。

**DoctorCheckCode** — Stable doctor failure codes. — string: enum `PACKAGE_MANIFEST_UNREADABLE`, `PACKAGE_MANIFEST_INVALID`, `TRUST_ANCHOR_MISSING`, `MANIFEST_DIGEST_MISMATCH`, `BUNDLE_FILE_INVALID`, `TOOLCHAIN_DESCRIPTOR_INVALID`, `ENGINE_EXECUTABLE_INVALID`, `ENGINE_VERSION_MISMATCH`, `TOOL_EXECUTABLE_INVALID`, `TOOL_VERSION_MISMATCH`, `CAPABILITIES_INVALID`, `TEMP_DIR_UNAVAILABLE`

**DoctorCheckName** — string: pattern=`^[a-z][a-z0-9-]{0,63}$`

**DoctorStatus** — string: enum `ok`, `failed`

**EngineIdentity** — Shared identity of the engine build. Reused by events, Manifests, `version`
and `doctor`; no consumer may define an approximate variant.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `engineCacheCompatibilityId` | → `CacheCompatibilityId` | 是 |
| `engineCommit` | → `CommitHash` | 是 |
| `engineVersion` | string | 是 |
| `implementedOperations` | array<br>items: → `Operation` | 是 |
| `supportedProtocolVersions` | array<br>items: → `ProtocolVersion` | 是 |
| `target` | string | 是 |

未知字段拒绝（`additionalProperties: false`）。

**Operation** — Operations frozen by Protocol 0.1. — string: enum `probe`, `extract_preview`

**ProtocolVersion** — string: const `0.1`

**Sha256Digest** — string: pattern=`^sha256:[0-9a-f]{64}$`

**ToolchainIdentity** — Shared fixed-toolchain identity. The fingerprint covers target, FFmpeg and
FFprobe versions, build source, configure flags, capabilities and the
dynamic library closure.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `capabilitySetFingerprint` | → `Sha256Digest` | 是 |
| `ffmpegVersion` | string | 是 |
| `ffprobeVersion` | string | 是 |
| `target` | string | 是 |
| `toolchainFingerprint` | → `Sha256Digest` | 是 |

未知字段拒绝（`additionalProperties: false`）。

## `package-manifest.json` — bundle 包内 manifest（逐文件完整性）

**PackageManifest** — Bundle package manifest. Never lists itself and never carries a timestamp.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `capabilitiesRef` | → `RelativeRef` | 是 |
| `distributionProfile` | → `DistributionProfile` | 是 |
| `engine` | → `PackagedEngine` | 是 |
| `files` | array<br>items: → `PackagedFile` | 是 |
| `name` | string | 是 |
| `packageManifestVersion` | → `DescriptorVersion` | 是 |
| `packageVersion` | string | 是 |
| `sbomRef` | → `RelativeRef` | 是 |
| `target` | string | 是 |
| `thirdPartyLicensesRef` | → `RelativeRef` | 是 |
| `toolchainDescriptorRef` | → `RelativeRef` | 是 |
| `toolchainFingerprint` | → `Sha256Digest` | 是 |
| `tools` | array<br>items: → `PackagedTool` | 是 |

未知字段拒绝（`additionalProperties: false`）。

### 共享定义（`$defs`）

**CacheCompatibilityId** — string: pattern=`^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`

**CommitHash** — string: pattern=`^[0-9a-f]{40}$`

**DescriptorVersion** — Frozen descriptor schema revision. Protocol 0.1 registers only `"1"` for
both `inputSetVersion` and `derivationDescriptorVersion`. — string: enum `1`

**DistributionProfile** — Distribution license profile of the bundle. — string: enum `lgpl`

**LicenseExpression** — string: pattern=`^[A-Za-z0-9.()+-]+( [A-Za-z0-9.()+-]+)*$`

**Operation** — Operations frozen by Protocol 0.1. — string: enum `probe`, `extract_preview`

**PackagedEngine** — Engine entry of the package manifest.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `commit` | → `CommitHash` | 是 |
| `engineCacheCompatibilityId` | → `CacheCompatibilityId` | 是 |
| `implementedOperations` | array<br>items: → `Operation` | 是 |
| `path` | → `RelativeRef` | 是 |
| `supportedProtocolVersions` | array<br>items: → `ProtocolVersion` | 是 |
| `version` | string | 是 |

未知字段拒绝（`additionalProperties: false`）。

**PackagedFile** — One file listed with its integrity data.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `byteSize` | int<br>format=uint64<br>min=0 | 是 |
| `path` | → `RelativeRef` | 是 |
| `sha256` | → `Sha256Digest` | 是 |

未知字段拒绝（`additionalProperties: false`）。

**PackagedTool** — Fixed tool entry of the package manifest.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `licenseProfile` | → `LicenseExpression` | 是 |
| `name` | → `ToolName` | 是 |
| `path` | → `RelativeRef` | 是 |
| `sourceSha256` | → `Sha256Digest` | 是 |
| `sourceUrl` | string | 是 |
| `version` | string | 是 |

未知字段拒绝（`additionalProperties: false`）。

**ProtocolVersion** — string: const `0.1`

**RelativeRef** — string: minLen=1; maxLen=1024

**Sha256Digest** — string: pattern=`^sha256:[0-9a-f]{64}$`

**ToolName** — string: pattern=`^[a-z][a-z0-9-]{0,31}$`

## `cli-error.json` — CLI 无法产生正常输出时的单个错误对象

**CliErrorOutput** — Controlled diagnostic failure for the standalone CLI. The engine writes
exactly one such object to stdout when it cannot produce a normal command
output, then exits non-zero.

| 字段 | 类型 / 约束 | 必填 |
|---|---|---|
| `code` | → `ErrorCode` | 是 |
| `message` | string | 是 |
| `nextStep` | string | 是 |
| `schemaVersion` | → `DescriptorVersion` | 是 |

未知字段拒绝（`additionalProperties: false`）。

### 共享定义（`$defs`）

**DescriptorVersion** — Frozen descriptor schema revision. Protocol 0.1 registers only `"1"` for
both `inputSetVersion` and `derivationDescriptorVersion`. — string: enum `1`

**ErrorCode** — Frozen Protocol 0.1 error codes. — string: enum `INVALID_REQUEST`, `UNSUPPORTED_PROTOCOL`, `OPERATION_UNAVAILABLE`, `INPUT_NOT_FOUND`, `INPUT_CHANGED`, `UNSUPPORTED_INPUT`, `CORRUPT_MEDIA`, `MISSING_VIDEO_STREAM`, `RESOURCE_LIMIT`, `TOOL_UNAVAILABLE`, `TOOL_FAILED`, `TIMEOUT`, `CANCELLED`, `ENGINE_INTERNAL`
