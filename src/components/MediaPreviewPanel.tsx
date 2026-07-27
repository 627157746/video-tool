import { convertFileSrc } from "@tauri-apps/api/core";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  generateMediaPreview,
  getJobMediaOverview,
  getTranscriptCues,
} from "../api";
import type {
  JobMediaFile,
  JobMediaOverview,
  TranscriptCue,
} from "../types";
import { formatBytes } from "./CapacityPanel";

function formatCueTimestamp(milliseconds: number): string {
  const totalSeconds = Math.floor(milliseconds / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const paddedMinutes = String(minutes).padStart(2, "0");
  const paddedSeconds = String(seconds).padStart(2, "0");
  return hours > 0
    ? `${hours}:${paddedMinutes}:${paddedSeconds}`
    : `${paddedMinutes}:${paddedSeconds}`;
}

const MEDIA_KIND_LABEL: Record<string, string> = {
  preview: "预览副本",
  merged: "合并产物",
  original: "原始媒体",
  segment: "分段",
  other: "其他",
};

interface MediaPreviewPanelProps {
  jobId: string;
  /** Disable preview generation while the job is running / queued. */
  isJobBusy: boolean;
  onError: (message: string) => void;
  onStatus: (message: string) => void;
}

/**
 * Job detail "preview" section (v0.3): play media artifacts in-app via the
 * asset protocol, with subtitle-list sync (click cue → seek; playback
 * highlights the current cue). Incompatible containers can be remuxed into
 * `media/preview.mp4` (ffmpeg -c copy).
 */
export function MediaPreviewPanel({
  jobId,
  isJobBusy,
  onError,
  onStatus,
}: MediaPreviewPanelProps) {
  const [mediaOverview, setMediaOverview] = useState<JobMediaOverview | null>(
    null,
  );
  const [subtitleCues, setSubtitleCues] = useState<TranscriptCue[]>([]);
  const [selectedFileName, setSelectedFileName] = useState<string | null>(null);
  const [isGeneratingPreview, setIsGeneratingPreview] = useState(false);
  const [activeCueIndex, setActiveCueIndex] = useState<number | null>(null);
  const [playbackError, setPlaybackError] = useState<string | null>(null);
  const mediaElementRef = useRef<HTMLVideoElement | HTMLAudioElement | null>(
    null,
  );
  const cueListRef = useRef<HTMLUListElement | null>(null);
  const loadRequestVersionRef = useRef(0);

  const loadOverview = useCallback(async () => {
    const requestVersion = ++loadRequestVersionRef.current;
    try {
      const [overview, cueDocument] = await Promise.all([
        getJobMediaOverview(jobId),
        getTranscriptCues(jobId).catch(() => null),
      ]);
      if (loadRequestVersionRef.current !== requestVersion) {
        return;
      }
      setMediaOverview(overview);
      setSubtitleCues(cueDocument?.has_srt ? cueDocument.cues : []);
      setActiveCueIndex(null);
      setPlaybackError(null);
      setSelectedFileName((previousSelection) => {
        if (
          previousSelection != null &&
          overview.files.some((file) => file.file_name === previousSelection)
        ) {
          return previousSelection;
        }
        const preferredFile =
          overview.files.find((file) => file.playability === "direct") ??
          overview.files.find((file) => file.playability === "maybe") ??
          overview.files[0];
        return preferredFile?.file_name ?? null;
      });
    } catch (error) {
      if (loadRequestVersionRef.current === requestVersion) {
        const message =
          error instanceof Error ? error.message : String(error);
        // Running downloads rewrite source.json frequently; a single transient
        // miss should not surface as a hard "任务不存在" banner.
        if (message.includes("任务不存在") && isJobBusy) {
          return;
        }
        onError(message);
      }
    }
  }, [isJobBusy, jobId, onError]);

  useEffect(() => {
    void loadOverview();
  }, [loadOverview]);

  const selectedFile: JobMediaFile | null = useMemo(
    () =>
      mediaOverview?.files.find(
        (file) => file.file_name === selectedFileName,
      ) ?? null,
    [mediaOverview, selectedFileName],
  );

  const mediaSourceUrl = useMemo(
    () => (selectedFile ? convertFileSrc(selectedFile.absolute_path) : null),
    [selectedFile],
  );

  const handleGeneratePreview = useCallback(async () => {
    setIsGeneratingPreview(true);
    try {
      const previewFileName = await generateMediaPreview(jobId);
      onStatus("预览副本已生成（media/preview.mp4）");
      await loadOverview();
      setSelectedFileName(previewFileName);
    } catch (error) {
      onError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsGeneratingPreview(false);
    }
  }, [jobId, loadOverview, onError, onStatus]);

  const handleTimeUpdate = useCallback(() => {
    const mediaElement = mediaElementRef.current;
    if (mediaElement == null || subtitleCues.length === 0) {
      return;
    }
    const currentMs = mediaElement.currentTime * 1000;
    // Cues are time-ordered; linear scan is fine for typical cue counts.
    let nextActiveIndex: number | null = null;
    for (const cue of subtitleCues) {
      if (currentMs >= cue.start_ms && currentMs < cue.end_ms) {
        nextActiveIndex = cue.index;
        break;
      }
      if (cue.start_ms > currentMs) {
        break;
      }
    }
    setActiveCueIndex((previousIndex) => {
      if (previousIndex === nextActiveIndex) {
        return previousIndex;
      }
      if (nextActiveIndex != null) {
        const activeElement = cueListRef.current?.querySelector(
          `[data-cue-index="${nextActiveIndex}"]`,
        );
        activeElement?.scrollIntoView({ block: "nearest" });
      }
      return nextActiveIndex;
    });
  }, [subtitleCues]);

  const handleCueClick = useCallback((cue: TranscriptCue) => {
    const mediaElement = mediaElementRef.current;
    if (mediaElement == null) {
      return;
    }
    mediaElement.currentTime = cue.start_ms / 1000;
    void mediaElement.play().catch(() => {
      /* Autoplay restrictions are non-fatal; the user can press play. */
    });
  }, []);

  if (mediaOverview == null) {
    return <p className="muted small">正在加载媒体信息…</p>;
  }

  if (mediaOverview.media_purged && mediaOverview.files.length === 0) {
    return (
      <p className="muted small">
        媒体已清理。转写与总结产物仍可查看；下载任务可重跑「获取媒体」重新下载。
      </p>
    );
  }

  if (mediaOverview.files.length === 0) {
    return <p className="muted small">该任务还没有媒体文件。</p>;
  }

  const showGeneratePreviewButton =
    !mediaOverview.has_preview &&
    mediaOverview.files.some(
      (file) => !file.is_audio && file.playability !== "direct",
    );

  return (
    <article className="card soft">
      <div className="card-title-row">
        <h3>媒体预览</h3>
        <div className="preview-actions">
          <label className="preview-file-select">
            <span className="visually-hidden">选择媒体文件</span>
            <select
              value={selectedFileName ?? ""}
              onChange={(event) => {
                setSelectedFileName(event.target.value);
                setPlaybackError(null);
              }}
            >
              {mediaOverview.files.map((file) => (
                <option key={file.file_name} value={file.file_name}>
                  {MEDIA_KIND_LABEL[file.kind] ?? file.kind} · {file.file_name}{" "}
                  ({formatBytes(file.size_bytes)})
                </option>
              ))}
            </select>
          </label>
          {showGeneratePreviewButton && (
            <button
              type="button"
              className="btn secondary"
              disabled={isGeneratingPreview || isJobBusy}
              title="ffmpeg -c copy 转封装为 MP4，不重新编码"
              onClick={() => void handleGeneratePreview()}
            >
              {isGeneratingPreview ? "生成中…" : "生成预览副本"}
            </button>
          )}
        </div>
      </div>

      {selectedFile == null || mediaSourceUrl == null ? (
        <p className="muted small">请选择要播放的媒体文件。</p>
      ) : selectedFile.playability === "incompatible" ? (
        <p className="muted small">
          该容器（{selectedFile.file_name}）无法在应用内播放。
          {selectedFile.is_audio
            ? ""
            : "可点击「生成预览副本」转封装为 MP4，或用外部播放器打开任务目录。"}
        </p>
      ) : (
        <div className="preview-player-layout">
          <div className="preview-player-main">
            {selectedFile.is_audio ? (
              <audio
                key={mediaSourceUrl}
                ref={(element) => {
                  mediaElementRef.current = element;
                }}
                className="preview-audio"
                src={mediaSourceUrl}
                controls
                onTimeUpdate={handleTimeUpdate}
                onError={() =>
                  setPlaybackError(
                    "播放失败：音频编码可能不受 WebView 支持，请用外部播放器打开。",
                  )
                }
              />
            ) : (
              <video
                key={mediaSourceUrl}
                ref={(element) => {
                  mediaElementRef.current = element;
                }}
                className="preview-video"
                src={mediaSourceUrl}
                controls
                onTimeUpdate={handleTimeUpdate}
                onError={() =>
                  setPlaybackError(
                    selectedFile.playability === "maybe"
                      ? "播放失败：该容器/编码不受 WebView 支持，可尝试「生成预览副本」。"
                      : "播放失败：编码可能不受 WebView 支持，请用外部播放器打开。",
                  )
                }
              />
            )}
            {playbackError && <p className="error-text small">{playbackError}</p>}
          </div>

          {subtitleCues.length > 0 && (
            <ul className="preview-cue-list" ref={cueListRef}>
              {subtitleCues.map((cue) => (
                <li
                  key={cue.index}
                  data-cue-index={cue.index}
                  className={
                    cue.index === activeCueIndex
                      ? "preview-cue-row active"
                      : "preview-cue-row"
                  }
                >
                  <button
                    type="button"
                    className="preview-cue-btn"
                    title="跳转到该时间点"
                    onClick={() => handleCueClick(cue)}
                  >
                    <span className="preview-cue-time mono">
                      {formatCueTimestamp(cue.start_ms)}
                    </span>
                    <span className="preview-cue-text">{cue.text}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </article>
  );
}
