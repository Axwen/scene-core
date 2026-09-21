# SC-P1-04：固定诊断预览与 Artifact Manifest

## 目的

实现固定 profile 的诊断预览与 Manifest：单帧 JPEG、opening/midpoint 规则、实际帧时间可证明、Artifact 流式 hash、原子 finalize。

## 范围

- profile 固定：JPEG、最大边 512、不放大、保持比例、应用显示旋转、移除 metadata、软件解码。
- 有有效 duration 请求 0ms 与 `floor(durationMs/2)`；时间相同只保留 opening；无有效 duration 只请求 opening；无视频流失败。
- 单帧记录 `requestedTimeMs`，可证明时提供 `presentationTimeMs`；不生成 `startMs/endMs`。
- Artifact 固定 ID/ref 与顺序；`byteSize`/`contentHash` 流式计算；Manifest 通过 `artifact-manifest.json` Schema 与 DTO 校验；staging 内原子 finalize。
- `extract_preview` completed result 同时携带归一化 `media`、`artifactManifest` 和 `resourceUsage`。

## 验收标准

1. 合成媒体（含 rotation、奇数尺寸、VFR）与真实脱敏样本产出确定性 JPEG，尺寸/比例/旋转符合 profile。
2. 请求时间与实际帧时间分别记录；无法证明实际时间时为 null，不用请求时间替代证据。
3. Manifest 与文件 hash/size 一致；缺失/篡改由 doctor 类校验拒绝；staged 产物不发布。

## 实现状态（2026-09-16）

- `scene-core-media/src/preview.rs`：固定 profile（JPEG、最大边 512、不放大、保持比例、force_divisible_by=2、解码器自动应用显示旋转、移除 metadata、软件解码），`-ss` 精确定位到素材时间（origin + requested），写临时文件后 rename 原子 finalize；JPEG SOF 解析尺寸。
- `run` 分发 `extract_preview`：先 probe（缺视频流 → `MISSING_VIDEO_STREAM`），按 duration 生成 opening（0ms）与 midpoint（`floor(durationMs/2)`，为 0 时省略）；Manifest 由请求身份 + engine + toolchain fingerprint + Artifact 组成并通过 DTO 校验；completed result 携带 media/artifactManifest/resourceUsage。
- 事件：accepted → progress(stage `preview`, total 2) → completed；`implemented_operations()` 现为 `[probe, extract_preview]`，version/doctor/bundle smoke 随之对齐。
- 测试：preview 合同（640x480 → 512x384；64x48 不放大）、JPEG 解析与秒格式化单元测试、run 会话中 extract_preview 的 manifest 校验；workspace 共 151 tests，clippy 无 allow。

边界备注：`presentationTimeMs` 当前无法证明时保持 `null`；磁盘不足注入已由 SC-P1-05 覆盖，旋转真实样本仍属于 SC-P1-06 的真实媒体验收缺口。

## 依赖

SC-P1-02、SC-P1-03。
