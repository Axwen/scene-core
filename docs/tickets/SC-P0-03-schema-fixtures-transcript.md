# SC-P0-03：生成 JSON Schema、Transcript Validator 与协议 fixture

## 目的

让 Rust DTO、Schema 和跨进程 JSONL 行为由同一组可重复 fixture 驱动，陌生消费者无需阅读实现细节即可验证兼容性。

## 范围

- 生成 `schemas/0.1/`：control message、event、normalized media、Manifest、TemporalSegment、InputSetDescriptor、DerivationDescriptor、version、doctor、package manifest、toolchain descriptor、capabilities。
- 建立 `fixtures/protocol/valid`、`invalid`、`golden-jsonl` 和缓存/身份 fixture；至少 10 个 valid、20 个 invalid、6 个 golden transcript；增加 `fixture-manifest.json`，逐文件映射规则、Schema、预期结果和错误码。
- 实现 transcript validator：sequence 从 1 开始且连续，accepted 后恰一个终态，终态后无事件，terminal/exit code 一致。纯状态机（含身份回传一致性）已由 SC-P0-02 在 `scene-core-protocol` 的 `EventStreamValidator` 中实现；本 Ticket 负责 fixture 驱动的一致性验证、golden transcript 与 runner。
- 覆盖 BOM、重复 key、1 MiB 行上限、第二个 start/cancel、错误 requestId、取消、超时、部分结果隔离、符号链接/reparse/ADS 路径和 Schema drift。
- 将[时间规则验收向量](../specs/media-time-policy-draft.md#7-验收样本与测试)中的纯转换/协议案例纳入 fixture manifest；真实媒体、未来音频及 Seek 播放验证按阶段归属，不伪装成 Phase 0 已通过。

## 验收标准

1. 重新生成 Schema 无 diff；每个 fixture 报告预期 accepted/rejected 及错误码。
2. valid/invalid/golden fixture 在 Linux CI 运行，并可被未来非 Rust consumer 复用。
3. validator 拒绝坏序号、坏终态、终态后事件、身份不一致和可发布的 partial Artifact。
4. canonical JSON 的字段顺序、Unicode、null 和 hash 测试有固定 golden 结果；每个 fixture 可由 manifest 追溯到唯一规则和错误码。
5. Schema 允许有符号 stream.startTimeMs，但拒绝负公共请求时间；container.startTimeMs 只接受 0/null。向量覆盖 80ms 音视频偏移、负原点、未知原点、±1.5ms 舍入及 [1.2,1.8)ms 向外取整。

## 实现状态（2026-09-16，增量一：Schema）

- `schemars` 已作为 `scene-core-protocol` 的常规依赖：DTO 派生 `JsonSchema`，严格字符串类型（Identifier、Sha256Digest、ProtocolVersion、InputRef、UtcTimestamp 等）手写 `JsonSchema` 并带 pattern/const/format 约束；`container.startTimeMs` 在 Schema 中为 `maximum: 0` 且允许 null。
- `tests/schema_contract.rs` 从 DTO 生成并对比 `schemas/0.1/*.json`；`scripts/check-schema-drift.sh` 已替换为生成式 drift gate，CI 现有的 `cargo test --workspace` 同时覆盖。重生成：`UPDATE_SCHEMAS=1 cargo test -p scene-core-protocol --test schema_contract`。
- 已生成：control-message、engine-event、normalized-media、artifact-manifest、temporal-segment、input-set-descriptor、derivation-descriptor，以及共享的 engine-identity、toolchain-identity。
- 验收 5 的 Schema 断言已覆盖：有符号 `stream.startTimeMs`、`container.startTimeMs` 仅 0/null、公共 `requestedTimeMs` 非负。

未完成：
- version、doctor、package manifest 的 Schema 需要 SC-P0-04 冻结对应 DTO；toolchain descriptor、capabilities 需要 SC-P0-05 确定产物结构；生成器支持一行注册新类型。
- fixture（≥10 valid / ≥20 invalid / ≥6 golden）、`fixture-manifest.json` 与 fixture runner 待交付。

## 依赖

SC-P0-02。
