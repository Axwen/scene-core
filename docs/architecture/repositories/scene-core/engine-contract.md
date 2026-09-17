# `scene-core` 引擎调用协议 Protocol 0.1

> 本文是 Host-facing JSON/JSONL 契约的消费侧摘要。Rust DTO 是结构事实源，Schema 与 fixture 必须从 DTO 生成；本文不得定义第二套近似字段。
>
> 生成式 Schema 全量参考（字段、必填与约束）：[schema-0.1-reference.md](../../../specs/schema-0.1-reference.md)。

## 1. 传输与边界

- 一条 Host 请求进入 stdin 或等价的受控进程通道；事件逐条写 stdout 并 flush。
- 媒体和 Artifact 字节不内嵌 JSON，由 Host 管理私有 staging。staging 内不得出现指向根外的符号链接/reparse：`input/source.media` 及其父目录以及每个待写入路径（含从 Artifact 派生的临时文件，如 `opening.tmp`）的任一层是符号链接时受控失败，不跟随链接读取或写入。
- 请求的单行 JSONL 必须是有效的 UTF-8 且有 1 MiB 硬上限；读取阶段即按上限截断，超限或非 UTF-8 的行按 framing 错误处理，不会先整行分配再校验。
- stderr 只用于受控诊断，不是稳定协议；不得输出绝对路径、完整 argv、原始 FFmpeg stderr、媒体内容或凭据。
- 事件流是契约的一部分：终态事件无法写入 stdout（调用方提前关闭管道等）时进程以非零退出码结束，不得按成功结束。
- Core 不连接业务数据库、对象存储、缓存数据库或权限系统；HTTP/gRPC、Tauri 和桌面安装包不属于 Core 0.1。

## 2. StartRequest

Host-facing `StartRequest` 的 `engineProtocolVersion` 必须精确为 `0.1`，并包含：

```text
engineProtocolVersion
messageType: start
requestId
operation
sourceVersionId
inputs
inputFingerprint
operationConfigHash
outputContractVersion
derivationKey
executionContext
deadlineMs（可选，默认 120000）
options
```

`executionContext` 必须包含 `runId`、`generation`、`attempt` 和 `scope`。`inputs` 在 0.1 恰好包含一个 `source_media`，其 `ref` 固定为 `input/source.media`；绝对路径、UNC、drive path、`..`、ADS、符号链接/reparse escape 均拒绝。

`inputFingerprint` 为 `sha256(RFC8785(InputSetDescriptor))`；`derivationKey` 为 `sha256(RFC8785(DerivationDescriptor))`。Core 必须重算两者，并分别将摘要不一致判为 `INVALID_REQUEST`、实际快照 size/hash 不一致判为 `INPUT_CHANGED`。

## 3. CancelRequest

```json
{
  "engineProtocolVersion": "0.1",
  "messageType": "cancel",
  "requestId": "req_01"
}
```

第一行只能是一个 StartRequest，之后最多一行同 requestId 的 CancelRequest。EOF 不表示取消；重复 start/cancel 或 requestId 不匹配是协议错误。

## 4. Event 与状态机

所有可解析 Host 请求的事件必须回传 `requestId`、`sourceVersionId`、`inputFingerprint`、`derivationKey` 和完整 `executionContext`。

```text
Boot -> Accepted -> Running -> Completed | Failed | Cancelled | TimedOut
```

- `accepted.sequence` 必须为 1；后续 sequence 严格连续，无跳号、重复或回退。
- `accepted` 后恰有一个终态；终态后不得有事件。
- 首行完全无法解析：stdout 为空，安全 stderr，exit 2。
- 可取得 requestId 但请求无效：输出 sequence 1 的 `failed`，不先输出 accepted，exit 2。
- `progress` 的 completed/total 非负，同一 stage 单调。
- completed 携带 operation-specific result；failed/cancelled/timed_out 携带受控 error，不携带可发布 Manifest。

## 5. 错误码矩阵

| Error code | Terminal event | Retryable | Exit | 说明 |
|---|---|---:|---:|---|
| `INVALID_REQUEST` | `failed` | 否 | 2 | 字段、hash、范围或 framing 无效 |
| `UNSUPPORTED_PROTOCOL` | `failed` | 否 | 2 | 协议版本不支持 |
| `OPERATION_UNAVAILABLE` | `failed` | 否 | 2 | 当前 bundle 未实现 operation |
| `INPUT_NOT_FOUND` | `failed` | 否 | 2 | staging 输入缺失 |
| `INPUT_CHANGED` | `failed` | 否 | 2 | 快照 size/hash 改变 |
| `UNSUPPORTED_INPUT` | `failed` | 否 | 3 | 媒体格式不受支持 |
| `CORRUPT_MEDIA` | `failed` | 否 | 3 | 媒体损坏或无法解析 |
| `MISSING_VIDEO_STREAM` | `failed` | 否 | 3 | operation 需要视频但不存在 |
| `RESOURCE_LIMIT` | `failed` | 是 | 3 | 超过明确资源边界 |
| `TOOL_UNAVAILABLE` | `failed` | 是 | 2 | 固定工具或 DLL 不可用 |
| `TOOL_FAILED` | `failed` | 是 | 3 | 固定工具返回失败 |
| `TIMEOUT` | `timed_out` | 是 | 124 | 到达 deadline |
| `CANCELLED` | `cancelled` | 否 | 130 | Host 明确取消 |
| `ENGINE_INTERNAL` | `failed` | 是 | 1 | 未分类内部错误 |

错误 payload 只允许固定字段：`code`、安全且可行动的 `message`/`cause`/`nextStep`，以及受控的 `stage`、`limit`、`actual`。Host 强杀、崩溃、无终态、身份/sequence/terminal-exit 不一致统一视为不可发布的协议失败。

## 6. 共享身份 DTO

`EngineIdentity` 在事件、Manifest、`version` 和 `doctor` 中复用：`engineVersion`、`engineCommit`、`target`、`engineCacheCompatibilityId`、`supportedProtocolVersions`、`implementedOperations`。

`ToolchainIdentity` 复用 `toolchainFingerprint`、target、FFmpeg/FFprobe 版本和能力集合摘要。`engineCacheCompatibilityId` 只有规范化结果或 Artifact 字节兼容性改变时才递增；工具链变化由 `toolchainFingerprint` 体现。

## 7. Manifest 与 Artifact

Manifest 只描述引擎产出，不承担存储归属。每个 Artifact 使用 `relativeRef`、`byteSize`、`contentHash` 和适用时的时间字段；时间区间必须满足 `startMs >= 0`、`endMs > startMs`。单帧只记录请求时间和可选 presentation time，不伪造区间。

`extract_preview` 的请求时间与 `requestedTimeMs` 都是素材时间，即相对容器展示原点 O 的时间；解码定位使用同一坐标系（FFmpeg 输入侧 `-ss` 在默认 `-seek_timestamp 0` 下已按输入起点偏移，调用方不得再加 O）。`presentationTimeMs` 未知时为 `null`，不得用请求时间顶替。

只有 completed + exit 0 + Manifest/文件/Hash 校验通过并由 Host 原子 finalize 的完整集合可注册或缓存。staged、失败、取消、超时、崩溃或未完成产物不得发布、索引或成为缓存命中源。缓存键、cacheScope 隔离与命中规则见 [Host 集成与缓存契约](../../../specs/host-cache-contract.md)。

## 8. `version` 与 `doctor`

`version --json` 和 `doctor --json [--bundle-root <path>]` 是不经过 Host-facing 请求协议的独立 CLI：

- 成功或可诊断失败时 stdout 恰好输出一个 JSON object 和一个换行；不混入日志、进度或彩色文本。
- `doctor` 的 check 顺序、名称、状态、错误码和 nextStep 稳定；非零退出码与 payload 一致。
- 仅当进程级 I/O 阻止 JSON 输出时才写受控 stderr；不得泄露绝对路径、完整 argv 或原始工具 stderr。

`doctor` 至少检查 package manifest、包外可信 manifest digest/签名输入、逐文件 size/hash、相对工具路径、可启动性、版本匹配、能力清单和临时目录读写。

## 9. 兼容与发布

- 0.x 请求精确匹配已支持协议版本；新增字段、枚举或语义发布新的精确协议版本。
- engine version、engine commit、engine cache compatibility 和 toolchain fingerprint 独立记录。
- Bundle manifest 只做完整性校验，不构成信任根。Host/发布流程从包外可信 release metadata、digest 或签名验证 manifest，再验证文件 hash；不要求付费 Authenticode。
- Windows sidecar、Linux bundle 和未来其他 target 分别构建、锁定和验证，不假设二进制或 Artifact hash 跨平台相同。
