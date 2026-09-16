# SC-P0-04：实现 `version`、`doctor` 与 package manifest 校验

## 目的

提供消费者在启动和升级前可以自动调用的身份与完整性检查，不把“进程能启动”误当成 bundle 可用。

## 范围

- 实现 `scene-core version --json`，准确输出共享 `EngineIdentity` 与 `ToolchainIdentity`，包括 protocol、engine、target、engine cache compatibility、toolchain fingerprint 和 implemented operations。
- 实现 `scene-core doctor --json [--bundle-root <path>]`，检查 manifest、文件 hash/size、相对路径、工具可启动性、版本匹配、能力清单及临时目录创建/删除。
- 固化 `package-manifest.json`、包外可信 manifest digest/签名输入、detached checksum、SBOM/license 引用和逐文件排序规则。
- 对缺失、篡改、版本错配、未列出的 DLL 和工具不可启动返回非零与受控 nextStep。

## 验收标准

1. `version --json` 通过自身 Schema，Phase 0 的 `implementedOperations` 为空且不伪报 capability。
2. 完整 bundle 的 doctor 返回 `ok`/0；每种破坏 fixture 返回 `failed`/非零和稳定错误码。
3. manifest 不自引用；所有非自身 bundle 文件均有相对 path、byteSize、SHA-256。
4. doctor 不输出绝对路径、完整 argv、原始 stderr 或敏感信息；stdout 始终是单个稳定 JSON object，stderr 仅用于无法产生 JSON 的进程级故障。

## 实现状态（2026-09-16，增量一：DTO 与 Schema）

- `crates/scene-core-protocol` 新增 `package.rs`：`PackageManifest`、`PackagedEngine`、`PackagedTool`、`PackagedFile`、`DistributionProfile`；校验覆盖逐文件按 path 字节序排序、禁止自引用、engine/tools/toolchain-descriptor/capabilities/SBOM 引用必须出现在 files 中、工具顺序固定为 ffmpeg/ffprobe、sourceUrl 必须是不可变 https。
- 新增 `cli.rs`：`VersionOutput`（扁平输出共享 EngineIdentity 字段 + toolchainFingerprint + schemaVersion）、`DoctorOutput`/`DoctorCheck`/`DoctorStatus`/`DoctorCheckCode`；failed 检查必须带 code 与 nextStep，整体 status 与 checks 必须一致。
- `schemas/0.1/` 新增 `package-manifest.json`、`version.json`、`doctor.json`，共 12 个 Schema，全部由 DTO 生成并受 drift gate 保护。
- 新增严格字符串类型 `ToolName`、`LicenseExpression`、`DoctorCheckName`；测试总数 100。
- doctor 退出码约定：成功 0，失败取首个失败 check 的 `DoctorCheckCode` 退出码（`TEMP_DIR_UNAVAILABLE` 为 3，其余为 2）。

增量二（2026-09-16，engine CLI）：

- `scene-core-engine` 新增 lib + `[[bin]] name = "scene-core"`：`scene-core version --json`、`scene-core doctor --json [--bundle-root <path>] [--trusted-manifest-sha256 <digest>]`；stdout 恰好一个 JSON object，退出码与错误码/检查码矩阵一致。
- `version` 从可执行文件同目录读取 `toolchain-descriptor.json`；不可读时输出单个 `CliErrorOutput`（`TOOL_UNAVAILABLE`）并以 2 退出。
- `doctor` 顺序检查并在首个失败处停止：package-manifest、manifest-trust-anchor（缺失或与包外 digest 不符即失败）、toolchain-descriptor、bundle-files（size/hash/缺失/未列出文件）、engine-executable、engine-version、tool-executables、tool-versions、capabilities、temp-dir；真实工具执行走可注入 `CommandRunner`，测试使用假 runner。
- 新增 `CliErrorOutput` 与 `cli-error.json` Schema（共 13 个）；engine 侧 10 个测试覆盖健康 bundle、篡改/缺失/未列出文件、信任锚、工具与能力失败；测试总数 110。
- `doctor` 退出码：成功 0，失败取首个失败 check 的 code（`TEMP_DIR_UNAVAILABLE` 为 3，其余为 2）。

未完成：

- 真实 FFmpeg/FFprobe 启动、DLL closure、capabilities 与 `-buildconf` 比对验证依赖 SC-P0-05/06；`capabilities.json` 当前只做结构检查（五个能力列表）。

## 依赖

SC-P0-01、SC-P0-03。
