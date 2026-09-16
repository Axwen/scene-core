# SC-P0-07：固化 Host/cache 契约并修订跨仓库文档

## 目的

明确 Core、Host、`scene-seek` 和 Web Worker 的责任，确保缓存复用不会跨权限、跨版本或跨当前结果错误写回。

## 范围

- 定义 Host 输入快照、`sourceVersionId`、content hash、input fingerprint、derivation key、共享 identity DTO 和 current-result gate 的顺序。
- 定义缓存键 `(cacheScope, derivationKey)`；Host 负责租约、single-flight、持久化、逐出、配额和权限域，Core 不持有缓存数据库。
- 只允许完整、成功、退出码 0、Manifest/Artifact 全校验通过的不可变集合进入缓存；失败、取消、超时、崩溃和 partial 不得命中。
- 缓存命中创建新的 Run/provenance，不复用旧 run identity；同步 `engine-contract.md`、`responsibilities.md`、`roadmap.md`、`implementation-plan.md` 和跨仓库契约。

## 验收标准

1. 文档明确 `inputFingerprint` 不是文件 hash，也不是缓存 key；`derivationKey` 不含 request/source/execution identity。
2. 文档包含 cacheScope 的租户/用户/权限隔离要求和 cache-hit 新 Run 规则。
3. 文档统一 `engineProtocolVersion: 0.1`、`engineCacheCompatibilityId`、终态 Manifest 和不可发布的 staged/incomplete 产物，并移除 Core 实现 Tauri/AppContainer 的表述。
4. `scene-seek` 现有按 input fingerprint 复用的逻辑被标记为迁移输入，不成为 Core 兼容约束。

## 实现状态（2026-09-16）

- 新增 `docs/specs/host-cache-contract.md`：职责边界表、身份与顺序（快照 → contentHash → inputFingerprint → derivationKey → StartRequest → Core 重算 → 校验/finalize → current-result gate）、缓存键 `(cacheScope, derivationKey)`、可缓存性与命中规则、并发与持久化归属、scene-seek 字幕复用迁移说明。
- `cross-repository-contracts.md` 同步：新增 §3.3 缓存契约；§4.2 请求/事件字段改为 0.1 精确字段（engine identity 由 `version --json` 获取，不在请求里虚构）；§4.3 Manifest 改为 0.1 实际字段并明确 staged/失败不可发布；§7.1 版本固定增加 `engineCacheCompatibilityId` 与 `toolchainFingerprint`；§7.3 增加缓存负向与命中用例。
- `responsibilities.md`、`engine-contract.md`、主 Spec §13、`implementation-plan.md` 增加缓存归属与链接；架构文档无“Core 实现 Tauri/AppContainer”表述。
- 验收对照：`inputFingerprint` 非文件 hash 且非缓存键、`derivationKey` 排除 request/source/execution identity、cacheScope 权限隔离、命中创建新 Run、终态 Manifest 与 staged 不可发布、scene-seek 旧复用标记为迁移输入，均已写入文档。

## 依赖

SC-P0-02、SC-P0-03、SC-P0-05。
