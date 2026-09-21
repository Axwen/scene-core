# 真实媒体验收记录

样本不进入仓库；下表仅保留匿名特征与结果。复现方式：把本地真实样本放到
`REAL_MEDIA=<path>`，再用 `scene-core run`（staging 内固定为 `input/source.media`）执行
`probe` 与 `extract_preview`，并与锁定工具的 `ffprobe` 输出对照。

## 样本 A（2026-09-16）

匿名特征：H.264/AAC MP4，≈14.5 MiB，96.52 s，1280x720 @ 30 fps，音视频起点均为 0，无旋转、无附件图、无编辑列表。

| 项 | ffprobe（基准） | 引擎 `probe` |
|---|---|---|
| 容器 | `mov,mp4,m4a,3gp,3g2,mj2`，duration 96.5239 s，start 0 | `container.startTimeMs=0`，`durationMs=96524`，`fileSizeBytes=15241195` |
| 视频流 | h264 1280x720，time_base 1/19200，avg 30/1，start 0 | kind=video，start 0，duration 96500，timeBase 1/19200，显示 1280x720，avg 30/1，rotation null |
| 音频流 | aac 44100 Hz，2 声道，time_base 1/44100，start 0 | kind=audio，start 0，duration 96524，timeBase 1/44100，sampleRate 44100，channels 2 |

`extract_preview` 结果（同一请求）：

| Artifact | requestedTimeMs | presentationTimeMs | 尺寸 | 字节 |
|---|---|---|---|---|
| preview-opening | 0 | null（无法证明实际帧时间，按政策不伪造） | 512x288 | 39,676 |
| preview-midpoint | 48,262（= floor(96524/2)） | null | 512x288 | 29,007 |

进程表现：probe 0.02 s / 38.8 MiB 峰值；extract_preview 0.13 s / 86.9 MiB 峰值；事件中无主机路径。

## 2026-09-21 复测（当前引擎，含 `extract_audio_pcm`）

用同一真实样本与新增的竖屏样本（均不入库）复跑：

| 项 | 样本 A（1280x720，96.524 s） | 样本 B（1080x1920，53.406 s） |
|---|---|---|
| `probe` | container 96524/start 0；video 96500、audio 96524；AAC 44.1 kHz 立体声 | container 53406/start 0；video 53400、audio 53406；同参数 |
| `extract_preview` | opening 512x288；midpoint `requestedTimeMs=48250` | opening/midpoint 288x512（不放大） |
| `extract_audio_pcm` 整轨 | 4,256,704 样本（=96.524 s）、`presentationTimeMs=0` | 2,355,200 样本（=53.406 s）、`presentationTimeMs=0` |
| 裁剪 `[10 s,20 s)` | 441000 样本；与整轨逐字节一致 | 440999 样本；对齐在 ±2 输入样本内 |
| 裁剪 + 16 kHz 单声道 | 160000 样本 | 160000 样本 |

注：midpoint 由 `floor(containerDurationMs/2)`（2026-09-16 记录的 48262）改为夹取到主视频轨时长（`min(container, video)/2 = 48250`），属 P1 审查修复后的预期行为。音频细节见 [audio-acceptance.md](audio-acceptance.md)。

## 覆盖与缺口

- 已覆盖：真实 CFR 素材的容器/轨道归一化、音视频同步起点、MP4/H.264/AAC 常见路径、预览 profile 缩放与 midpoint 规则。
- 仍需真实素材：非零/负轨道起点、编辑列表、VFR、B 帧、旋转、附件图、损坏与时间戳异常样本（当前由合成样本与单元测试覆盖，见 `crates/scene-core-media/tests/media_acceptance.rs`）。
- 播放器点击对照仍未做。
