# 实现记录：任务完成系统通知

> 状态：已实现并通过自动化验证（2026-07-26）

## 改动清单

| 文件 | 改动 |
|------|------|
| `src-tauri/Cargo.toml` | 新增 `tauri-plugin-notification = "2"`（因本机 rustc 1.85，锁 `notify-rust` 传递依赖为 4.11.7） |
| `src-tauri/src/lib.rs` | 注册 `tauri_plugin_notification::init()` |
| `src-tauri/capabilities/default.json` | 新增 `notification:default` 权限 |
| `src-tauri/src/config/mod.rs` | `AppConfig.notify_on_job_finish`（`#[serde(default = "default_true")]`）；`Default`、`public_view`、`candidate_with_update`、测试 helper 同步 |
| `src-tauri/src/models/job.rs` | `SaveConfigRequest.notify_on_job_finish: Option<bool>` |
| `src-tauri/src/distribution/mod.rs` | 配置导出包含该开关；旧导出包缺字段默认 `true` |
| `src-tauri/src/pipeline/runner.rs` | `notify_job_finished`：终态（Succeeded/Failed）读取最新配置 → 开关关闭/窗口聚焦时抑制 → 标题截断 60 字符、失败正文含 `error_code` + 120 字符错误摘要；挂在 `start_queued_work` 线程结束处与 `pump_queue` 启动失败分支 |
| `src/types.ts` | `AppConfigPublic` / `SaveConfigRequest` 镜像新字段 |
| `src/App.tsx` | 设置页「常规」新增开关（默认开），load/save 链路接通 |
| `docs/PRODUCT_SPEC.md` | 决策日志新增第 21 条 |

顺带修复（clippy `-D warnings` 拦截的 v0.2.5 遗留问题，非本任务引入）：

- `douyin.rs:123` `unnecessary_lazy_evaluations` → `then_some`
- `douyin.rs:378` 可省略的显式生命周期
- `douyin.rs` / `record.rs` rustfmt 格式偏差

## 验证

- `cargo +stable fmt --all -- --check` 通过
- `cargo +stable clippy --all-targets -- -D warnings` 通过
- `cargo +stable test`：88 passed / 0 failed / 1 ignored
- `pnpm typecheck` + `pnpm build` 通过

## 未验证（需真机）

- Windows 通知实际弹出、前台抑制行为、旧 `config.json` 加载——需 `pnpm tauri:dev` 冒烟（自动化不覆盖 Tauri runtime）。
- R5 点击通知置前窗口未实现（插件默认点击行为；PRD 允许降级，记为 known 限制）。
