# FFmpeg 工具链对外分发许可复审材料

状态：待用户/法律确认（2026-09-21 准备）。对应 [ADR-0001](../adr/0001-windows-toolchain-license.md) 的“复审门禁”，用于首次对外分发前的二选一决策。

本文只整理事实、义务与选项，**不构成法律意见**；最终结论由用户/法律确认。

## 1. 分发触发条件

以下任一情形视为“对外分发”，触发 LGPL 义务：

- 向本法律实体之外的客户/合作方提供 bundle（ZIP 或解压目录）；
- 公开发布 bundle、把归档镜像为公开 release 资产或公开下载链接；
- 把 FFmpeg 二进制嵌入任何对外提供的安装包/设备镜像。

仅内部使用（不向第三方提供二进制）不触发分发义务；公开源码与文档本身不构成 FFmpeg 二进制分发。

## 2. 构件事实与证据

| 事实 | 值 | 证据 |
|---|---|---|
| 供应商/变体 | BtbN/FFmpeg-Builds `win64-lgpl-shared`，release `autobuild-2026-09-15-13-18` | `packaging/toolchains/x86_64-pc-windows-msvc/toolchain.lock.json` |
| 版本 | `n9.0.1-30-g9258bacca5`（上游 tag `n9.0.1`，commit `3a7c002718…`） | lock `ffmpegVersion`/`source` |
| configure flags | `--enable-version3 --disable-debug --enable-shared --disable-static`；无 `--enable-gpl`/`--enable-nonfree` | lock `build.configureFlags`、CI `bundle-smoke.ps1` 的 `-buildconf` 校验 |
| 许可档位 | **LGPL-3.0-or-later**（`--enable-version3` 将 (L)GPL 升到 v3） | lock `license.profile`、`toolchain-descriptor.json`、package manifest `tools[].licenseProfile`、SBOM `licenseConcluded` |
| 链接方式 | 动态链接（7 个 libav* shared DLL + ffmpeg/ffprobe exe） | lock `archive.sharedLibraries`、`pe-imports.py` 扫描、doctor DLL 闭包校验 |
| 随包许可正文 | `THIRD_PARTY_LICENSES/COPYING.LGPLv3`（LGPLv3 全文，7651 字节，SHA-256 `da7eabb7…`） | `scripts/build-windows-bundle.ps1`、lock `license.licenseFile/Sha256` |
| SBOM | SPDX-2.3：scene-core、ffmpeg、ffprobe 及 7 个 libav* 各自一个 package，`filesAnalyzed:false` | `build-windows-bundle.ps1` 生成的 `SBOM.spdx.json` |
| 修改情况 | 未修改 FFmpeg 源码，直接使用供应商预编译构件 | 供应商构建脚本 hash 记录在 lock `build.variantScripts` |
| 引擎自身 | 无 LICENSE 文件；SBOM 中 scene-core `licenseConcluded=NOASSERTION` | 仓库根目录、SBOM |

CI 门禁（每次 bundle 构建执行）：`verify-toolchain-lock.sh`（归档 size/SHA-256 + DLL 闭包）、`pe-imports.py`（非系统导入必须在 bundle 内且被记录）、`bundle-smoke.ps1`（`-buildconf` 无 GPL/nonfree、doctor 十项、篡改/缺失/未列出 fail closed）、确定性 ZIP。

## 3. LGPLv3 义务对照（选项 A：接受 LGPL-3.0-or-later）

| 义务（LGPLv3 §4 等） | 现状 | 缺口/动作 |
|---|---|---|
| a) 显著声明使用了 Library 且受 LGPL 覆盖 | SBOM + `THIRD_PARTY_LICENSES/` 目录已有，但无面向收件人的声明文本 | 新增 `THIRD_PARTY_LICENSES/README`（或 NOTICE），声明组件、版本、许可与源码获取方式 |
| b) 随附 **GPLv3 与 LGPLv3 两份正文** | 只有 LGPLv3 正文 | **缺口 G1**：补充 GPLv3 正文（如 `COPYING.GPLv3`），并让 manifest/SBOM 覆盖 |
| c) 保留版权声明（copyright notices） | SBOM 无 `copyrightText`，bundle 无 NOTICE | **缺口 G2**：生成/随附 copyright notices（上游 `LICENSE.txt` 之外，建议汇总 FFmpeg 及随附库版权行） |
| d) 动态链接机制（§4(d)(1)） | shared DLL，运行时使用 bundle 内副本，接口兼容的修改版可替换 | 已满足；需在声明中说明可替换 DLL |
| 对应源码获取说明（GPLv3 §6 路径 0 或 d)(1) 不需要源码，但建议提供） | lock 记录精确源码归档 URL + SHA-256、构建脚本 hash | **缺口 G3**：对外材料写明 source offer（URL + hash + 保留期），并把归档镜像到本项目 release asset 防上游删除 |
| 修改说明 | 未修改 | 在声明中明示 “no modifications” |
| Installation Information（反 Tivoization，GPLv3 §6） | 不适用或未确认 | **待确认 G4**：仅当分发形态构成 “User Product” 时需要；桌面软件 bundle 通常不构成，需法律确认 |
| 不得附加限制用户行使 LGPL 权利的条款 | 尚无 EULA/服务条款 | **缺口 G5**：对外 EULA/条款需法律审阅；scene-core 自身许可也需确定（当前 NOASSERTION） |
| 专利许可 | LGPLv3 自带专利授权 | 接受该档位即接受 |

## 4. 选项对比

### A. 接受 LGPL-3.0-or-later（推荐，若法律确认）

- 工作量：补齐 G1/G2/G3 的随包材料 + G5 条款审阅；不改工具链、不改 `toolchainFingerprint`、不失效缓存。
- 风险：LGPLv3 比 v2.1 多出安装信息/专利终止等条款；对桌面 sidecar 通常可控，但需法律确认 G4。

### B. 切换自建 `--disable-version3`（LGPL-2.1-or-later）

- 工作量：新增源码构建 CI（可复现、能力基线、DLL 闭包、SBOM、许可材料全部重做）。
- 影响：新 `toolchainFingerprint` → 全部 `derivationKey` 变化、缓存失效（预期行为，按升级 PR 流程并附能力 diff）。
- 收益：许可档位回到 LGPL-2.1+，义务相对轻；但**不消除** LGPL 的源码/通知义务，只是去掉 v3 特有条款。

两个选项都不改变“无 GPL/nonfree 组件”的事实（已由 CI 门禁保证）。

## 5. 可复核命令

```bash
# 归档完整性 + DLL 闭包（下载 lock 中的两个归档）
bash scripts/verify-toolchain-lock.sh x86_64-pc-windows-msvc

# bundle 内许可正文与 lock 记录一致
sha256sum packaging/bundles/windows-x86_64/bundle/THIRD_PARTY_LICENSES/COPYING.LGPLv3
# 期望：da7eabb7bafdf7d3ae5e9f223aa5bdc1eece45ac569dc21b3b037520b4464768

# PE 导入闭包（非系统导入必须在 bundle 内）
python3 scripts/pe-imports.py packaging/bundles/windows-x86_64/bundle

# buildconf 无 GPL/nonfree、doctor 正负向
$sha = (Get-Content packaging/bundles/windows-x86_64/package-manifest.json.sha256).Split(" ")[0]
./scripts/bundle-smoke.ps1 -BundleRoot packaging/bundles/windows-x86_64/bundle -TrustedManifestSha256 $sha
```

## 6. 决策清单

- [ ] 确认分发形态（内部 / 客户 / 公开 release），以及是否可能随硬件或消费设备提供（G4）。
- [ ] 选择 A 或 B；若选 A，确认接受 LGPL-3.0-or-later。
- [ ] A 路径：指定 source offer 的托管位置与保留期（建议本项目 release asset 镜像 + 至少 3 年）。
- [ ] A 路径：确定 scene-core 自身许可与对外条款（G5），确保不限制 LGPL 权利。
- [ ] B 路径：为自建构建排期，并接受 `toolchainFingerprint`/缓存失效与能力基线重做。
- [ ] 无论哪条路径：把 G1/G2/G3 材料加入 bundle 并在 CI 校验（manifest 列出、hash 记录）。

## 7. 关联文档

- [ADR-0001：Windows 工具链许可档位](../adr/0001-windows-toolchain-license.md)
- [SC-P0-05：FFmpeg 供应链](../tickets/SC-P0-05-ffmpeg-supply-chain.md)、[SC-P0-06：bundle 门禁](../tickets/SC-P0-06-windows-bundle-spike.md)
- [工具链供应链策略](../../packaging/toolchains/README.md)、[Protocol 0.1 Spec §11](../specs/scene-core-phase0-protocol-0.1.md)
