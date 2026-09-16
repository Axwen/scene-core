# SC-P0-02：实现 Protocol 0.1 DTO、输入身份、Scene/Shot seam 与 derivation key

## 目的

把控制流、身份语义和未来时间分段的公共边界固化为严格 Rust DTO 与纯校验函数，避免 `inputFingerprint` 继续兼任文件 hash 或缓存键。

## 范围

- 实现 Start/Cancel DTO、事件 Envelope、错误、Manifest 和 operation union；拒绝未知字段、重复 key、非法枚举和越界值。Host-facing StartRequest 的 `executionContext` 必填。
- 实现 `InputSetDescriptor`：Protocol `0.1` 只允许一个 `source_media`，按 role canonicalize；`inputFingerprint = sha256(RFC8785(...))`。
- 实现公共 `EngineIdentity`、`ToolchainIdentity`、`DerivationDescriptor` DTO；StartRequest、completed result、Manifest、`version` 和 `doctor` 复用同一结构。`derivationKey` 纳入 input fingerprint、operation、effective config hash、output contract、engine cache compatibility id、toolchain fingerprint；排除 request/run/source/execution identity。
- 实现 `TemporalSegment` DTO 与验证：`[startMs,endMs)`、稳定排序、ordinal、父子覆盖、无环、policy ref 和 provenance。
- 保持 Core 无状态；只计算并回传 key material，不读写缓存数据库。
- 按[已确认时间规则](../specs/media-time-policy-draft.md)实现归一化媒体 DTO 与纯时间转换：公共请求非负，轨道起点有符号，未知为 null；有理数换算、明确舍入及检查溢出，不引入 FFmpeg 执行。

## 验收标准

1. 改变输入字节 hash/size/role 会改变 input fingerprint；改变 requestId、sourceVersionId 或 attempt 不会改变它。
2. 改变 operation/config/output contract/toolchain 会改变 derivation key；同一公共 descriptor 在 Host/Core 产生相同 key。
3. 实际 staging 文件与声明 size/hash 不一致返回 `INPUT_CHANGED`；摘要不一致返回 `INVALID_REQUEST`。
4. 非法时间区间、环形父子关系、重复 role、绝对路径和不安全 ref 均被拒绝。
5. `probe`/`extract_preview` 不生成 Scene/Shot；后续 segmentation 只能通过新 operation/version 接入。
6. `container.startTimeMs` 仅为 0 或 null；`stream.startTimeMs` 接受负值。负公共请求被拒绝，音视频偏移保留，未知原点不默认零。
7. `tests/media_time_contract.rs` 验证正负半毫秒舍入、区间向外取整、精确值先判界再舍入、溢出和未知值；实际工具时间提取及播放器对齐留在 Phase 1/消费者验收。

## 实现状态（2026-09-16）

已在 `crates/scene-core-protocol` 落地，并通过 `cargo fmt --check`、`cargo check --workspace --locked`、`cargo test --workspace`（83 个用例，含 `tests/media_time_contract.rs`）和 `cargo clippy --workspace --all-targets -- -D warnings`（无 lint allow）：

- Start/Cancel、事件 Envelope、错误、Manifest 与 operation union 的严格 DTO；拒绝未知字段、重复 key、非法枚举和越界值；Host-facing `StartRequest.executionContext` 必填。
- 调用方 `options` 为冻结的空对象类型：任意键、重复键或数组形态在解析阶段被拒绝，不进入自由键值 map。
- `InputSetDescriptor` 的 0.1 单 `source_media` 规则、按 role canonicalize、RFC 8785 canonical JSON + SHA-256 `inputFingerprint`，以及 staged size/hash 复核（`INPUT_CHANGED` / `INPUT_NOT_FOUND`）和摘要不一致（`INVALID_REQUEST`）。
- 公共 `EngineIdentity`、`ToolchainIdentity`、`DerivationDescriptor`、`DerivationIdentity` 与 Host/Core 可独立复算的 `derivationKey`；契约版本与描述符版本使用类型化取值而非裸字符串。
- `ProtocolError` 在 `Result` 中装箱（`ProtocolResult`），`ControlMessage`/`OperationResult` 的大变体同样装箱，不依赖 clippy allow。
- `TemporalSegment` 的排序、ordinal、父子覆盖和无环校验，wire kind 为 `shot`/`sceneCandidate`；`probe`/`extract_preview` 结果不含 Scene/Shot。
- `EventStreamValidator` 纯状态机：accepted 起于 sequence 1、严格连续（拒绝重复/回退/跳号）、恰一个终态、终态后无事件、请求身份原样回传；终态退出码映射见 `EventEnvelope::exit_code`。SC-P0-03 只保留 golden transcript 与 fixture runner。
- 归一化媒体 DTO 与已确认时间规则的纯转换：有符号轨道起点、唯一容器原点、半毫秒远离零舍入、区间向下/向上取整、先判界再舍入、未知为 null、溢出受控。
- 新增 workspace-private 依赖 `serde`、`serde_json`、`sha2`；`Cargo.lock` 已更新。

未完成：`schemas/0.1/`、正反/golden fixture 与 transcript fixture runner 属 SC-P0-03；`version`/`doctor` 输出结构属 SC-P0-04；真实工具时间提取和播放器对齐属 Phase 1/消费者验收。

## 依赖

SC-P0-01。
