# 媒体性能基线（首版）

设备/环境：WSL2 x86_64（Linux），debug 构建（`cargo test` 默认 profile），锁定工具链 `x86_64-unknown-linux-gnu`（BtbN `n9.0.1-31-g3a7c002718`）。
复现命令：

```bash
export SCENE_CORE_FFMPEG_DIR=<fetch-toolchain.sh 输出的 bin 目录>
cargo test -p scene-core-media --test media_acceptance -- --nocapture
```

| 指标 | 样本 | 结果 |
|---|---|---|
| 预览生成 | 640x480 testsrc 1s → JPEG 512x384 | 25 ms，17,687 字节（< 512 KiB 上限断言） |
| 流式 SHA-256 | 64 MiB 文件，debug 构建 | 1.24 s ≈ 52 MiB/s（断言 > 20 MiB/s 防退化） |
| probe | 合成 mkv（≤1s） | 与 ffprobe 进程启动同量级（未计入毫秒级 P50/P95） |

诚实声明：

- 这是 debug 构建的首版冒烟数据，不是发布性能承诺；FFmpeg I/O、20 GiB 大文件、release 构建的 P50/P95、峰值内存与磁盘预算仍待 SC-P1-06 后续补充。
- 峰值内存当前未采集（Go/`/usr/bin/time -v` 可由 CI 补充）。
