# SC-P1-02：FFprobe 归一化与展示时间转换

## 目的

把 ffprobe 输出变成 Protocol 0.1 的 `NormalizedMedia`，所有时间用 `ExactTime` 有理运算按已确认规则换算，不猜原点、不归零轨道。

## 范围

- 解析固定版本 ffprobe 的 JSON：container、streams（kind/codec/timeBase/duration/start/default/attachedPicture、视频 coded/display 尺寸、rotation、平均帧率、音频 sampleRate/channels/layout）。
- 展示原点与轨道起点按 [素材时间规则](../../specs/media-time-policy-draft.md)：`container.startTimeMs` 只 0/null；`stream.startTimeMs` 有符号；未知一律 null；时间戳复位、展示回退等不可靠时间轴 → `UNSUPPORTED_INPUT`。
- 损坏容器/无法解析 → `CORRUPT_MEDIA`；需要视频但无视频流 → `MISSING_VIDEO_STREAM`。
- 合成媒体生成脚本（固定工具链生成小样本）与 golden 归一化 JSON fixture。

## 验收标准

1. 向量：O=10s 音视频偏移 80ms、负原点、负轨道起点、无 origin（null）、极大 ticks 溢出受控。
2. 损坏媒体、无音轨、无视频轨、VFR、B 帧样本的预期结果写入 fixture manifest。
3. 归一化输出通过 `normalized-media.json` Schema 与 DTO 校验，且确定性可重现。

## 依赖

SC-P1-01。
