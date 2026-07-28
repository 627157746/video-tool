import { useCallback, useEffect, useRef, useState } from "react";
import { getTranscriptCues, saveTranscriptEdit } from "../api";
import { confirmAction } from "../confirmAction";
import type { Job, TranscriptCueDocument } from "../types";

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

interface TranscriptProofreadPanelProps {
  jobId: string;
  /** Disable saving while the job is running / queued. */
  isJobBusy: boolean;
  /** Job already carries manual edits (shows badge). */
  transcriptEditedAt?: string | null;
  onSaved: (updatedJob: Job) => void;
  onError: (message: string) => void;
  onStatus: (message: string) => void;
}

/**
 * Job detail "proofread" section (v0.3): edit merged transcript per subtitle
 * cue (SRT jobs) or as whole text (fallback). Saving invalidates chapters and
 * summaries, which must be re-run.
 */
export function TranscriptProofreadPanel({
  jobId,
  isJobBusy,
  transcriptEditedAt,
  onSaved,
  onError,
  onStatus,
}: TranscriptProofreadPanelProps) {
  const [cueDocument, setCueDocument] = useState<TranscriptCueDocument | null>(
    null,
  );
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  /** Cue index → edited text (only cues the user touched). */
  const [cueDrafts, setCueDrafts] = useState<Map<number, string>>(new Map());
  const [plainDraft, setPlainDraft] = useState("");
  const loadRequestVersionRef = useRef(0);

  const loadCues = useCallback(async () => {
    const requestVersion = ++loadRequestVersionRef.current;
    setIsLoading(true);
    try {
      const loadedDocument = await getTranscriptCues(jobId);
      if (loadRequestVersionRef.current !== requestVersion) {
        return;
      }
      setCueDocument(loadedDocument);
      setCueDrafts(new Map());
      setPlainDraft(loadedDocument.plain_text);
    } catch (error) {
      if (loadRequestVersionRef.current === requestVersion) {
        onError(error instanceof Error ? error.message : String(error));
      }
    } finally {
      if (loadRequestVersionRef.current === requestVersion) {
        setIsLoading(false);
      }
    }
  }, [jobId, onError]);

  useEffect(() => {
    void loadCues();
  }, [loadCues]);

  const hasCueChanges = cueDrafts.size > 0;
  const hasPlainChanges =
    cueDocument != null &&
    !cueDocument.has_srt &&
    plainDraft !== cueDocument.plain_text;
  const hasChanges = cueDocument?.has_srt ? hasCueChanges : hasPlainChanges;

  const handleSave = useCallback(async () => {
    if (cueDocument == null || !hasChanges) {
      return;
    }
    if (
      !(await confirmAction(
        "确定保存校对结果吗？\n\n保存后章节与总结会失效，需要重新运行对应步骤才能更新。",
      ))
    ) {
      return;
    }
    setIsSaving(true);
    try {
      const updatedJob = cueDocument.has_srt
        ? await saveTranscriptEdit({
            job_id: jobId,
            cues: Array.from(cueDrafts.entries()).map(([index, text]) => ({
              index,
              text,
            })),
          })
        : await saveTranscriptEdit({ job_id: jobId, plain_text: plainDraft });
      onStatus("校对已保存；章节与总结已失效，需要重跑对应步骤");
      onSaved(updatedJob);
      await loadCues();
    } catch (error) {
      onError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSaving(false);
    }
  }, [
    cueDocument,
    cueDrafts,
    hasChanges,
    jobId,
    loadCues,
    onError,
    onSaved,
    onStatus,
    plainDraft,
  ]);

  if (isLoading && cueDocument == null) {
    return <p className="muted small">正在加载转写内容…</p>;
  }

  if (cueDocument == null) {
    return <p className="muted small">尚未加载转写内容。</p>;
  }

  if (!cueDocument.plain_exists && !cueDocument.has_srt) {
    return (
      <p className="muted small">
        该任务还没有合并后的转写产物；请先完成「合并文字」步骤。
      </p>
    );
  }

  return (
    <article className="card soft">
      <div className="card-title-row">
        <h3>
          {cueDocument.has_srt ? "按字幕行校对" : "整篇校对（无字幕时间轴）"}
        </h3>
        <div className="proofread-actions">
          {transcriptEditedAt && (
            <span className="badge" title={`校对于 ${transcriptEditedAt}`}>
              已手工校对
            </span>
          )}
          <button
            type="button"
            className="btn secondary"
            disabled={!hasChanges || isSaving || isJobBusy}
            onClick={() => void loadCues()}
          >
            放弃修改
          </button>
          <button
            type="button"
            className="btn"
            disabled={!hasChanges || isSaving || isJobBusy}
            title={
              isJobBusy
                ? "任务运行期间不能保存校对"
                : hasChanges
                  ? "保存后合并字幕与全文同步更新，章节/总结需重跑"
                  : "没有修改"
            }
            onClick={() => void handleSave()}
          >
            {isSaving ? "保存中…" : "保存校对"}
          </button>
        </div>
      </div>

      <p className="muted small">
        保存前会自动备份上一版（plain.prev.txt / srt.prev.srt）。清空某行文本表示删除该字幕行。
      </p>

      {cueDocument.has_srt ? (
        <ul className="proofread-cue-list">
          {cueDocument.cues.map((cue) => {
            const draftText = cueDrafts.get(cue.index);
            const isDirty = draftText != null && draftText !== cue.text;
            return (
              <li
                key={cue.index}
                className={
                  isDirty ? "proofread-cue-row dirty" : "proofread-cue-row"
                }
              >
                <span className="proofread-cue-time mono" title={cue.timing_line}>
                  {formatCueTimestamp(cue.start_ms)}
                </span>
                <textarea
                  className="proofread-cue-input"
                  rows={1}
                  value={draftText ?? cue.text}
                  disabled={isSaving || isJobBusy}
                  aria-label={`字幕行 ${cue.index + 1}`}
                  onChange={(event) => {
                    const nextText = event.target.value;
                    setCueDrafts((previousDrafts) => {
                      const nextDrafts = new Map(previousDrafts);
                      if (nextText === cue.text) {
                        nextDrafts.delete(cue.index);
                      } else {
                        nextDrafts.set(cue.index, nextText);
                      }
                      return nextDrafts;
                    });
                  }}
                />
              </li>
            );
          })}
        </ul>
      ) : (
        <textarea
          className="proofread-plain-input"
          rows={18}
          value={plainDraft}
          disabled={isSaving || isJobBusy}
          aria-label="合并全文"
          onChange={(event) => setPlainDraft(event.target.value)}
        />
      )}
    </article>
  );
}
