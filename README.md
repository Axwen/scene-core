# scene-core

`scene-core` 是独立的 Rust 媒体核心仓库，负责受控的 FFmpeg/FFprobe 调用、媒体探测、基础 Artifact、Manifest、取消、超时和资源统计。

当前迁移内容位于 [`docs/architecture/repositories/`](docs/architecture/repositories/README.md)。P1 已实现 `probe` 与固定诊断预览（`extract_preview`），实现必须遵守其中的引擎协议和 readiness gate；后续音频抽取与分析抽帧按各自 Spec 立项。

## 许可

本仓库代码以 [MIT](LICENSE) 发布。bundle 内随附的 FFmpeg/FFprobe 为独立的 LGPL-3.0-or-later 构件（动态链接、无 GPL/nonfree 组件），许可与源码获取说明见 [`docs/legal/ffmpeg-lgpl-review.md`](docs/legal/ffmpeg-lgpl-review.md)。
