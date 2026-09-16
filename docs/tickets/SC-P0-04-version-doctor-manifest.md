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

## 依赖

SC-P0-01、SC-P0-03。
