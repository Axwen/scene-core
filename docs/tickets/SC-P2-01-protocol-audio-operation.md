# SC-P2-01：协议扩展 extract_audio_pcm

## 目的

在 `scene-core-protocol` 中把 `extract_audio_pcm` 做成与 `probe`/`extract_preview` 同级的严格 DTO 路径，保持现有请求/事件线格式不变。

## 范围

- `Operation::ExtractAudioPcm`（wire `extract_audio_pcm`），注册契约 `extract-audio-pcm-result/1`。
- `OperationOptions` 从“只接受空对象”改为按 operation 区分的联合（`probe`/`extract_preview` 仍只接受 `{}`；音频接受可选 `audioStreamIndex`），并校验选项与 operation 匹配。
- `ExtractAudioPcmResult`（`media`/`artifactManifest`/`resourceUsage` 同预览形状）。
- `ArtifactKind::AudioPcm`、`ArtifactRole::Audio`、`Artifact.audioPcm`（`sampleRate`/`channels`/`sampleCount`）；Manifest 按 operation 分支校验固定 artifact 槽位（`audio-pcm`、`output/audio/track.wav`、`audio/wav`、`presentationTimeMs` 必填、预览不得带 `audioPcm`）。
- `ErrorCode::MissingAudioStream`（`MISSING_AUDIO_STREAM`，非可重试）。
- `OperationResult::ExtractAudioPcm`；`implemented_operations()` 增项。
- Schema 重生成（`UPDATE_SCHEMAS=1 cargo test -p scene-core-protocol --test schema_contract`）；valid/invalid/golden fixture 与 transcript 校验。

## 验收标准

1. `StartRequest` 的 `options` 对三种 operation 的合法/非法形状有正反用例（含未知键、错误类型、operation 与选项不匹配）。
2. 音频 Manifest 正反用例：槽位字段固定；缺 `presentationTimeMs`/`audioPcm`、预览带 `audioPcm`、多/少 artifact 均拒绝。
3. `derivationKey` 覆盖 `audioStreamIndex`（不同选项产生不同 key）。
4. Schema drift 门禁通过；fixture conformance 通过；既有 probe/preview 用例不回归。

## 实现状态（2026-09-21）

- `Operation::ExtractAudioPcm`（`extract-audio-pcm-result/1`）、`OperationOptions` 联合（`EmptyOptions` / `AudioPcmOptions`，手工 `Deserialize` 强制对象形态并拒绝未知/重复键）、`ExtractAudioPcmResult`、`ArtifactKind::AudioPcm`、`ArtifactRole::Audio`、`Artifact.audioPcm`、`ErrorCode::MissingAudioStream` 已实现。
- Manifest 按 operation 分支校验音频槽位；`completed` 身份回传覆盖音频结果。
- Schema 重生成（15 个错误码、新 operation/options/result/artifact）；新增 8 个 fixture（含 golden transcript），fixture 总数 81。
- 测试：protocol 9 个目标全绿（options 正反、音频 Manifest 正反、身份回传、`ALL.len()==15`）。

## 依赖

无。

## 备注

0.1 仍是候选协议：本 ticket 是扩展而非新版本；被消费者固定后需新协议版本。
