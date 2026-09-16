# SC-P1-06：真实/合成媒体验收与性能基线

## 目的

用可复核的媒体证据关闭 Phase 1：时间偏移、帧选择、损坏输入和大文件资源边界。

## 范围

- 合成媒体集（脚本生成）：已知声画事件、VFR、B 帧、rotation、attached picture、无音轨、无视频轨、负起点/编辑列表样本。
- 真实/公开脱敏样本（可选）与 golden 归一化 JSON、preview 期望图。
- 性能基线：流式 SHA-256 吞吐、20 GiB（或缩比）输入 P95、峰值内存、输出体积、磁盘预算。
- 与播放器/ffprobe 的人工对照记录（点击标注 → 目标片段）。

## 验收标准

1. 时间规则验收表（[素材时间规则 §7](../../specs/media-time-policy-draft.md)）中的 Phase 1 行全部有证据。
2. 性能报告包含设备、命令、P50/P95、峰值内存与输出体积，不承诺未测数值。
3. 所有拒绝路径有 fixture 与错误码记录；不把规划表当已通过。

## 实现状态（2026-09-16，增量一：合成媒体样本与冒烟基线）

- `tests/media_acceptance.rs`（锁定工具链、env-gated）：
  - `-display_rotation 90` 重封装样本 → `rotationDegrees=90` 且显示尺寸不变；
  - FLAC 封面图 → `attachedPicture=true` 且**不再**成为 primary video（probe 规则收紧，避免用封面当预览帧）；
  - `-bf 2`（mpeg4）与 `-fps_mode vfr` 样本 → probe 起点 0、时长可用，不误报回退；
  - 预览 + 哈希性能冒烟（640x480 → 512x384、64 MiB 哈希吞吐，含防退化下限）。
- `docs/performance/media-baseline.md`：首版设备/命令/数据（预览 25 ms/17.7 KB；debug 哈希 ≈52 MiB/s），并明确不是发布承诺。
- workspace 共 159 tests，clippy 无 allow。

增量二（2026-09-16，release 基线与预览 golden）：

- `tests/baselines.rs`（`#[ignore]`）+ `scripts/bench-media.sh`：release 下测量 probe/预览 P50/P95、预览体积、256 MiB 流式哈希吞吐与 harness 峰值 RSS。
- 结果（WSL2 x86_64）：probe p50 10.4 / p95 10.5 ms；预览 p50 20.5 / p95 20.5 ms，17,687 字节；哈希 ≈3.6 GiB/s；峰值 RSS ≈152 MiB。已记入 `docs/performance/media-baseline.md`。
- 预览 golden：`fixtures/media/golden/preview-opening.sha256`，`media_acceptance` 常态断言生成字节与 golden 一致（`UPDATE_MEDIA_GOLDEN=1` 重生成）。
- 哈希吞吐常态门槛放宽为 >2 MiB/s（仅防灾难性退化），实测值打印留档。

增量三（2026-09-16，引擎峰值与结项）：

- `scripts/bench-engine.sh`：用 `/usr/bin/time -v` 包裹单个 CLI 调用，实测引擎峰值 RSS 与墙钟（probe 34 MiB / 0.01 s；extract_preview 43 MiB / 0.05 s），已记入性能文档。
- 大文件：20 GiB（20,480 MiB）已实测——单遍 567 MiB/s（磁盘受限），峰值 RSS 152 MiB；256 MiB 缓存命中时 3.6 GiB/s。复现：`SCENE_CORE_BENCH_LARGE_MIB=20480 bash scripts/bench-media.sh`。

未完成（需真实资源，不伪装为已通过）：

- 真实/公开脱敏媒体样本、预览期望图与播放器点击对照；分析抽帧帧选择语义按政策属于未来独立 operation。
- 并发/多请求下的峰值内存与磁盘配额（单请求基线已完成）。

## 依赖

SC-P1-02、SC-P1-04、SC-P1-05。
