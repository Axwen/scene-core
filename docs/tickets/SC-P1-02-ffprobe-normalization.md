# SC-P1-02：FFprobe 归一化与展示时间转换

## 目的

把 ffprobe 输出变成 Protocol 0.1 的 `NormalizedMedia`，所有时间用 `ExactTime` 有理运算按已确认规则换算，不猜原点、不归零轨道。

## 范围

- 解析固定版本 ffprobe 的 JSON：container、streams（kind/codec/timeBase/duration/start/default/attachedPicture、视频 coded/display 尺寸、rotation、平均帧率、音频 sampleRate/channels/layout）。
- 展示原点与轨道起点按 [素材时间规则](../specs/media-time-policy-draft.md)：`container.startTimeMs` 只 0/null；`stream.startTimeMs` 有符号；未知一律 null；时间戳复位、展示回退等不可靠时间轴 → `UNSUPPORTED_INPUT`。
- 损坏容器/无法解析 → `CORRUPT_MEDIA`；需要视频但无视频流 → `MISSING_VIDEO_STREAM`。
- 合成媒体生成脚本（固定工具链生成小样本）与 golden 归一化 JSON fixture。

## 验收标准

1. 向量：O=10s 音视频偏移 80ms、负原点、负轨道起点、无 origin（null）、极大 ticks 溢出受控。
2. 损坏媒体、无音轨、无视频轨、VFR、B 帧样本的预期结果写入 fixture manifest。
3. 归一化输出通过 `normalized-media.json` Schema 与 DTO 校验，且确定性可重现。

## 实现状态（2026-09-16）

- 新增 `scene-core-media/src/probe.rs`：运行锁定 ffprobe（`-show_format -show_streams`，120s deadline）并映射到 `NormalizedMedia`。
- 时间规则：以容器展示起点为唯一原点；`container.startTimeMs` 只 0/null；`stream.startTimeMs` 有符号毫秒（差值经 `ExactTime` 有理运算再半毫秒远离零舍入）；`N/A`/`0/0`/负 duration 视为未知 null；存在但无法解析的时间戳 → `UNSUPPORTED_INPUT`。
- 失败分类：spawn 失败 `TOOL_UNAVAILABLE`；`Invalid data`/`moov atom`/`malformed` → `CORRUPT_MEDIA`；其余非零 → `UNSUPPORTED_INPUT`；输出非 JSON → `ENGINE_INTERNAL`。
- 附件图不计为 primary video；字幕/数据/附件流不带音视频字段；`avg_frame_rate` 为 `0/0` 时 null。
- 测试：10 个内嵌 ffprobe JSON 单元测试（偏移、未知原点、负起点、±1.5ms 舍入、附件图、N/A、字幕、失败分类）；`tests/probe_contract.rs` 用锁定工具链生成合成音视频并对比 golden `fixtures/media/golden/synthetic-av.json`（`UPDATE_MEDIA_GOLDEN=1` 重生成），另覆盖纯音频与损坏/缺失输入。
- workspace 共 137 tests，clippy 无 allow。

未完成（后续 ticket）：rotation 真实样本、编辑列表/时间复位等不可靠时间轴检测、VFR/B 帧黄金样本。

## 依赖

SC-P1-01。
