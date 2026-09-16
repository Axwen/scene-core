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

## 依赖

SC-P0-01。
