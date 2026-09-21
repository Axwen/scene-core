# 音频抽取验收记录（extract_audio_pcm）

日期：2026-09-21。范围：SC-P2-04。工具链：锁定 BtbN LGPL 构件（`n9.0.1-31-g3a7c002718`，见 `packaging/toolchains/`）。样本均为合成、无版权风险。

规范：[extract_audio_pcm 操作规格](../specs/extract-audio-pcm.md)；时间规则：[素材时间规则 §5](../specs/media-time-policy-draft.md)。

## 自动化证据

| 用例 | 测试 | 观测 | 判定 |
|---|---|---|---|
| 单轨映射（48 kHz 立体声 1 s） | `run_contract::audio_extraction_reports_the_track_mapping` | `completed`；`sampleRate=48000`、`channels=2`、`sampleCount=48000`、`presentationTimeMs=0`；产物存在 | ✅ |
| 已知阶跃起点 0.5 s | `audio_contract::tone_onset_maps_within_one_millisecond` | 首个非静音帧映射到 500 ms，偏差 0 ms（阈值 1 ms） | ✅ |
| 正轨道起点 +80 ms | `run_contract::audio_extraction_reports_a_positive_stream_start` | `presentationTimeMs=80`；`80 + n/rate` 把首帧放回 80 ms（偏差 ≤1 ms） | ✅ |
| 预滚负起点 | `run::tests::audio_material_start_clamps_pre_roll_to_zero` | `-80 → 0`（单元） | ✅（无真实样本） |
| 双音轨显式选择 | `run_contract::audio_stream_selection_is_explicit` | 默认最低 index（48 kHz），`audioStreamIndex=1` → 8 kHz | ✅ |
| 无音轨 | `run_contract::missing_audio_stream_fails_with_its_code` | `failed` + `MISSING_AUDIO_STREAM`、exit 3、无产物 | ✅ |
| 只读输出目录 | `run_contract::unwritable_audio_output_reports_resource_limit` | `failed` + `RESOURCE_LIMIT`、exit 3、无产物 | ✅ |
| deadline=1 ms | `run_contract::audio_deadline_reaps_the_tool_and_exits_124` | `timed_out`、exit 124、无产物 | ✅ |
| 取消（预置 flag） | `audio_contract::cancellation_stops_the_extraction` | `Cancelled`、无产物发布 | ✅ |
| WAV 头解析 | `audio::tests::{reads_pcm_header_facts, rejects_truncated_and_non_wav_input}` | 正反用例 | ✅ |
| 输出预算估算 | `audio::tests::estimates_pcm_output_bytes` | 1 h ≈ 691 MB；5 h 超 2 GiB 上限 | ✅（未用真实大文件复现） |
| 性能门禁（音频） | `baselines::performance_smoke_gate` | 30 s/48 kHz 立体声 → 843× realtime（floor 10×）；`sampleCount` 精确校验 | ✅ |
| Windows bundle 内端到端 | `scripts/bundle-smoke.ps1`（Windows bundle spike job） | 生成音频 → `extract_audio_pcm` → `completed` + `audioPcm` + `output/audio/track.wav` | ✅（CI 执行） |

映射公式与回映示例：`素材时间 = presentationTimeMs + n / sampleRate`。例如正起点 80 ms 的素材，ASR 局部 1.5 s 回映为 1.580 s；未来裁剪版本按实际首样本起点计算，不用请求起点相加。

## 资源

- 30 s 48 kHz 立体声 → 5.76 MB PCM，抽取 843× realtime（WSL2 本机，锁定工具链）。
- 输出预算按 `durationMs × sampleRate × channels × 2` 估算，超过 2 GiB 在启动工具前拒绝。

## 未覆盖（不伪装为已通过）

- 真实压缩音频（AAC/MP3 等）的编码延迟/priming 核验：当前映射证据来自 PCM 合成样本。
- 时间戳复位/展示回退等不可靠时间轴的真实样本：目前只有“原点/流起点缺失拒绝 + 时长粗差拒绝”与单元级 clamp，缺少真实异常样本复现。
- 负轨道起点预滚：只有单元测试，无真实样本。
- 裁剪/重采样与多段映射：按 Epic 后置，不属于本记录范围。
- >2 GiB 预算拒绝未用真实大文件复现。
- Windows 运行时由 bundle spike 覆盖；本机没有 Windows 复测记录。
