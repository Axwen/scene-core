# SC-P2-03：engine 分发与 Manifest 校验

## 目的

把 `extract_audio_pcm` 接入 `run` 状态机，产出可被 Host 校验的音频 Manifest，并与 `version`/`doctor`/bundle 对齐。

## 范围

- `MediaBackend` 增加 `extract_audio_pcm(staging, control, audio_stream_index)`；`ProbeBackend` 实现，`UnavailableBackend` 返回 `ToolUnavailable`，测试假后端补齐。
- `run_session` 分发：进度 `accepted → progress(stage "audio", 0/1) → terminal`；失败 stage 为 `audio`（`failure_error` 按 operation 映射）。
- Manifest 由引擎组装（身份回传、`toolchainFingerprint`、`operationConfigHash`），`ArtifactManifest::validate` 通过。
- 错误映射：`MissingAudioStream` → `MISSING_AUDIO_STREAM`（非可重试）；`UnsupportedInput`/`ResourceLimit` 沿用矩阵。
- `implemented_operations()` 变为三项；`version --json`、doctor 的 engine 对齐、`build-windows-bundle.ps1` 生成的 manifest 与 `bundle-smoke.ps1` 随之更新。

## 验收标准

1. `tests/run_contract.rs`：音频成功会话事件数/终态/exit 0；错误路径 stage=`audio`；无音轨、资源限制、取消/超时沿用既有断言风格。
2. Manifest 身份回传与 `audioPcm` 槽位在真实工具链端到端用例中通过。
3. `version --json` 输出三项 implementedOperations；doctor 合成 bundle 用例通过；bundle 冒烟在 Windows CI 通过。
4. 既有 probe/preview 合同与 run 合同不回归。

## 实现状态（2026-09-21）

- `MediaBackend::extract_audio_pcm`（真实/Unavailable/测试假后端）；`run_session` 进度 `audio` 0/1、Manifest 组装与身份回传、失败 stage `audio`。
- 流选择（显式 index 或最低非附件音频流）、容器原点与流起点未知 → `UNSUPPORTED_INPUT`、输出预算预检、时长粗差拒绝、`presentationTimeMs = max(stream.startTimeMs, 0)`。
- `implemented_operations()` 三项；`version --json`、doctor 与 `bundle-smoke.ps1` 白名单同步。
- 测试：`run_contract` 22 项全绿，新增音频映射、正起点、显式选择、`MISSING_AUDIO_STREAM`（exit 3）四项。

## 依赖

SC-P2-01、SC-P2-02。
