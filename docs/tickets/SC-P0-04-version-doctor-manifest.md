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

未完成：

- engine crate 的 `version`/`doctor` CLI、合成 bundle fixture 与命令执行抽象；包外信任锚（manifest digest）与真实 FFmpeg/FFprobe 启动、DLL closure 验证依赖 SC-P0-05/06。

## 依赖

SC-P0-01、SC-P0-03。
