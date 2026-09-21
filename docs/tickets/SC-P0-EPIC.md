# SC-P0：scene-core Protocol 0.1 与 Sidecar 基础 Epic

## 目标

在 `scene-core` 内建立可执行的 Rust workspace、版本化 JSON/JSONL 协议、输入身份与任务缓存推导契约，并验证 Windows x64 LGPL sidecar 的供应链和发布闭包。Phase 1 应能只依据 Spec、Schema 和 fixture 开始媒体执行，不再重新决定字段、路径、版本或缓存身份。

## 范围

- Protocol `0.1` 的 Start/Cancel、事件状态机、受控错误和 Manifest。
- `contentHash`、`inputFingerprint`、`sourceVersionId`、`derivationKey` 的分层语义。
- `TemporalSegment` 的 Scene/Shot 扩展 seam，不实现 segmentation 算法。
- Rust workspace、Schema、fixture、共享 identity DTO、`version`、`doctor`。
- FFmpeg 来源、许可、能力基线、SBOM、CVE/升级/回滚策略。
- Windows bundle 的 DLL closure、hash、干净环境验证。
- Host cache ownership 与跨仓库文档统一。

## 明确不做

P0 不实现真实 `run` 媒体操作、probe、preview、PCM、shot/scene 检测、缓存数据库、Tauri、AppContainer、Job Object、安装包、最终签名、AI Adapter 或 GitHub Issue 创建；这些能力在已创建的 P1 Epic 中交付。

## Tickets

| ID | 任务 | 依赖 |
|---|---|---|
| [SC-P0-01](SC-P0-01-workspace-toolchain-ci.md) | Workspace、toolchain 与 CI | 无 |
| [SC-P0-02](SC-P0-02-protocol-identity-segments.md) | Protocol DTO、输入身份、Scene/Shot seam、derivation key | 01 |
| [SC-P0-03](SC-P0-03-schema-fixtures-transcript.md) | Schema、fixture、transcript validator | 02 |
| [SC-P0-04](SC-P0-04-version-doctor-manifest.md) | `version`、`doctor`、package manifest 校验 | 01、03 |
| [SC-P0-05](SC-P0-05-ffmpeg-supply-chain.md) | FFmpeg 供应链与许可门禁 | 01 |
| [SC-P0-06](SC-P0-06-windows-bundle-spike.md) | Windows bundle 可复现验证 | 04、05 |
| [SC-P0-07](SC-P0-07-host-cache-contract-docs.md) | Host/cache 契约与文档统一 | 02、03、05 |

## 完成定义

- 7 张 Ticket 的验收条件全部通过。
- Linux CI 通过格式、编译、测试、lint 和 Schema drift；Windows CI 完成 bundle 验证；fixture manifest、错误矩阵和 CLI 输出契约可独立复核。
- 所有协议、输入身份、缓存和供应链 fixture 可被陌生实现者直接使用。
- 聚焦 Eng Review 无阻断项；不因本 Epic 触发 Design Review。

## 结项状态（2026-09-16）

| Ticket | 实现 | 证据 |
|---|---|---|
| SC-P0-01 workspace/toolchain/CI | 完成 | Linux/Windows CI 全绿；toolchain 固定 1.85.1 |
| SC-P0-02 DTO/身份/时间/seam | 完成 | 113 个测试；media_time/user 规则向量；derivation golden |
| SC-P0-03 Schema/fixture/runner | 完成 | 14 个生成式 Schema + drift gate；73 fixture（16 valid / 49 invalid / 8 golden）+ manifest runner；transcript 状态机 |
| SC-P0-04 version/doctor/manifest | 完成 | `version --json`/`doctor --json` 契约与实测；package manifest 校验 |
| SC-P0-05 供应链 | 完成（许可门禁另记） | lock（来源/size/SHA-256/source/flags/closure/fingerprint）、verify 脚本、策略文档、能力基线 364/538/228/531/44；[ADR-0001](../adr/0001-windows-toolchain-license.md) |
| SC-P0-06 Windows bundle | 完成 | CI 组包+PE 扫描+buildconf+双 ZIP 确定性+冒烟；用户干净 Windows 11 正向与四条负向实测通过；detached checksum |
| SC-P0-07 Host/cache 契约 | 完成 | host-cache-contract.md + 跨仓库/职责/引擎契约同步 |

聚合证据：main 分支最近 100 次 CI run 无失败；bundle 工件名 `scene-core-0.1.0-alpha.1-windows-x86_64.zip`。

### 仍然开放（不阻塞 P0 结项）

1. ~~对外分发前的许可复审~~ 已完成：用户 2026-09-21 决定接受 LGPL-3.0-or-later（不切换自建构建），scene-core 采用 MIT。对外分发前补齐 G1–G3 随包材料（GPLv3 正文、copyright notices、source offer 说明），见 [ffmpeg-lgpl-review.md](../legal/ffmpeg-lgpl-review.md)。
2. 静态 CRT 新构件建议在干净 Windows 上重跑一次 `version`/`doctor` 正向命令留档。
3. 聚焦 Eng Review：本 Epic 完成定义要求无阻断项；建议由用户按既有审查流程执行（可基于本表证据复核）。
4. P1 Engine Core 已在 [SC-P1-EPIC](SC-P1-EPIC.md) 中拆分并完成首轮实现；Windows 条件测试和后续 P2 门禁仍按该 Epic 跟踪。
