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

## 依赖

SC-P2-01、SC-P2-02。
