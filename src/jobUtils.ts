import type {
  Job,
  JobGroupDefinition,
  JobListItem,
  JobStep,
} from "./types";

export function getPipelineStepProgress(job: Job, step: JobStep) {
  return (
    job.step_statuses.find((progress) => progress.step === step) ?? {
      step,
      status: "pending" as const,
      detail: "尚未运行，可随时手动执行",
    }
  );
}

export function normalizeJobGroup(
  group: string | null | undefined,
): string | null {
  const trimmedGroup = group?.trim() ?? "";
  return trimmedGroup ? trimmedGroup : null;
}

/** Resolve Job.group (id or legacy free-text) to a display name. */
export function resolveJobGroupLabel(
  groupValue: string | null | undefined,
  catalog: JobGroupDefinition[],
): string | null {
  const trimmedGroupValue = normalizeJobGroup(groupValue);
  if (!trimmedGroupValue) {
    return null;
  }
  const matchedById = catalog.find(
    (groupEntry) => groupEntry.id === trimmedGroupValue,
  );
  if (matchedById) {
    return matchedById.name;
  }
  const matchedByName = catalog.find(
    (groupEntry) =>
      groupEntry.name.trim().toLowerCase() ===
      trimmedGroupValue.toLowerCase(),
  );
  if (matchedByName) {
    return matchedByName.name;
  }
  return trimmedGroupValue;
}

/**
 * Stable filter key for a job group value.
 * Known catalog entries use `id:<id>`; orphans use `legacy:<name>`.
 */
export function resolveJobGroupFilterKey(
  groupValue: string | null | undefined,
  catalog: JobGroupDefinition[],
): string | null {
  const trimmedGroupValue = normalizeJobGroup(groupValue);
  if (!trimmedGroupValue) {
    return null;
  }
  const matchedById = catalog.find(
    (groupEntry) => groupEntry.id === trimmedGroupValue,
  );
  if (matchedById) {
    return `id:${matchedById.id}`;
  }
  const matchedByName = catalog.find(
    (groupEntry) =>
      groupEntry.name.trim().toLowerCase() ===
      trimmedGroupValue.toLowerCase(),
  );
  if (matchedByName) {
    return `id:${matchedByName.id}`;
  }
  return `legacy:${trimmedGroupValue.toLowerCase()}`;
}

/**
 * Value for the job-detail group <select>: catalog id when known,
 * raw legacy string when orphaned, or "" when ungrouped.
 */
export function resolveJobGroupSelectValue(
  groupValue: string | null | undefined,
  catalog: JobGroupDefinition[],
): string {
  const trimmedGroupValue = normalizeJobGroup(groupValue);
  if (!trimmedGroupValue) {
    return "";
  }
  const matchedById = catalog.find(
    (groupEntry) => groupEntry.id === trimmedGroupValue,
  );
  if (matchedById) {
    return matchedById.id;
  }
  const matchedByName = catalog.find(
    (groupEntry) =>
      groupEntry.name.trim().toLowerCase() ===
      trimmedGroupValue.toLowerCase(),
  );
  if (matchedByName) {
    return matchedByName.id;
  }
  return trimmedGroupValue;
}

export function createClientGroupId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `group-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

export function jobToListItem(job: Job): JobListItem {
  return {
    id: job.id,
    status: job.status,
    kind: job.source.kind,
    title: job.source.title?.trim()
      ? job.source.title
      : job.source.url || job.source.local_path || job.id,
    source_reference: job.source.url || job.source.local_path || "",
    group: normalizeJobGroup(job.group),
    batch_id: normalizeJobGroup(job.batch_id),
    current_step: job.current_step,
    progress: job.progress ?? 0,
    error_message: job.error_message,
    error_code: job.error_code,
    created_at: job.created_at,
    updated_at: job.updated_at,
  };
}

export function mergeJobListSnapshots(
  currentJobs: JobListItem[],
  refreshedJobs: JobListItem[],
): JobListItem[] {
  const refreshedJobIds = new Set(refreshedJobs.map((job) => job.id));
  const currentJobsById = new Map(currentJobs.map((job) => [job.id, job]));
  const mergedJobs = refreshedJobs.map((refreshedJob) => {
    const currentJob = currentJobsById.get(refreshedJob.id);
    if (currentJob && currentJob.updated_at >= refreshedJob.updated_at) {
      return currentJob;
    }
    return refreshedJob;
  });
  for (const currentJob of currentJobs) {
    if (!refreshedJobIds.has(currentJob.id)) {
      mergedJobs.push(currentJob);
    }
  }
  return mergedJobs.sort((left, right) =>
    right.created_at.localeCompare(left.created_at),
  );
}

export function resolveExistingDefaultId(
  preferredId: string,
  availableIds: string[],
): string {
  const normalizedPreferredId = preferredId.trim();
  if (availableIds.includes(normalizedPreferredId)) {
    return normalizedPreferredId;
  }
  return availableIds.find((availableId) => availableId.trim())?.trim() ?? "";
}

/**
 * Models often wrap the entire Markdown answer in a ```markdown fence.
 * Strip only that outer document fence so ReactMarkdown can render headings
 * and lists; nested fenced code blocks inside the body are kept.
 * A missing closing fence is still unwrapped.
 */
export function unwrapOuterMarkdownFence(markdownText: string): string {
  const trimmedText = markdownText.trim();
  if (!trimmedText.startsWith("```")) {
    return trimmedText;
  }

  const lines = trimmedText.split(/\r?\n/);
  const openingLine = lines[0]?.trim() ?? "";
  if (!openingLine.startsWith("```")) {
    return trimmedText;
  }

  const languageTag = openingLine.replace(/^`+/, "").trim();
  if (languageTag && !/^[A-Za-z0-9_-]+$/.test(languageTag)) {
    return trimmedText;
  }

  let bodyLines = lines.slice(1);
  if (
    bodyLines.length > 0 &&
    bodyLines[bodyLines.length - 1]?.trim() === "```"
  ) {
    bodyLines = bodyLines.slice(0, -1);
  }

  return bodyLines.join("\n").trim();
}

/** Dedupe/trim model names and ensure the default model is present. */
export function normalizeProviderModels(
  models: string[],
  defaultModel: string,
): { models: string[]; default_model: string } {
  const seenModelNames = new Set<string>();
  const normalizedModels: string[] = [];
  for (const modelName of models) {
    const trimmedModelName = modelName.trim();
    if (!trimmedModelName || seenModelNames.has(trimmedModelName)) {
      continue;
    }
    seenModelNames.add(trimmedModelName);
    normalizedModels.push(trimmedModelName);
  }
  const trimmedDefaultModel = defaultModel.trim();
  if (trimmedDefaultModel && !seenModelNames.has(trimmedDefaultModel)) {
    normalizedModels.unshift(trimmedDefaultModel);
  }
  const resolvedDefaultModel =
    trimmedDefaultModel || normalizedModels[0] || "";
  return {
    models: normalizedModels,
    default_model: resolvedDefaultModel,
  };
}

export function providerModelsListText(models: string[]): string {
  return models.join("\n");
}

export function parseProviderModelsListText(text: string): string[] {
  return text
    .split(/[\n,]/)
    .map((modelName) => modelName.trim())
    .filter(Boolean);
}

export function resolveProviderModelOptions(
  provider:
    | { default_model: string; models?: string[] | null }
    | undefined,
): string[] {
  if (!provider) {
    return [];
  }
  return normalizeProviderModels(
    provider.models ?? [],
    provider.default_model,
  ).models;
}
