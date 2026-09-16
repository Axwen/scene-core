# 手工验证流程（Phase 1）

用于人工核对 bundle 与真实/异常媒体的行为。所有样本均为合成、无版权风险，仓库内位于
`fixtures/media/samples/`（也可用 `scripts/make-media-samples.sh` 重新生成，需要锁定工具链）。

## 前置

1. 从最新成功的 `windows-bundle` run 下载 artifact 并解压：`gh run download -n scene-core-0.1.0-alpha.1-windows-x86_64`（或 Actions 页面 Artifacts）。
2. 取样本（二选一）：
   - 已有仓库克隆：`git pull`；
   - 单独下载：`https://raw.githubusercontent.com/Axwen/scene-core/main/fixtures/media/samples/<file>`。
3. 只需解压后的 bundle + PowerShell；本流程不需要 Rust/Python/系统 FFmpeg。

## A. 健康与完整性

```powershell
cd <解压后的 bundle>
$sha = (Get-Content package-manifest.json.sha256).Split(" ")[0]
.\bin\scene-core.exe version --json
.\bin\scene-core.exe doctor --json --bundle-root . --trusted-manifest-sha256 $sha
```

预期：`version` 显示 `implementedOperations: ["probe","extract_preview"]`；`doctor` 十项全 `ok`、退出码 0。

负向（对副本操作，原始 bundle 不动）：

```powershell
Copy-Item -Recurse . ..\bundle-tampered
[IO.File]::AppendAllText("..\bundle-tampered\bin\ffmpeg.exe", "x")
.\bin\scene-core.exe doctor --json --bundle-root ..\bundle-tampered --trusted-manifest-sha256 $sha
# 预期 exit=2、status=failed、最后一个 check code=BUNDLE_FILE_INVALID
```

## B. 样本行为（`scripts/run-sample.ps1`）

```powershell
.\scripts\run-sample.ps1 -BundleRoot <bundle> -Sample fixtures\media\samples\<file> -Operation probe
.\scripts\run-sample.ps1 -BundleRoot <bundle> -Sample fixtures\media\samples\baseline-cfr.mp4 -Operation extract_preview
```

预期（详见 `fixtures/media/samples/manifest.json`）：

| 样本 | Operation | 预期 |
|---|---|---|
| baseline-cfr.mp4 | probe | completed；video/audio start 0ms，mpeg4+aac |
| av-offset-80ms.mkv | probe | completed；audio `startTimeMs=80`，video 0（共享原点） |
| vfr.mkv | probe | completed；起点 0，无时间回退误报 |
| bframes.mkv | probe | completed；`averageFrameRate=10/1` |
| rotation-90.mkv | probe | completed；`rotationDegrees=90`，显示 64x48 |
| attached-picture.flac | probe | completed；`attachedPicture=true`，`primaryVideoStreamIndex=null` |
| video-only.mkv | probe | completed；无音频流 |
| audio-only.m4a | extract_preview | failed、`MISSING_VIDEO_STREAM`、退出码 3 |
| corrupt.mp4 | probe | failed、`CORRUPT_MEDIA`、退出码 3，无 Artifact |
| baseline-cfr.mp4 | extract_preview | completed；opening 0ms 与 midpoint 500ms，512 边长内缩放，JPEG |

请把每个样本的 `exit` 与最后一个 check/错误码记到本文件末尾的“实测记录”。

## C. 播放器对照（需人工）

1. 用常用播放器打开 `baseline-cfr.mp4`，记录画面上有明显变化的时刻（例如起始与约 0.5s）。
2. 对同一样本跑 `extract_preview`，打开 `output/preview/opening.jpg` 与 `midpoint.jpg`。
3. 在播放器跳到 0s 与 `floor(duration/2)`（本样本 500ms），确认画面与对应 JPEG 一致（合成 testsrc 图案可辨认）。

## D. 实测记录

### 2026-09-16 · Windows 11 x64（干净、非管理员）· bundle artifact（engineCommit b3e46ad）

命令：`gh run download -n scene-core-0.1.0-alpha.1-windows-x86_64`；`$sha` 取自 `package-manifest.json.sha256`。

| 步骤 | 结果 |
|---|---|
| `version --json` | `implementedOperations: ["probe","extract_preview"]`，engineCommit `b3e46ad` |
| `doctor --json --trusted-manifest-sha256 $sha` | `status: ok`，十项全 `ok`，exit 0 |

| 样本 | Operation | 观测 | 判定 |
|---|---|---|---|
| baseline-cfr.mp4 | probe | video/audio start 0，mpeg4+aac，duration 1000 | ✅ |
| av-offset-80ms.mkv | probe | video 0ms、**audio startTimeMs=80** | ✅ 共享原点 |
| vfr.mkv | probe | start 0，avg 10/1 | ✅ |
| bframes.mkv | probe | start 0，avg 10/1 | ✅ |
| rotation-90.mkv | probe | **rotationDegrees=90**，显示 64x48 | ✅ |
| attached-picture.flac | probe | attachedPicture=true、**primaryVideoStreamIndex=null** | ✅ |
| audio-only.m4a | extract_preview | failed、`MISSING_VIDEO_STREAM`、exit 3 | ✅ |
| corrupt.mp4 | probe | failed、`CORRUPT_MEDIA`、exit 3、无产物 | ✅ |
| baseline-cfr.mp4 | extract_preview | opening 0ms / midpoint 500ms，均 64x48（不放大），presentationTimeMs null，字节 1465/1450 | ✅ |

待补：`video-only.mkv` probe；本 bundle 的 tamper 负向；播放器点击对照（C）。
