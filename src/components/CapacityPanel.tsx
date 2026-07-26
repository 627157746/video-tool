import { useCallback, useEffect, useState } from "react";
import { getWorkspaceUsage, purgeJobMedia } from "../api";
import type { WorkspaceUsageReport } from "../types";

export function formatBytes(sizeInBytes: number): string {
  if (sizeInBytes >= 1024 * 1024 * 1024) {
    return `${(sizeInBytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }
  if (sizeInBytes >= 1024 * 1024) {
    return `${(sizeInBytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  if (sizeInBytes >= 1024) {
    return `${(sizeInBytes / 1024).toFixed(1)} KB`;
  }
  return `${sizeInBytes} B`;
}

interface CapacityPanelProps {
  /** Jump to job detail in the jobs view. */
  onOpenJob: (jobId: string) => void;
  /** Refresh job list / detail after a purge changed job state. */
  onJobsChanged: () => void;
  onError: (message: string) => void;
  onStatus: (message: string) => void;
}

/**
 * Settings "capacity" section: workspace disk usage overview plus manual
 * per-job media purge (text assets are always kept). v0.3.
 */
export function CapacityPanel({
  onOpenJob,
  onJobsChanged,
  onError,
  onStatus,
}: CapacityPanelProps) {
  const [usageReport, setUsageReport] = useState<WorkspaceUsageReport | null>(
    null,
  );
  const [isLoadingUsage, setIsLoadingUsage] = useState(false);
  const [purgingJobId, setPurgingJobId] = useState<string | null>(null);

  const refreshUsage = useCallback(async () => {
    setIsLoadingUsage(true);
    try {
      setUsageReport(await getWorkspaceUsage());
    } catch (error) {
      onError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsLoadingUsage(false);
    }
  }, [onError]);

  useEffect(() => {
    void refreshUsage();
  }, [refreshUsage]);

  const handlePurge = useCallback(
    async (jobId: string, jobTitle: string) => {
      const confirmed = window.confirm(
        `确定清理任务「${jobTitle}」的全部媒体文件吗？\n\n` +
          "转写文本、总结与日志会保留，但媒体删除后不可恢复：\n" +
          "· 下载任务可重跑「获取媒体」重新下载\n" +
          "· 直播录制的媒体无法重新获得",
      );
      if (!confirmed) {
        return;
      }
      setPurgingJobId(jobId);
      try {
        await purgeJobMedia(jobId);
        onStatus(`已清理任务媒体：${jobTitle}`);
        onJobsChanged();
        await refreshUsage();
      } catch (error) {
        onError(error instanceof Error ? error.message : String(error));
      } finally {
        setPurgingJobId(null);
      }
    },
    [onError, onJobsChanged, onStatus, refreshUsage],
  );

  return (
    <article className="card soft">
      <div className="card-title-row">
        <h3>工作区占用</h3>
        <button
          type="button"
          className="btn secondary"
          disabled={isLoadingUsage}
          onClick={() => void refreshUsage()}
        >
          {isLoadingUsage ? "统计中…" : "重新统计"}
        </button>
      </div>

      {usageReport == null ? (
        <p className="muted small">
          {isLoadingUsage ? "正在统计磁盘占用…" : "尚未统计"}
        </p>
      ) : (
        <>
          <dl className="meta-list">
            <div>
              <dt>工作区目录</dt>
              <dd className="mono">{usageReport.workspace_dir}</dd>
            </div>
            <div>
              <dt>总占用</dt>
              <dd>
                {formatBytes(usageReport.total_bytes)}（其中媒体{" "}
                {formatBytes(usageReport.total_media_bytes)}）
              </dd>
            </div>
            <div>
              <dt>磁盘剩余</dt>
              <dd>
                {usageReport.free_disk_gb != null
                  ? `${usageReport.free_disk_gb} GB`
                  : "未知"}
              </dd>
            </div>
          </dl>

          {usageReport.jobs.length === 0 ? (
            <p className="muted small">暂无任务。</p>
          ) : (
            <ul className="capacity-job-list">
              {usageReport.jobs.map((jobUsage) => (
                <li key={jobUsage.job_id} className="capacity-job-row">
                  <div className="capacity-job-main">
                    <button
                      type="button"
                      className="link-btn capacity-job-title"
                      onClick={() => onOpenJob(jobUsage.job_id)}
                      title="打开任务详情"
                    >
                      {jobUsage.title}
                    </button>
                    <span className="muted small">
                      媒体 {formatBytes(jobUsage.media_bytes)} · 文字资产{" "}
                      {formatBytes(jobUsage.text_bytes)}
                      {jobUsage.media_purged ? " · 媒体已清理" : ""}
                    </span>
                  </div>
                  <button
                    type="button"
                    className="btn danger"
                    disabled={
                      purgingJobId != null ||
                      jobUsage.media_purged ||
                      jobUsage.status === "running" ||
                      jobUsage.status === "queued" ||
                      jobUsage.media_bytes === 0
                    }
                    title={
                      jobUsage.status === "running" ||
                      jobUsage.status === "queued"
                        ? "任务运行/排队期间不能清理"
                        : jobUsage.media_purged
                          ? "媒体已清理"
                          : jobUsage.media_bytes === 0
                            ? "没有可清理的媒体"
                            : "删除 media/ 下全部文件，保留文字资产"
                    }
                    onClick={() =>
                      void handlePurge(jobUsage.job_id, jobUsage.title)
                    }
                  >
                    {purgingJobId === jobUsage.job_id ? "清理中…" : "清理媒体"}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </article>
  );
}
