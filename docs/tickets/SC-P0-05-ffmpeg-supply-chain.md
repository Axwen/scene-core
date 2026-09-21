# SC-P0-05：固化 FFmpeg 供应链、许可、能力基线与升级回滚策略

## 目的

把 FFmpeg 从“下载两个 exe”提升为可审计、可升级、可回滚的第三方 toolchain，避免 bundle 发布后才发现许可、DLL 或能力差异。

## 范围

- 为每个 target 建立 `packaging/toolchains/<target>/toolchain.lock.json`，锁定不可变来源、下载/source hash、精确版本、configure flags、license profile 和构建信息。
- 生成 `toolchain-descriptor.json`、`capabilities.json`、SPDX SBOM、许可证正文、copyright notices 和源码/重链接材料说明。
- 记录并校验 demuxer/decoder/encoder/filter/protocol 基线；对意外新增能力同样失败，避免 LGPL profile 静默变成 GPL/nonfree。
- 设计 DLL closure、CVE 响应、升级 PR、保留和回滚规则；第三方构建不满足门禁时切换自建源码 CI。

## 验收标准

1. 不存在浮动 URL、未记录来源、GPL/nonfree flags 或未列出的非系统 DLL。
2. lock、能力清单、SBOM、许可和 toolchain fingerprint 可由 CI 重建并比较；bundle 使用具体精确 toolchain，不使用 `latest`。
3. 升级样例包含能力 diff、许可检查、CVE 处理、消费者兼容说明和旧版本回滚路径。
4. Windows/Linux toolchain 分开标识，不假设二进制或 Artifact hash 跨平台相同。

## 实现状态（2026-09-16）

- `packaging/toolchains/x86_64-pc-windows-msvc/toolchain.lock.json` 已锁定：BtbN/FFmpeg-Builds `autobuild-2026-09-15-13-18` 变体 `win64-lgpl-shared`（`n9.0.1-30-g9258bacca5`）、不可变归档 URL + 字节数 + SHA-256、上游 `n9.0.1` tag/commit 与源码归档 hash、三个构建脚本 hash、configure flags、DLL closure（7 个）与许可档位。
- 新增 `scripts/verify-toolchain-lock.sh`：核对归档 size/SHA-256 与 DLL closure；Linux CI 每次运行并缓存归档（`actions/cache`）。
- 新增 `packaging/toolchains/README.md`：lock 规则、镜像策略、升级回滚、CVE、capabilities.json 格式与第三方/自建切换门禁。

许可决定（2026-09-21）：

- 该候选构建使用 `--enable-version3`，实际许可档位是 **LGPL-3.0-or-later**（不是 package manifest 示例中的 LGPL-2.1-or-later）。仍属 LGPL、无 GPL/nonfree 组件。
- 用户决定接受 LGPL-3.0-or-later，不切换自建 `--disable-version3` 构建；scene-core 自身以 MIT 发布。决策记录见 [ADR-0001](../adr/0001-windows-toolchain-license.md) 与 [ffmpeg-lgpl-review.md](../legal/ffmpeg-lgpl-review.md)。
- 对外分发前补齐随包材料：GPLv3 正文、copyright notices、面向收件人的 source offer 说明（复审材料 G1–G3）；不改变工具链与 `toolchainFingerprint`。

未完成：

- `capabilities.json` 精确基线与 SBOM 需要运行固定工具链（Windows），归 SC-P0-06 Windows CI 生成后入库并启用逐项比较。
- `-buildconf` 无 GPL/nonfree 核验、PE import closure 扫描与干净环境验证属 SC-P0-06。

## 依赖

SC-P0-01。
