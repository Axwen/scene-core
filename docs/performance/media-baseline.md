# 媒体性能基线（首版）

设备/环境：WSL2 x86_64（Linux），release 构建，锁定工具链 `x86_64-unknown-linux-gnu`（BtbN `n9.0.1-31-g3a7c002718`）。
复现命令：

```bash
export SCENE_CORE_FFMPEG_DIR=<fetch-toolchain.sh 输出的 bin 目录>
bash scripts/bench-media.sh
```

## 测量结果（2026-09-16）

| 指标 | 样本 | 结果 |
|---|---|---|
| probe | 合成 mkv 640x480/30fps/1s，20 次 | p50 10.4 ms，p95 10.5 ms |
| 预览生成 | 同一样本 → JPEG | p50 20.5 ms，p95 20.5 ms；输出 17,687 字节（512x384） |
| 流式 SHA-256 | 256 MiB 文件 | ≈3.6 GiB/s（release） |
| 进程峰值 RSS | 上述 harness（含 256 MiB 流式哈希，分块写入） | ≈152 MiB |
| 引擎进程峰值 RSS | `scripts/bench-engine.sh`（probe / extract_preview，release） | 34 MiB / 43 MiB |
| 引擎墙钟 | 同上，合成 1s 640x480 样本 | probe 0.01 s / extract_preview 0.05 s |

debug 构建对照（较慢，仅参考）：64 MiB 哈希 ≈52 MiB/s；预览 25 ms。

## 断言与门槛

- `media_acceptance.rs` 常态断言：预览 ≤512 KiB、最长边 512、非等比不裁剪；哈希吞吐 >2 MiB/s（仅防灾难性退化，避免 CI 负载抖动）。
- `baselines.rs` 默认 `#[ignore]`，由 `scripts/bench-media.sh` 以 `--release --ignored` 运行。

## 诚实声明（未完成）

- 20 GiB 输入未在本机执行（磁盘与时长不可行）；当前证据是可缩放的单文件 256 MiB 哈希吞吐与 1s 样本，需在拥有大文件的 CI/机器上按 `bash scripts/bench-media.sh` 复测。
- 引擎峰值 RSS 已用 `/usr/bin/time -v` 包裹单个 CLI 调用实测（见上表）；大文件与并发下的峰值仍待复测。
- 真实/公开脱敏样本、预览期望图与播放器点击对照仍待补充；分析抽帧的帧选择语义属于未来独立 operation。
