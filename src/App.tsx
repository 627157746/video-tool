import { useCallback, useEffect, useMemo, useState } from "react";
import {
  createDownloadJob,
  createImportJob,
  createLiveRecordJob,
  getAppInfo,
  getConfig,
  listJobs,
  openJobDirectory,
  probeSidecars,
} from "./api";
import type {
  AppConfigPublic,
  AppInfo,
  JobListItem,
  JobKind,
  SidecarStatus,
} from "./types";
import "./App.css";

type CreateMode = "download" | "live" | "import" | null;
type MainView = "jobs" | "settings";

const KIND_LABEL: Record<JobKind, string> = {
  download: "下载",
  live_record: "直播录制",
  import_local: "本地导入",
};

const STATUS_LABEL: Record<string, string> = {
  pending: "等待中",
  running: "运行中",
  succeeded: "成功",
  failed: "失败",
  cancelled: "已取消",
};

function formatTime(value: string): string {
  try {
    return new Date(value).toLocaleString();
  } catch {
    return value;
  }
}

function App() {
  const [view, setView] = useState<MainView>("jobs");
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [config, setConfig] = useState<AppConfigPublic | null>(null);
  const [jobs, setJobs] = useState<JobListItem[]>([]);
  const [sidecars, setSidecars] = useState<SidecarStatus | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [createMode, setCreateMode] = useState<CreateMode>(null);

  const [formUrl, setFormUrl] = useState("");
  const [formTitle, setFormTitle] = useState("");
  const [formLocalPath, setFormLocalPath] = useState("");
  const [formSegmentMinutes, setFormSegmentMinutes] = useState(30);
  const [autoTranscribe, setAutoTranscribe] = useState(true);
  const [autoSummarize, setAutoSummarize] = useState(false);

  const refresh = useCallback(async () => {
    setErrorMessage(null);
    try {
      const [nextInfo, nextConfig, nextJobs, nextSidecars] = await Promise.all([
        getAppInfo(),
        getConfig(),
        listJobs(),
        probeSidecars(),
      ]);
      setAppInfo(nextInfo);
      setConfig(nextConfig);
      setJobs(nextJobs);
      setSidecars(nextSidecars);
      setFormSegmentMinutes(nextConfig.default_segment_minutes);
      setAutoTranscribe(nextConfig.default_auto_transcribe);
      setAutoSummarize(nextConfig.default_auto_summarize);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const filteredJobs = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    if (!query) {
      return jobs;
    }
    return jobs.filter((job) => {
      const haystack = [
        job.id,
        job.title,
        job.status,
        job.kind,
        job.error_message ?? "",
      ]
        .join(" ")
        .toLowerCase();
      return haystack.includes(query);
    });
  }, [jobs, searchQuery]);

  function resetCreateForm() {
    setFormUrl("");
    setFormTitle("");
    setFormLocalPath("");
    setFormSegmentMinutes(config?.default_segment_minutes ?? 30);
    setAutoTranscribe(config?.default_auto_transcribe ?? true);
    setAutoSummarize(config?.default_auto_summarize ?? false);
  }

  function openCreate(mode: CreateMode) {
    resetCreateForm();
    setCreateMode(mode);
    setStatusMessage(null);
    setErrorMessage(null);
  }

  async function submitCreate() {
    if (!createMode) {
      return;
    }

    setErrorMessage(null);
    setStatusMessage(null);

    const pipeline = {
      auto_transcribe: autoTranscribe,
      auto_summarize: autoSummarize,
      provider_profile_id: config?.default_provider_profile_id ?? null,
      template_id: config?.default_template_id ?? null,
    };

    try {
      if (createMode === "download") {
        await createDownloadJob({
          url: formUrl,
          title: formTitle || undefined,
          pipeline,
        });
      } else if (createMode === "live") {
        await createLiveRecordJob({
          url: formUrl,
          title: formTitle || undefined,
          segment_minutes: formSegmentMinutes,
          pipeline,
        });
      } else {
        await createImportJob({
          local_path: formLocalPath,
          title: formTitle || undefined,
          pipeline,
        });
      }

      setCreateMode(null);
      setStatusMessage("任务已创建（骨架阶段：仅落盘 Job 目录，尚未真正下载/转写）");
      await refresh();
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleOpenDirectory(jobId: string) {
    try {
      const path = await openJobDirectory(jobId);
      setStatusMessage(`任务目录：${path}`);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand">
          <div className="brand-mark">VT</div>
          <div>
            <div className="brand-title">{appInfo?.name ?? "video-tool"}</div>
            <div className="brand-subtitle">
              {appInfo?.description ?? "任务中心骨架"} · v
              {appInfo?.version ?? "0.1.0"}
            </div>
          </div>
        </div>

        <nav className="nav">
          <button
            className={view === "jobs" ? "nav-btn active" : "nav-btn"}
            onClick={() => setView("jobs")}
            type="button"
          >
            任务中心
          </button>
          <button
            className={view === "settings" ? "nav-btn active" : "nav-btn"}
            onClick={() => setView("settings")}
            type="button"
          >
            设置
          </button>
        </nav>

        <div className="top-actions">
          <button className="btn secondary" onClick={() => void refresh()} type="button">
            刷新
          </button>
          <button className="btn" onClick={() => openCreate("download")} type="button">
            新建下载
          </button>
          <button className="btn" onClick={() => openCreate("live")} type="button">
            录制直播
          </button>
          <button className="btn" onClick={() => openCreate("import")} type="button">
            本地导入
          </button>
        </div>
      </header>

      {(errorMessage || statusMessage) && (
        <div className="banner-row">
          {errorMessage && <div className="banner error">{errorMessage}</div>}
          {statusMessage && <div className="banner ok">{statusMessage}</div>}
        </div>
      )}

      <main className="content">
        {view === "jobs" ? (
          <section className="panel">
            <div className="panel-header">
              <div>
                <h1>任务中心</h1>
                <p className="muted">
                  统一 Job 列表。当前为初始化骨架：可创建任务并生成 `workspace/jobs/&lt;id&gt;/`。
                </p>
              </div>
              <input
                className="search"
                placeholder="搜索标题 / URL / 状态 / ID"
                value={searchQuery}
                onChange={(event) => setSearchQuery(event.target.value)}
              />
            </div>

            {isLoading ? (
              <div className="empty">加载中…</div>
            ) : filteredJobs.length === 0 ? (
              <div className="empty">
                暂无任务。用右上角三个入口创建第一个 Job。
              </div>
            ) : (
              <div className="table-wrap">
                <table>
                  <thead>
                    <tr>
                      <th>标题</th>
                      <th>类型</th>
                      <th>状态</th>
                      <th>当前步骤</th>
                      <th>创建时间</th>
                      <th>操作</th>
                    </tr>
                  </thead>
                  <tbody>
                    {filteredJobs.map((job) => (
                      <tr key={job.id}>
                        <td>
                          <div className="job-title">{job.title}</div>
                          <div className="mono muted">{job.id}</div>
                          {job.error_message && (
                            <div className="error-text">{job.error_message}</div>
                          )}
                        </td>
                        <td>{KIND_LABEL[job.kind]}</td>
                        <td>
                          <span className={`pill status-${job.status}`}>
                            {STATUS_LABEL[job.status] ?? job.status}
                          </span>
                        </td>
                        <td className="mono">{job.current_step ?? "-"}</td>
                        <td>{formatTime(job.created_at)}</td>
                        <td>
                          <button
                            className="btn secondary small"
                            type="button"
                            onClick={() => void handleOpenDirectory(job.id)}
                          >
                            目录路径
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>
        ) : (
          <section className="panel settings">
            <h1>设置（只读预览）</h1>
            <p className="muted">
              配置文件与工作区路径按规格分离；完整编辑 UI 后续迭代。
            </p>

            <div className="cards">
              <article className="card">
                <h2>工作区</h2>
                <dl>
                  <div>
                    <dt>workspace</dt>
                    <dd className="mono">{config?.workspace_dir ?? "-"}</dd>
                  </div>
                  <div>
                    <dt>config</dt>
                    <dd className="mono">{config?.config_path ?? "-"}</dd>
                  </div>
                  <div>
                    <dt>默认分段</dt>
                    <dd>{config?.default_segment_minutes ?? "-"} 分钟</dd>
                  </div>
                  <div>
                    <dt>默认流水线</dt>
                    <dd>
                      转写 {config?.default_auto_transcribe ? "开" : "关"} / 总结{" "}
                      {config?.default_auto_summarize ? "开" : "关"}
                    </dd>
                  </div>
                </dl>
              </article>

              <article className="card">
                <h2>Sidecar 探测</h2>
                <div className="sidecar-list">
                  {sidecars &&
                    Object.values(sidecars).map((binary) => (
                      <div key={binary.name} className="sidecar-row">
                        <div>
                          <strong>{binary.name}</strong>
                          <span className={`pill source-${binary.source}`}>
                            {binary.source}
                          </span>
                        </div>
                        <div className="mono muted">
                          {binary.path ?? "未找到"}
                        </div>
                        <div className="muted small">
                          {binary.version ?? "无版本信息"}
                        </div>
                      </div>
                    ))}
                </div>
              </article>

              <article className="card">
                <h2>Provider 档案</h2>
                <ul className="plain-list">
                  {config?.providers.map((provider) => (
                    <li key={provider.id}>
                      <strong>{provider.name}</strong>
                      <div className="muted small">
                        {provider.protocol} · {provider.default_model} · Key{" "}
                        {provider.has_api_key ? "已配置" : "未配置"}
                      </div>
                      <div className="mono small">{provider.base_url}</div>
                    </li>
                  ))}
                </ul>
              </article>

              <article className="card">
                <h2>总结模板</h2>
                <ul className="plain-list">
                  {config?.templates.map((template) => (
                    <li key={template.id}>
                      <strong>{template.name}</strong>
                      <div className="muted small">{template.id}</div>
                    </li>
                  ))}
                </ul>
              </article>
            </div>
          </section>
        )}
      </main>

      {createMode && (
        <div className="modal-backdrop" role="presentation">
          <div className="modal" role="dialog" aria-modal="true">
            <div className="modal-header">
              <h2>
                {createMode === "download" && "新建下载任务"}
                {createMode === "live" && "新建直播录制"}
                {createMode === "import" && "导入本地视频"}
              </h2>
              <button
                className="btn secondary small"
                type="button"
                onClick={() => setCreateMode(null)}
              >
                关闭
              </button>
            </div>

            <div className="form-grid">
              {(createMode === "download" || createMode === "live") && (
                <label>
                  <span>URL / 流地址</span>
                  <input
                    value={formUrl}
                    onChange={(event) => setFormUrl(event.target.value)}
                    placeholder="https://... 或 m3u8/flv 地址"
                  />
                </label>
              )}

              {createMode === "import" && (
                <label>
                  <span>本地文件路径</span>
                  <input
                    value={formLocalPath}
                    onChange={(event) => setFormLocalPath(event.target.value)}
                    placeholder="D:/videos/example.mp4"
                  />
                </label>
              )}

              <label>
                <span>标题（可选）</span>
                <input
                  value={formTitle}
                  onChange={(event) => setFormTitle(event.target.value)}
                  placeholder="便于列表识别"
                />
              </label>

              {createMode === "live" && (
                <label>
                  <span>分段时长（分钟）</span>
                  <input
                    type="number"
                    min={1}
                    value={formSegmentMinutes}
                    onChange={(event) =>
                      setFormSegmentMinutes(Number(event.target.value) || 30)
                    }
                  />
                </label>
              )}

              <div className="checkbox-row">
                <label className="checkbox">
                  <input
                    type="checkbox"
                    checked={autoTranscribe}
                    onChange={(event) => setAutoTranscribe(event.target.checked)}
                  />
                  完成后自动转写
                </label>
                <label className="checkbox">
                  <input
                    type="checkbox"
                    checked={autoSummarize}
                    onChange={(event) => setAutoSummarize(event.target.checked)}
                  />
                  转写后自动总结
                </label>
              </div>
            </div>

            <div className="modal-actions">
              <button
                className="btn secondary"
                type="button"
                onClick={() => setCreateMode(null)}
              >
                取消
              </button>
              <button className="btn" type="button" onClick={() => void submitCreate()}>
                创建任务
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
