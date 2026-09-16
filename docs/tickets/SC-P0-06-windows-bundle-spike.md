# SC-P0-06：验证 Windows x64 LGPL bundle 的 DLL closure 与可复现门禁

## 目的

证明消费者可携带一个自包含、可校验的 Windows sidecar，而不是依赖开发机 PATH 或系统 FFmpeg。

## 范围

- 生成 `scene-core-0.1.0-alpha.1-windows-x86_64.zip`，包含 `scene-core.exe`、FFmpeg/FFprobe、所需 DLL、Schema、toolchain descriptor、capabilities、manifest、SBOM 和许可证。
- 执行 PE import/DLL closure 扫描、逐文件 hash、ZIP/manifest detached checksum、包外可信 manifest digest/签名验证和能力基线检查。
- 在干净 Windows 11 x64、普通用户、无 Rust/Python/系统 FFmpeg/PATH 修改的环境中运行 `version` 与 `doctor`。
- 记录第三方构建 spike；若其来源、许可、源码可得性或闭包不达标，切换仓库控制的源码构建，不降低门禁。

## 验收标准

1. ZIP 解压后 `version`/`doctor` 成功；缺失、篡改、未列 DLL 的负向测试失败。
2. manifest 覆盖全部非自身文件，路径按字节序排序；包外可信 digest/签名先验证 manifest，再由 manifest 验证文件，detached checksum 可独立复核。
3. 无 GPL/nonfree 组件、PATH 依赖、管理员权限要求或未记录的动态库。
4. bundle provenance 能回指精确版本、来源 hash、configure flags、能力清单和许可材料。

## 实现状态（2026-09-16，增量一：descriptor 与 fingerprint 链路）

- 新增 `ToolchainDescriptor`/`ToolchainLibrary` DTO（协议 crate）：bundle 内 `toolchain-descriptor.json` 携带 target、FFmpeg/FFprobe 版本、lock 文件 SHA-256、configure flags（校验拒绝 `--enable-gpl`/`--enable-nonfree`）、能力集摘要和 DLL closure。
- `toolchainFingerprint` 重新定义为 **descriptor 文件字节的 SHA-256**，descriptor 自身不含该字段，任何人可独立复算；engine `read_toolchain_identity` 由 descriptor 派生共享 `ToolchainIdentity`。
- doctor 新增能力摘要校验：`capabilities.json` 的文件摘要必须等于 descriptor 记录的 `capabilitySetFingerprint`。
- Schema 增至 14 个（新增 `toolchain-descriptor.json`）；engine 合成 bundle 测试已改用 descriptor。

增量二（2026-09-16，Windows 打包与冒烟）：

- `scripts/build-windows-bundle.ps1`：校验 lock 归档 size/SHA-256，按白名单组装 bundle（scene-core.exe、ffmpeg/ffprobe、7 个 DLL、许可证、Schema），运行固定工具生成 `capabilities.json`、`toolchain-descriptor.json`（fingerprint = 文件字节 SHA-256）、SPDX SBOM 和 `package-manifest.json`，并输出 manifest/ZIP detached checksum。
- 能力基线已固化：`packaging/toolchains/x86_64-pc-windows-msvc/capabilities.json`（364 demuxers / 538 decoders / 228 encoders / 531 filters / 44 protocols），打包时逐字节比较，漂移即失败。
- `scripts/bundle-smoke.ps1`：健康 bundle 的 `version`/`doctor` 通过；篡改工具、删除 DLL、新增未列出文件、错误信任锚全部 fail closed 且带稳定 code。
- CI 新增 `windows-bundle` job（构建 → 打包 → 冒烟 → 上传 ZIP 与 checksum 工件）；bundle 产物已加入 `.gitignore`。

未完成：

- 原始 PE import 表扫描（当前为白名单组装 + DLL closure hash + 运行时启动验证的组合门禁）；ZIP 字节级可复现；SBOM 细化到 libav* 组件；干净 Windows 11 无依赖实测与最终许可确认由用户执行。

## 依赖

SC-P0-04、SC-P0-05。
