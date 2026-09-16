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

## 依赖

SC-P0-04、SC-P0-05。
