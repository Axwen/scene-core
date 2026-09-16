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

## 覆盖与缺口

- 已覆盖：真实 CFR 素材的容器/轨道归一化、音视频同步起点、MP4/H.264/AAC 常见路径、预览 profile 缩放与 midpoint 规则。
- 仍需真实素材：非零/负轨道起点、编辑列表、VFR、B 帧、旋转、附件图、损坏与时间戳异常样本（当前由合成样本与单元测试覆盖，见 `crates/scene-core-media/tests/media_acceptance.rs`）。
- 播放器点击对照仍未做。
