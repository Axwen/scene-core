# extract_audio_pcm 操作规格（候选 Protocol 0.1 扩展）

状态：2026-09-21 立项；实现前评审。规范性依据：[素材时间规则 §5](media-time-policy-draft.md)、[Protocol 0.1 主 Spec](scene-core-phase0-protocol-0.1.md)。本规格只定义 `extract_audio_pcm`，不改变 `probe`/`extract_preview` 的线格式。

0.1 仍是未发布候选协议：本操作以新增 operation、Artifact kind、错误码的方式扩展 0.1，既有字段语义不变；一旦被消费者固定，后续变更需新协议版本。

## 1. 目的与范围

给 ASR/字幕消费者提供可定位回原素材时间轴的整轨 PCM 音频，并如实报告映射依据（首个输出样本的素材时间、采样率、样本数），不做静默修复。

v1 范围：

- 抽取所选音轨的**完整轨道**，输出 WAV/PCM `s16le`，保持源采样率与声道数（不重采样、不混音）。
- 支持多音轨文件的显式轨道选择；默认取最低 index 的非附件音频流。
- 报告映射字段并在验收中用已知声画事件验证。

v1 不做（后置，见 SC-P2-EPIC）：

- 裁剪（`startMs`/`endMs`）、重采样、单声道混音；
- 去空白/拼接/变速/多段映射；
- 压缩音频输出（AAC/Opus 等）；
- 字幕/数据流。

## 2. 线格式

### 2.1 请求

`operation: "extract_audio_pcm"`，`outputContractVersion: "extract-audio-pcm-result/1"`。

`options` 从“只接受空对象”扩展为按 operation 区分的联合；`probe`/`extract_preview` 仍只接受 `{}`：

```json
{ "audioStreamIndex": 2 }
```

- `audioStreamIndex`（可选，u32）：要抽取的 ffprobe 流 index，必须指向存在的非附件音频流。
- 省略时取最低 index 的非附件音频流；不存在音频流 → `MISSING_AUDIO_STREAM`。
- 未知键、重复键、类型错误按严格解析拒绝（沿用现有 framing/validation 规则）。
- 有效 options 进入 `operationConfigHash`（canonical JSON），从而进入 `derivationKey`。

### 2.2 结果

与 `extract_preview` 同形：`{ media, artifactManifest, resourceUsage }`，`media` 为同一探测结果的归一化快照。

Artifact（恰好一个）：

| 字段 | 值 |
|---|---|
| `artifactId` | `audio-pcm` |
| `kind` | `audioPcm`（新增 ArtifactKind） |
| `role` | `audio`（新增 ArtifactRole） |
| `mediaType` | `audio/wav` |
| `relativeRef` | `output/audio/track.wav` |
| `byteSize` / `contentHash` | 实际文件大小与流式 SHA-256 |
| `requestedTimeMs` | `0`（v1 无裁剪，请求窗口为整轨） |
| `presentationTimeMs` | **实际首个输出样本的素材时间**（ms，u64，必填） |
| `audioPcm` | `{ "sampleRate": u32, "channels": u32, "sampleCount": u64 }` |

`audioPcm` 为新增可选对象：音频 Artifact 必填、预览 Artifact 必须缺席。

### 2.3 映射语义

- 输出连续、未变速、未重采样：`素材时间 = presentationTimeMs + n / sampleRate`（n 为输出样本序号）。
- `presentationTimeMs = max(stream.startTimeMs, 0)`：负轨道起点（预滚）不进入产物，产物只定位素材时间 `>= 0` 的内容；正起点保留（前段无样本）。
- `sampleCount` 来自 WAV `data` chunk 的实际帧数，不用请求时长推算。
- ASR 局部秒数按上式回映；不得仅用请求起点相加。
- 无法保持单一连续映射（时间戳复位、展示回退、映射不唯一）→ `UNSUPPORTED_INPUT`，不发布产物。

## 3. 错误与资源

| 场景 | 码 | 终态/退出码 |
|---|---|---|
| 无可用音频流 | `MISSING_AUDIO_STREAM`（新增，非可重试） | `failed` / 2 |
| 不可靠时间轴、无法验证映射 | `UNSUPPORTED_INPUT` | `failed` / 3 |
| 输出预算超限、磁盘不足 | `RESOURCE_LIMIT` | `failed` / 3 |
| 工具缺失/失败、取消、超时 | 沿用现有码与 stage 规则 | 不变 |

- 输出预算：按 `durationMs × sampleRate × channels × 2` 估算，超过上限（实现常量，建议 2 GiB）在启动工具前拒绝；时长未知时依赖工具写满失败与 Host 磁盘配额，仍映射 `RESOURCE_LIMIT`。
- 失败/取消/超时/部分产物沿用不可发布、不可缓存规则。

## 4. 验收

| 用例 | 预期 |
|---|---|
| 1 s 合成音频（已知 tone 起点 0、48 kHz 立体声） | `completed`；`presentationTimeMs=0`、`sampleRate=48000`、`channels=2`、`sampleCount≈48000`；WAV 可解码 |
| 音轨起点 +80 ms（已知声画事件） | `presentationTimeMs=80`；ASR 局部 5 s → 素材 5.080 s |
| 音轨起点 -80 ms（预滚） | 产物从素材 0 开始，`presentationTimeMs=0`，预滚样本不入产物 |
| 多音轨（两条不同 tone） | 默认取最低 index；显式 `audioStreamIndex` 选中另一条，产物内容与选择一致 |
| 无音频流 | `failed` + `MISSING_AUDIO_STREAM`，无 Artifact |
| 时间戳复位/不连续样本 | `failed` + `UNSUPPORTED_INPUT`，不伪造映射 |
| 超出输出预算的时长 | 启动工具前 `RESOURCE_LIMIT` |
| 取消/超时/只读输出目录 | 沿用 SC-P1-05 的终态、退出码与无产物断言 |

## 5. 兼容与派生

- `implementedOperations` 变为 `["probe","extract_preview","extract_audio_pcm"]`；`version --json`、doctor、bundle manifest 与 CI 冒烟随之对齐。
- 新 operation 不改变既有请求/事件的公共字段与状态机；`derivationKey` 因 operation 与 options 不同自然区分。
- Schema 与 fixture 由 Rust DTO 生成，新增 operation/options/result/artifact/error 的 valid/invalid/golden 用例。
