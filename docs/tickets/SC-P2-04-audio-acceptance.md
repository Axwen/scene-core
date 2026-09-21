# SC-P2-04：音频验收与性能

## 目的

用可复核证据关闭 `extract_audio_pcm`：映射正确、边界受控、资源可预期。

## 范围

- 合成样本（`scripts/make-media-samples.sh` 或测试内生成）：已知 tone 起点、正/负音轨起点、多音轨、无音轨、时间戳异常。
- 映射验证：对已知声画事件断言 `presentationTimeMs + n/sampleRate` 偏差 ≤ 1 ms；ASR 局部时间回映示例（请求/实际起点）写入验收记录。
- 真实样本（可选、不入库）：与 ffprobe 对照采样率/声道/时长；记录映射与偏差。
- 资源：输出体积、抽取吞吐、峰值内存；把音频抽取纳入 `performance_smoke_gate`（时长下限/吞吐下限，非发布 SLA）。
- 取消/超时/只读输出目录：沿用 SC-P1-05 的终态、退出码、无产物断言。

## 验收标准

1. 验收记录（`docs/acceptance/audio-acceptance.md`）包含命令、样本特征、映射偏差与资源数据，明确未测项。
2. 边界用例全部有进程级证据：无音轨、不可靠时间轴、预算超限、取消/超时。
3. 性能门禁覆盖音频抽取并接入 CI；阈值只作回归冒烟。
4. 失败/取消/超时/部分产物不发布、不可缓存，有测试锁定。

## 实现状态（2026-09-21）

- 验收记录：[audio-acceptance.md](../../acceptance/audio-acceptance.md)。
- 映射偏差：已知阶跃 0.5 s 与正起点 80 ms 均在 1 ms 内；负起点为单元级 clamp。
- 边界：无音轨（`MISSING_AUDIO_STREAM`）、只读输出（`RESOURCE_LIMIT`）、deadline 124、取消，均有进程级证据且无产物发布。
- 性能：音频纳入 `performance_smoke_gate`（30 s → 843× realtime，floor 10×）；Windows bundle 冒烟新增 `extract_audio_pcm` 端到端。
- 未覆盖（记录中明示）：真实压缩音频的编码延迟、时间戳复位真实样本、>2 GiB 实测、本机 Windows 复测。

## 依赖

SC-P2-02、SC-P2-03。
