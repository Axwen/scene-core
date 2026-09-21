# SC-P2-02：media 音频抽取与映射字段

## 目的

在 `scene-core-media` 用锁定工具链实现整轨 PCM 抽取，并给出可验证的时间映射字段。

## 范围

- `audio::extract`：`ffmpeg -nostdin -i input -map 0:<index> -vn -sn -dn -c:a pcm_s16le -f wav -y <tmp>`，输出上限与取消/超时沿用 `process::run`。
- `audio::wav_info`：只读文件头（RIFF/fmt/data），返回 `sampleRate`/`channels`/`sampleCount`（`data` 字节 / block align）；不整读文件。
- 映射：`presentationTimeMs = max(stream.startTimeMs, 0)`，`sampleCount` 取自实际 WAV；负起点预滚不入产物。
- 输出预算：按 `durationMs × rate × channels × 2` 估算，超过常量上限（建议 2 GiB）在启动工具前拒绝。
- 错误：无音轨/轨道越界 → `MissingAudioStream`；不可靠时间轴 → `UnsupportedInput`；磁盘/权限 → `ResourceLimit`；工具失败 → `ToolFailed`。
- 多音轨：调用方传入 `audioStreamIndex`，默认取最低 index 的非附件音频流。

## 验收标准

1. `tests/audio_contract.rs`（锁定工具链、env-gated）：
   - 1 s 48 kHz 立体声合成音频 → `sampleRate=48000`、`channels=2`、`sampleCount=48000±1`、WAV 可被工具回读；
   - 音轨起点 +80 ms → `presentationTimeMs=80`；
   - 音轨起点 -80 ms → 产物从素材 0 开始（`presentationTimeMs=0`，样本数按剩余时长）；
   - 多音轨不同 tone → 默认与显式选择内容可区分；
   - 无音频流 → `MissingAudioStream`，无文件；
   - 输出预算超限（缩短常量或构造长时长）→ `ResourceLimit`，不启动工具。
2. `wav_info` 单元测试覆盖截断头、非 WAV、奇数 data 大小。
3. `media_acceptance` 用已知 tone 起点验证 `presentationTimeMs + n/sampleRate` 与事件偏差在 1 ms 内。

## 实现状态（2026-09-21）

- `scene-core-media/src/audio.rs`：`extract`（`-map 0:<index>`、`-vn/-sn/-dn`、`pcm_s16le`、WAV）、`wav_info`（只读 RIFF/fmt/data 头）、`estimate_output_bytes`、`MAX_OUTPUT_BYTES`（2 GiB）。
- 单元测试：WAV 头正反、输出估算；合同测试 `tests/audio_contract.rs`：单轨映射、双轨选择（48k/8k）、取消。
- 未做（归 SC-P2-04/后置）：编码延迟的真实样本核验、连续性与时间戳复位的深度检测（当前为流起点缺失拒绝 + 时长粗差拒绝）。

## 依赖

SC-P2-01。
