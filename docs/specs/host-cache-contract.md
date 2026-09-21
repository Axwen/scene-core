# Host 集成与缓存契约（候选 Protocol 0.1）

状态：SC-P0-07 固化，适用于未发布的候选 Protocol 0.1；消费者固定前可修订。字段事实源是 Rust DTO 与 [engine-contract.md](../architecture/repositories/scene-core/engine-contract.md)，本文不定义第二套字段。

## 1. 职责边界

| 职责 | Core | Host（`scene-seek` / Web Worker） | 说明 |
|---|---|---|---|
| 输入快照与 staging | 不负责 | 创建每请求私有、初始为空的 staging，放入 `input/source.media` | Core 只读取 staging 内的相对引用 |
| 身份计算 | 重算并回传 `inputFingerprint`、`derivationKey` | 计算每文件 `contentHash`/`byteSize`、`inputFingerprint`、`derivationKey` | 两侧独立计算，不一致即拒绝 |
| 缓存数据库 | 不连接、不持有 | 持有 `(cacheScope, derivationKey)` 索引、租约、single-flight、逐出与配额 | Core 保持无状态 |
| 产物发布 | 生成 Manifest 与文件到 staging | 校验后原子 finalize、注册、映射为 ProviderArtifact/Evidence | 失败/取消/超时/partial 不得发布 |
| 权限与生命周期 | 不接收业务权限字段 | 决定可见性、删除、过期、Legal Hold | cacheScope 是权限隔离边界 |

## 2. 身份与顺序

```text
1. Host 创建每请求私有且初始为空的 staging root
2. 快照不可变输入到 input/source.media，记录 byteSize 与 contentHash
3. inputFingerprint = sha256(RFC8785(InputSetDescriptor))
4. 从 `scene-core version --json` 取得 engine identity 与 toolchainFingerprint
   derivationKey = sha256(RFC8785(DerivationDescriptor))
5. 发送 StartRequest（executionContext 必填，deadline 可选）
6. Core 重算两个摘要：不一致 → INVALID_REQUEST
   staging 实际 size/hash 与声明不符 → INPUT_CHANGED
7. completed 后 Host 校验协议版本、身份回传、sequence/终态配对、Manifest、
   relativeRef、regular file、size 与 hash，然后原子 finalize
8. 通过后才进入 current-result gate 与缓存/索引
9. 失败、取消、超时、崩溃或未 finalize 的 staged 产物由 Host 隔离清理
```

`inputFingerprint` 是逻辑输入集合的摘要，**不是**单文件 hash，也**不是**缓存键；`sourceVersionId` 与 request/run identity 不进入摘要。

## 3. 缓存键与作用域

- 持久缓存键固定为 `(cacheScope, derivationKey)`。
- `cacheScope` 至少隔离租户/用户、权限域与存储域；它**绝不进入 Core 协议**，也不由 Core 推断。
- `derivationKey` 覆盖：`inputFingerprint`、`operation`、补齐默认值后的 `operationConfigHash`、`outputContractVersion`、`engineCacheCompatibilityId`、`toolchainFingerprint`。
- `derivationKey` 排除：`requestId`、`sourceVersionId`、`executionContext`（run/generation/attempt）、时间戳、staging 路径与机器身份。

同一内容可以对应不同 `sourceVersionId`：这不改变 `inputFingerprint`/`derivationKey`，但 current-result gate 仍必须匹配当前 source version。

## 4. 可缓存性与缓存命中

只有同时满足以下条件的完整 Artifact 集可以注册或缓存：

- 终态 `completed` 且进程退出码 0；
- 事件身份回传、sequence 连续、恰一个终态；
- Manifest 通过 Schema 校验，相对引用为 regular file，size 与 hash 全部匹配；
- 已由 Host 原子 finalize。

以下结果**永不**成为命中源：`failed`、`cancelled`、`timed_out`、崩溃、被强杀、无终态、terminal/exit 不一致、staged/partial 产物、哈希校验失败的集合。

缓存命中时：

1. 以 `(cacheScope, derivationKey)` 找到完整不可变 Artifact 集；
2. 校验该条目仍属于当前 cacheScope/权限域；
3. 创建**新的 Run/provenance**，关联原缓存条目的 derivation identity 与 Artifact；
4. 重新执行 current-result gate；
5. 不复用旧 `requestId`、run/generation/attempt，不跳过任何校验。

## 5. current-result gate

在把结果写回当前 Evidence/索引/Release 前，必须逐项匹配：

```text
sourceVersionId   == 当前 source version
runId             == 当前 accepted run
inputFingerprint  == 当前输入
generation        == 当前 generation
attempt           == 当前允许的 attempt
run status        ∈ {queued, running, retrying}（写回门槛）
```

旧 generation、`cancelled`、`superseded`、过期或指纹不一致的结果可以保留为历史 Artifact/事件，但不得改变当前 Evidence 可用性、索引或 Release。

## 6. 并发与持久化归属

租约、并发去重（single-flight）、缓存索引、保留、逐出、磁盘配额、加密与备份全部属于 Host；Core 只提供 descriptor/key 计算、Schema 与 conformance fixture，不写任何数据库。

### 6.1 Host 并发准入与配额（P2 边界）

本节定义 Host 允许多个 Core 请求并发前的验收边界。Protocol 0.1 的 Core 仍是**单请求进程**：一个 `run` 会话只服务一个 StartRequest，不实现进程内并发、线程池或跨请求共享状态。Host 侧的并发、排队和配额尚未实现，本节是其实施规格，不是已交付能力。

```text
Host admission
  -> (cacheScope, derivationKey) single-flight：同键活动请求合并为一个 leader
  -> 全局/每 cacheScope 活动请求上限；超出 -> admission reject（不启动 Core）
  -> 内存预留 + 每请求 staging 磁盘配额检查
  -> 启动 Core（单请求进程，私有 staging）
  -> 完成/失败后释放预留、清理 staging、结束租约
```

- **最大活动请求数**：Host 维护全局上限与每 `cacheScope` 上限；两者任一饱和即拒绝新 admission。数值由 Host 按部署配置，不在 Core 协议中传递。
- **内存配额**：按活动请求数预留峰值内存预算（参考单请求实测峰值 152 MiB），预留失败即 admission reject；Core 不读取宿主机内存，只受自身输出上限约束。
- **磁盘配额**：每个 staging 在启动 Core 前校验 `input + 输出上限 + 余量` 是否落在剩余配额内；预留为 staging 生命周期内的记账，失败按 `admission reject` 处理，不启动 Core。
- **single-flight**：同一 `(cacheScope, derivationKey)` 只允许一个 leader 执行；跟随者等待同一完成事件（成功则按 §4 缓存命中路径注册新 Run，失败/取消/超时则各自得到不可缓存终态），不得并发跑同一派生工作。
- **admission reject 语义**：拒绝是可报告的 Host 状态（队列满/配额不足），不产生 Core `failed` 事件、不创建 Run、不写入缓存，且不得以重试掩盖为部分成功。
- **失败清理与不可缓存**：§4 的不可缓存集合在并发下不变；leader 失败必须通知并释放全部等待者与预留，跟随者不得复用 leader 的 staging 或 partial 产物。
- **顺序性**：admission → 预留 → 启动 → 完成校验 → finalize → 释放；任何一步失败都回滚该请求全部 Host 侧状态（staging、租约、预留），不留下孤儿进程或半初始化索引项。

Core 侧不新增并发实现；当 Host 需要多请求吞吐时，由 Host 决定进程池或队列策略，Core 的单请求契约和退出码矩阵保持不变。

## 7. 迁移说明

`scene-seek` 现阶段按 `sourceVersionId`/provider version/input fingerprint 查找成功 Run 来复用字幕。这是迁移输入，不是 Core `derivationKey` 的兼容约束：它没有覆盖 operation config、output contract 与 toolchain。迁移后旧索引可以保留为历史，但不得作为 `(cacheScope, derivationKey)` 缓存命中源，也不得跳过 current-result gate。

## 8. 关联文档

- [Protocol 0.1 主 Spec](scene-core-phase0-protocol-0.1.md) §3、§9、§13
- [引擎契约](../architecture/repositories/scene-core/engine-contract.md) §7、§9
- [跨仓库契约](../architecture/repositories/cross-repository-contracts.md) §3.2、§7
