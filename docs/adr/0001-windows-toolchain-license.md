# ADR-0001：Windows 工具链许可档位（BtbN LGPLv3）

日期：2026-09-16
状态：内部使用已接受；对外分发前必须复审（见“复审门禁”）

## 背景

- SC-P0-05 锁定的候选构建为 BtbN/FFmpeg-Builds `win64-lgpl-shared` 变体（`n9.0.1-30-g9258bacca5`）。
- 该变体构建脚本使用 `--enable-version3`，并随包 `COPYING.LGPLv3`；FFmpeg 的 `--enable-version3` 会把 (L)GPL 升到版本 3，因此实际许可档位是 **LGPL-3.0-or-later**，不是示例中的 LGPL-2.1-or-later。
- 项目当前定位为**内部使用**：不向本法律实体之外提供二进制。
- 替代方案：切换为仓库自建、`--disable-version3` 的 LGPL-2.1+ 构建；或在未来更换供应商。

## 决策

1. 接受 BtbN `win64-lgpl-shared` 候选作为 Phase 0/1 的内部使用工具链，仍属 LGPL、无 GPL/nonfree 组件。
2. lock、package manifest、SBOM 与许可证材料**必须如实记录** `LGPL-3.0-or-later`，不得沿用 LGPL-2.1 示例值。
3. `distributionProfile` 仍是粗粒度 `lgpl`；版本差异由 SPDX 表达式承担。

## 后果

- 内部使用不触发 LGPL 的分发义务（公开的是源码与文档，不是 FFmpeg 二进制）。
- 一旦对外分发（客户、合作方、公开发布 bundle，或把归档镜像为公开 release 资产），必须履行许可证正文、对应源码/重链接材料，以及 LGPLv3 的安装信息（反 Tivoization）等要求。
- 更换构建（包括改自建 v2.1）会改变 toolchain descriptor 与 `toolchainFingerprint`，从而更换 `derivationKey` 并失效缓存；这是预期行为，必须按升级 PR 流程执行并附能力 diff。

## 复审门禁

首次对外分发前二选一：

1. 由用户/法律确认接受 LGPL-3.0-or-later 义务；或
2. 切换到仓库自建 `--disable-version3` 构建并更新 lock、能力基线、SBOM 与许可材料。

## 参考

- `packaging/toolchains/x86_64-pc-windows-msvc/toolchain.lock.json`（`license` 段）
- `packaging/toolchains/README.md`（许可与 CVE 段）
- `docs/tickets/SC-P0-05-ffmpeg-supply-chain.md`、`docs/tickets/SC-P0-06-windows-bundle-spike.md`
