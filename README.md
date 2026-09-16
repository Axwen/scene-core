# scene-core

`scene-core` 是独立的 Rust 媒体核心规划仓库，负责受控的 FFmpeg/FFprobe 调用、媒体探测、基础 Artifact、Manifest、取消、超时和资源统计。

当前迁移内容位于 [`docs/architecture/repositories/`](docs/architecture/repositories/README.md)。真实媒体运行时尚未实现；实现必须遵守其中的引擎协议和 readiness gate。
