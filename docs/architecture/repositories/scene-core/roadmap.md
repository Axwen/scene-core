# `scene-core` 实施路线

## M0：仓库与协议骨架

- 创建 Rust workspace；
- 固定 MSRV、构建目标和版本策略；
- 实现 JSON/JSONL Envelope 的解析、校验和事件序号；
- 建立无真实用户媒体的合成 fixture；
- 输出协议 Schema、CLI help 和错误码表。

验收：协议测试能覆盖非法请求、事件乱序/重复和受控错误；不需要连接数据库或 Provider。

## M1：媒体探测与基础 Artifact

- probe 容器、音视频流和基础元数据；
- 固定 `extract_preview` 诊断预览（opening，以及适用时的 midpoint）；
- 明确素材播放时间轴与原始媒体时间戳的转换，保留音视频相对偏移；
- Hash/Fingerprint 和 Manifest；
- 取消、超时、进度和资源统计；
- 隔离未完成 Artifact，由 Host 管理 staging 清理。

首版不实现音频提取、分析用采样或 Scene/Shot 检测。诊断预览不承诺覆盖人物出镜或满足 OCR 图像质量。

验收：真实脱敏/公开测试媒体覆盖无音轨、无视频轨、损坏媒体、可变帧率和大文件边界；结果通过 `sourceVersionId` 和 request/run 关联。

## M2：可靠性和资源基线

- 进程崩溃、磁盘不足、超时和取消恢复；
- CPU/内存/磁盘基准；
- 受控错误和重试分类；
- 公共 sidecar 协议与 Manifest 一致性；内部 crate 不承诺公共 library API；
- Web/Desktop 调用方兼容矩阵。

验收：异常不会伪报完成，部分产物不会污染索引；建立 P50/P95 和资源报告。

## 消费者驱动的媒体扩展（M1 后独立立项）

- 音频提取由 Core 实现；Seek/上层分析模块负责 ASR 调用、转录结果和人物线索审核。
- 分析抽帧由 Core 实现为独立操作，不扩展固定诊断预览的含义。
- 两项能力不捆绑发布：首个真实消费者明确输入、输出和资源预算后，各自编写 Spec、Ticket 与验收 fixture。
- 音频规格必须明确音轨选择、裁剪、输出格式、重采样与原素材时间映射；抽帧规格必须明确采样方式、实际帧时间、数量及体积上限。
- 复用现有派生键、取消/超时和完整产物发布规则，不新增任务数据库或分析服务。
- 已被消费者固定的协议通过新版本扩展；未发布候选协议可修订，不预定扩展一定使用 `0.2`。

验收：音视频偏移、裁剪后 ASR 时间回映、VFR、无音轨、采样遗漏及取消/磁盘不足均有证据。音频后置时，ASR 自动线索流程也后置；Seek 人工人物标注可独立推进。

## M3：分层切分（后续）

- shot/scene segmentation；
- segmentationRef/shotPolicyRef 版本化；
- 时间边界和相邻关系；
- 与 Evidence Asset → Scene/Shot → Evidence 的映射 fixture。

验收：需要真实媒体黄金集、人工时间标注、Temporal IoU 和资源基线；没有这些证据不冻结算法参数。

## M4：可选服务化（不预设）

只有 Web 和桌面两个稳定消费者出现真实吞吐/隔离问题后，才评估 Worker Pool 或独立服务。服务化只能替换 Adapter 和部署，不改变引擎协议和 Manifest 语义。
