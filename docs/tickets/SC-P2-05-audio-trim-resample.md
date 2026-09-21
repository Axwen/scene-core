# SC-P2-05：音频裁剪与重采样

## 目的

让 `extract_audio_pcm` 支持请求区间与输出格式，长素材可按区间抽取、按 ASR 常用参数（如 16 kHz 单声道）输出，同时保持样本级的时间映射。

## 范围

- 请求 options：`startMs`、`endMs`（半开区间，`endMs > startMs`）、`sampleRate`（8000..=192000）、`channels`（1|2）；省略时保持整轨与源参数。
- media：`-copyts` + `atrim` 采样级裁剪；起点超过 2 s 时按 2 s 余量先 seek；`-ar/-ac` 在裁剪之后应用。
- 映射：`presentationTimeMs = max(startMs, stream.startTimeMs, 0)`；`requestedTimeMs = startMs`；`audioPcm` 报告输出侧实际值；空窗口（无样本）→ `UNSUPPORTED_INPUT`。
- Manifest：音频 Artifact 的 `requestedTimeMs` 不再固定为 0，但必须 `presentationTimeMs >= requestedTimeMs`，且 `sampleCount > 0`。
- Schema/fixture：trim 正例与非法区间/越界采样率/空窗口反例。

## 验收标准

1. 阶跃信号裁剪 `[400,700)` + 16 kHz 单声道：阶跃映射回 500 ms，偏差 ≤1 ms；`requestedTimeMs=400`、`presentationTimeMs=400`、`sampleCount=4800`。
2. 从起点解码的裁剪与整轨切片逐样本一致（媒体合同测试）。
3. 起点晚于轨道结束 → `UNSUPPORTED_INPUT` 且无产物；`endMs` 超出轨道结束 → 输出到轨道结束。
4. 真实样本：10 s 窗口在 44.1 kHz 下 `sampleCount≈441000`，16 kHz 单声道 `sampleCount=160000`，裁剪起点对齐在 ±2 输入样本内。
5. `startMs >= endMs`、采样率/声道越界 → `INVALID_REQUEST`，不启动工具。

## 实现状态（2026-09-21）

- 协议：options 扩展与校验、Manifest 规则、Schema 重生成、86 个 fixture（含 trim 正反例）。
- media：`AudioTrim` + `atrim`/`-copyts` 实现；合同测试覆盖逐样本一致、空窗口、重采样。
- engine：`probe_with_origin` 提供原始容器原点；窗口预算与期望时长守卫（真实样本验证时修掉“用整轨剩余时长当期望窗口”的缺陷）；新增 10 s AAC 窗口回归测试。
- 真实验证与未覆盖项见 [audio-acceptance.md](../../acceptance/audio-acceptance.md)。

## 依赖

SC-P2-02、SC-P2-03。
