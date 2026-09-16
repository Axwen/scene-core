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

不实现真实 `run` 媒体操作、probe、preview、PCM、shot/scene 检测、缓存数据库、Tauri、AppContainer、Job Object、安装包、最终签名、AI Adapter 或 GitHub Issue 创建。

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
