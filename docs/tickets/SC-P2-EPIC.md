# SC-P2：音频抽取（extract_audio_pcm）

## 目标

在 Protocol 0.1 候选版上新增 `extract_audio_pcm`：抽取所选音轨的完整轨道或请求区间（WAV/s16le，可重采样到请求的采样率/声道），并如实报告可回映原素材时间轴的映射字段，供 ASR/字幕消费者使用。

规范：[extract_audio_pcm 操作规格](../specs/extract-audio-pcm.md)；时间依据：[素材时间规则 §5](../specs/media-time-policy-draft.md)。

## 范围

- 协议：新 operation、按 operation 区分的 `options`（含 `startMs`/`endMs`/`sampleRate`/`channels`）、结果与 Artifact（`audioPcm`/`audio`）、`MISSING_AUDIO_STREAM` 错误码、Schema/fixture。
- media：受控 ffmpeg 抽取、WAV 头解析（采样率/声道/帧数）、采样级裁剪与重采样、映射字段、输出预算。
- engine：`run` 分发、进度 stage `audio`、Manifest 校验、错误映射、`implementedOperations` 与 bundle/doctor 对齐。
- 验收：合成声画事件映射（含裁剪/重采样）、多音轨选择、预滚、无音轨、资源与取消路径、真实样本验证。

## 明确不做（后置）

- 去空白/拼接/变速/多段映射；压缩音频输出；字幕/数据流；ASR/模型侧工作。

## Tickets

| ID | 任务 | 依赖 |
|---|---|---|
| [SC-P2-01](SC-P2-01-protocol-audio-operation.md) | 协议扩展：operation/options/result/Artifact/错误码/Schema/fixture | 无 |
| [SC-P2-02](SC-P2-02-media-audio-extraction.md) | media 抽取与 WAV 映射字段、输出预算 | SC-P2-01 |
| [SC-P2-03](SC-P2-03-engine-audio-dispatch.md) | engine 分发、进度、Manifest 校验与身份对齐 | SC-P2-01、SC-P2-02 |
| [SC-P2-04](SC-P2-04-audio-acceptance.md) | 合成/真实样本验收与映射验证、资源与性能 | SC-P2-02、SC-P2-03 |
| [SC-P2-05](SC-P2-05-audio-trim-resample.md) | 裁剪与重采样（options/atrim/映射规则/验收） | SC-P2-02、SC-P2-03 |

## 状态（2026-09-21）

SC-P2-01/02/03/04/05 均已实现：协议、media 抽取（含裁剪/重采样）、engine 分发与验收（[audio-acceptance.md](../../acceptance/audio-acceptance.md)）通过；音频已纳入性能门禁与 Windows bundle 冒烟。未覆盖项（时间戳复位真实样本、超长输出实测）在验收记录中明示。

## 完成定义

- 五个 ticket 验收通过；`version --json` 如实报告 `implementedOperations: ["probe","extract_preview","extract_audio_pcm"]`。
- 合成样本上 `presentationTimeMs + n/sampleRate` 与已知声画事件的偏差在验收阈值内；无音轨与不可靠时间轴受控失败、不发布产物。
- Linux CI（锁定工具链）跑通合同测试；Windows bundle 冒烟覆盖新 operation；性能门禁覆盖音频抽取的时长/吞吐下限。
- 失败、取消、超时与部分产物沿用不可发布、不可缓存规则（SC-P0-07）。
