# 工具链供应链策略

本目录按 target 锁定 FFmpeg/FFprobe 的不可变来源与完整性数据。bundle 只能使用 lock 中记录的精确工具链；没有任何浮动下载。

## Lock 规则

- `packaging/toolchains/<target>/toolchain.lock.json` 记录：不可变下载 URL、字节数、SHA-256、供应商与构建变体、上游 source tag/commit 与源码归档 hash、configure flags、许可档位和构建时间。
- 禁止 `latest` 浮动 URL、GPL/nonfree flags 或组件、未记录的动态库、PATH 或系统安装依赖。
- 供应商 URL 失效时，把原归档镜像到 scene-core release asset，保持同一 SHA-256，并在 lock 同时记录原 URL 与镜像 URL。
- 升级以显式 PR 进行：更新 lock、生成能力 diff、核对许可与 CVE、保留旧 lock/bundle 到消费者升级完成；不允许原地覆盖已发布的 bundle。

## 验证

- `scripts/verify-toolchain-lock.sh <target>`：下载 lock 中的两个归档，核对字节数与 SHA-256，并比较 DLL closure；Linux CI 每次运行（带 archive cache）。
- Linux 测试工具链同样锁定在 `packaging/toolchains/x86_64-unknown-linux-gnu/`；`scripts/fetch-toolchain.sh <target> <dest>` 校验后解压并输出 bin 目录，Linux CI 用它设置 `SCENE_CORE_FFMPEG_DIR` 并运行 `scene-core-media` 合同测试。
- Windows CI 在打包后执行：`scripts/pe-imports.py` 解析每个 PE 的 import 表，非系统导入必须都在 bundle 内、descriptor 记录的库必须存在且被实际导入、不得有未记录的 DLL；`bundle-smoke.ps1` 校验 `ffmpeg -buildconf` 包含全部记录的 configure flags 且无 `--enable-gpl`/`--enable-nonfree`。
- engine 以 `-C target-feature=+crt-static` 构建，避免依赖 VC++ 运行库（如 `VCRUNTIME140.dll`）；`scene-core.exe` 只允许导入 Windows 系统 DLL。
- ZIP 使用固定时间戳与 ordinal 排序生成，打包脚本会连打两次并比较 SHA-256，字节不一致即失败。

## capabilities.json 格式（SC-P0-06 生成）

```json
{
  "capabilitiesVersion": "1",
  "target": "x86_64-pc-windows-msvc",
  "demuxers": [],
  "decoders": [],
  "encoders": [],
  "filters": [],
  "protocols": []
}
```

- 名称小写、去重、按字节序排序；由固定工具链的 `-formats`、`-codecs`、`-filters`、`-protocols` 归一化生成。
- 基线随仓库提交在 `packaging/toolchains/<target>/capabilities.json`；打包脚本对生成结果与基线做逐字节比较，缺失与意外新增都失败；升级工具链时必须作为显式 PR 更新基线并附能力 diff。

## 当前候选（2026-09-16）

- 来源：BtbN/FFmpeg-Builds，release tag `autobuild-2026-09-15-13-18`，变体 `win64-lgpl-shared`，`n9.0.1-30-g9258bacca5`。
- 许可：`--enable-version3` 使该构建适用 **LGPL-3.0-or-later**（非 LGPL-2.1）；无 GPL/nonfree 组件。首次发布前必须由法律/合规确认，并随 bundle 附许可正文、copyright notices 与对应源码获取说明。
- 能力基线与 SBOM 在 SC-P0-06 Windows 运行后生成并入库。

## 许可与 CVE

- 触发点在分发：仅在组织内部使用、不向第三方（含公开发布 bundle 或镜像归档）提供二进制时，LGPL-2.1 与 LGPL-3.0 的差异基本不产生额外义务；本仓库公开不影响这一点，因为公开的是源码与文档，不是 FFmpeg 二进制。
- 一旦对外分发（客户、合作方、公开发布 bundle/镜像），LGPL 档位义务生效：许可证正文、对应源码/重链接材料、修改说明；LGPL-3.0 另有安装信息（反 Tivoization）等要求。对外分发前必须确认；本文件不构成法律意见。
- 监控 FFmpeg 及随附库的 CVE；受影响版本不得静默覆盖，以新 bundle 版本修复并记录升级说明。
- 第三方构建不能满足来源、许可、源码可得性、能力清单或 DLL closure 任一门禁时，切换仓库控制的源码构建 CI，不降低验收标准。
