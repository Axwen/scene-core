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

## 依赖

SC-P1-02、SC-P1-03。
