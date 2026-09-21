# ADR-0001：Windows 工具链许可档位（BtbN LGPLv3）

日期：2026-09-16
状态：已接受 LGPL-3.0-or-later（内部与对外分发；用户 2026-09-21 决定）；scene-core 自身采用 MIT（见 `LICENSE`）

## 背景

- SC-P0-05 锁定的候选构建为 BtbN/FFmpeg-Builds `win64-lgpl-shared` 变体（`n9.0.1-30-g9258bacca5`）。
- 该变体构建脚本使用 `--enable-version3`，并随包 `COPYING.LGPLv3`；FFmpeg 的 `--enable-version3` 会把 (L)GPL 升到版本 3，因此实际许可档位是 **LGPL-3.0-or-later**，不是示例中的 LGPL-2.1-or-later。
- 项目当前定位为**内部使用**：不向本法律实体之外提供二进制。
- 替代方案：切换为仓库自建、`--disable-version3` 的 LGPL-2.1+ 构建；或在未来更换供应商。

## 决策

1. 接受 BtbN `win64-lgpl-shared` 候选作为 Phase 0/1 的内部使用工具链，仍属 LGPL、无 GPL/nonfree 组件。
2. lock、package manifest、SBOM 与许可证材料**必须如实记录** `LGPL-3.0-or-later`，不得沿用 LGPL-2.1 示例值。
3. `distributionProfile` 仍是粗粒度 `lgpl`；版本差异由 SPDX 表达式承担。
4. 2026-09-21 用户决定：接受 LGPL-3.0-or-later，不切换到自建 `--disable-version3` 构建；scene-core 自身以 MIT 发布。对外分发前补齐随包材料（GPLv3 正文、copyright notices、source offer 说明，见复审材料 G1–G3），不改变工具链与 `toolchainFingerprint`。

## 后果

- 内部使用不触发 LGPL 的分发义务（公开的是源码与文档，不是 FFmpeg 二进制）。
- 一旦对外分发（客户、合作方、公开发布 bundle，或把归档镜像为公开 release 资产），必须履行许可证正文、对应源码/重链接材料，以及 LGPLv3 的安装信息（反 Tivoization）等要求。
- 更换构建（包括改自建 v2.1）会改变 toolchain descriptor 与 `toolchainFingerprint`，从而更换 `derivationKey` 并失效缓存；这是预期行为，必须按升级 PR 流程执行并附能力 diff。

## 复审门禁

2026-09-21 已由用户选择：接受 LGPL-3.0-or-later（不采用自建 v2.1 路径）。

剩余动作（对外分发前，不改变工具链）：按复审材料的 G1–G3 补齐随包材料（GPLv3 正文、copyright notices、面向收件人的 source offer 说明）。

复审材料（事实、义务对照、选项与决策清单）见 [FFmpeg 工具链对外分发许可复审材料](../legal/ffmpeg-lgpl-review.md)。

## 参考

- `packaging/toolchains/x86_64-pc-windows-msvc/toolchain.lock.json`（`license` 段）
- `packaging/toolchains/README.md`（许可与 CVE 段）
- `docs/tickets/SC-P0-05-ffmpeg-supply-chain.md`、`docs/tickets/SC-P0-06-windows-bundle-spike.md`
