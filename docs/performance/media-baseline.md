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

debug 构建对照（较慢，仅参考）：64 MiB 哈希 ≈52 MiB/s；预览 25 ms。

## 断言与门槛

- `media_acceptance.rs` 常态断言：预览 ≤512 KiB、最长边 512、非等比不裁剪；哈希吞吐 >2 MiB/s（仅防灾难性退化，避免 CI 负载抖动）。
- `baselines.rs` 默认 `#[ignore]`，由 `scripts/bench-media.sh` 以 `--release --ignored` 运行。

## 诚实声明（未完成）

- 20 GiB（或缩比大文件）的 P95 吞吐/CPU/磁盘预算尚未纳入 CI；当前仅有 256 MiB 哈希与 1s 样本。
- 峰值内存为 harness 进程 RSS，不是引擎独立常驻内存；需要更细的采样（子进程 getrusage/`/usr/bin/time` 包裹单个 CLI 调用）。
- 真实/公开脱敏样本、预览期望图与播放器点击对照仍待补充；分析抽帧的帧选择语义属于未来独立 operation。
