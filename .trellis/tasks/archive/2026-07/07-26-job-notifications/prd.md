# PRD: 任务完成系统通知

> 父任务：`07-26-v03-usability`
> 状态：规划中（PRD 已定稿，待 design/implement）

## 目标

长耗时任务（下载/录制/转写/总结）结束时发送 Windows 系统通知，用户无需守着窗口等结果。

## 决策（2026-07-26 brainstorm）

- 触发范围：**仅 Job 整体终态**（成功一条 / 失败一条）；否决了逐步骤通知与仅失败通知。
- 打扰策略：主窗口前台聚焦时不发通知；设置中可整体开关，默认开。

## 代码库确认事实

- 无 `tauri-plugin-notification` 依赖（`src-tauri/Cargo.toml`）；capabilities 无通知权限。
- Rust 侧 Job 持久化时 emit `job-updated` 事件，前端已监听；Job 终态判定在 Rust 状态机内完成。
- 配置结构有 Serde 默认兼容惯例（旧配置缺字段不报错），新开关需遵循。

## 需求

- R1 依赖接入：新增 `tauri-plugin-notification`（Rust + 前端 npm 包按需）并在 capabilities 声明权限。
- R2 触发点：Rust 侧 Job 进入终态（完成 / 失败）持久化处发送：
  - 成功：标题 = 任务标题（截断），正文 = "已完成"（含最后一步名）；
  - 失败：正文含失败步骤与 `error_code` 对应的简短可读文案；
  - 不含 URL 之外的敏感信息，遵循脱敏纪律（不写 Key/Cookie）。
- R3 前台抑制：主窗口存在且聚焦时不发送（Rust 经 AppHandle 查询窗口 focus 状态）。
- R4 配置：`notify_on_job_finish: bool`（默认 `true`）；设置页通知开关；旧配置 Serde 默认兼容。
- R5 点击行为：若插件支持，点击通知将主窗口置前；不支持则接受仅提示（不阻塞 MVP）。

## 验收标准

- 窗口最小化/失焦时：Job 成功、失败各收到一条系统通知，文案含任务标题与结果。
- 窗口前台时不弹通知。
- 设置关闭开关后不再发送；重启后配置保持。
- 旧 `config.json`（无新字段）加载正常，默认为开。
- Rust fmt/clippy/test、`pnpm typecheck`/`pnpm build` 通过。

## 非目标

- 逐步骤通知、通知历史中心、声音自定义。
- 批量任务聚合通知（批量 URL 多 Job 完成会各发一条；known 噪音，后置聚合优化并在文档记录）。
- 托盘气泡与闪烁等替代提醒形式。

## 依赖与顺序

- 与其它三个子任务无依赖，可最先实施（体量最小，适合作为 v0.3 第一个交付）。
