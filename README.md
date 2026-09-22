# scene-core

`scene-core` 是独立的 Rust 媒体核心仓库，负责受控的 FFmpeg/FFprobe 调用、媒体探测、基础 Artifact、Manifest、取消、超时和资源统计。

当前迁移内容位于 [`docs/architecture/repositories/`](docs/architecture/repositories/README.md)。P1 已实现 `probe` 与固定诊断预览（`extract_preview`），实现必须遵守其中的引擎协议和 readiness gate；音频抽取（[SC-P2 Epic](docs/tickets/SC-P2-EPIC.md)、[操作规格](docs/specs/extract-audio-pcm.md)）已实现裁剪、重采样与时间映射，验收记录见 [`docs/acceptance/audio-acceptance.md`](docs/acceptance/audio-acceptance.md)，分析抽帧待立项。

## 许可

本仓库代码以 [MIT](LICENSE) 发布。bundle 内随附的 FFmpeg/FFprobe 为独立的 LGPL-3.0-or-later 构件（动态链接、无 GPL/nonfree 组件），许可与源码获取说明见 [`docs/legal/ffmpeg-lgpl-review.md`](docs/legal/ffmpeg-lgpl-review.md)。
