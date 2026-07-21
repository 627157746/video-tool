//! Local full-text search index (SQLite FTS5) under workspace/index/.

use crate::error::{AppError, AppResult};
use crate::models::Job;
use crate::workspace;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const INDEX_DIR_NAME: &str = "index";
const INDEX_DB_NAME: &str = "search.sqlite3";

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub job_id: String,
    pub title: String,
    pub kind: String,
    pub field: String,
    pub snippet: String,
    pub path: String,
}

fn index_db_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(INDEX_DIR_NAME).join(INDEX_DB_NAME)
}

fn open_connection(workspace_root: &Path) -> AppResult<Connection> {
    let directory = workspace_root.join(INDEX_DIR_NAME);
    fs::create_dir_all(&directory)?;
    let path = index_db_path(workspace_root);
    let connection = Connection::open(&path)
        .map_err(|error| AppError::message(format!("打开搜索索引失败: {error}")))?;
    connection
        .execute_batch(
            "
            PRAGMA journal_mode=WAL;
            CREATE VIRTUAL TABLE IF NOT EXISTS documents USING fts5(
                job_id UNINDEXED,
                title UNINDEXED,
                kind UNINDEXED,
                field UNINDEXED,
                path UNINDEXED,
                body,
                tokenize = 'unicode61'
            );
            ",
        )
        .map_err(|error| AppError::message(format!("初始化搜索索引失败: {error}")))?;
    Ok(connection)
}

/// Remove all indexed rows for a job, then re-index current artifacts.
pub fn upsert_job(workspace_root: &Path, job: &Job) -> AppResult<()> {
    let job_dir = workspace::validated_job_dir(workspace_root, &job.id)?;
    let connection = open_connection(workspace_root)?;
    connection
        .execute("DELETE FROM documents WHERE job_id = ?1", params![job.id])
        .map_err(|error| AppError::message(format!("清理搜索索引失败: {error}")))?;

    let title = job.display_title();
    let kind = match job.source.kind {
        crate::models::JobKind::Download => "download",
        crate::models::JobKind::LiveRecord => "live_record",
        crate::models::JobKind::ImportLocal => "import_local",
    };

    let mut owned_docs: Vec<(String, String, String)> = Vec::new();
    let plain_path = job_dir.join("transcript").join("plain.txt");
    if plain_path.exists() {
        if let Ok(body) = fs::read_to_string(&plain_path) {
            if !body.trim().is_empty() {
                owned_docs.push((
                    "transcript".to_string(),
                    "transcript/plain.txt".to_string(),
                    body,
                ));
            }
        }
    }
    let summary_path = job_dir.join("summary").join("summary.md");
    if summary_path.exists() {
        if let Ok(body) = fs::read_to_string(&summary_path) {
            if !body.trim().is_empty() {
                owned_docs.push((
                    "summary".to_string(),
                    "summary/summary.md".to_string(),
                    body,
                ));
            }
        }
    }
    let by_template_dir = job_dir.join("summary").join("by_template");
    if by_template_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&by_template_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) != Some("md") {
                    continue;
                }
                let Ok(body) = fs::read_to_string(&path) else {
                    continue;
                };
                if body.trim().is_empty() {
                    continue;
                }
                let file_name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| "extra.md".to_string());
                owned_docs.push((
                    "summary_template".to_string(),
                    format!("summary/by_template/{file_name}"),
                    body,
                ));
            }
        }
    }

    let mut insert = connection
        .prepare(
            "INSERT INTO documents (job_id, title, kind, field, path, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .map_err(|error| AppError::message(format!("准备搜索索引写入失败: {error}")))?;

    for (field, path, body) in owned_docs {
        insert
            .execute(params![job.id, title, kind, field, path, body])
            .map_err(|error| AppError::message(format!("写入搜索索引失败: {error}")))?;
    }
    Ok(())
}

pub fn remove_job(workspace_root: &Path, job_id: &str) -> AppResult<()> {
    if !index_db_path(workspace_root).exists() {
        return Ok(());
    }
    let connection = open_connection(workspace_root)?;
    connection
        .execute("DELETE FROM documents WHERE job_id = ?1", params![job_id])
        .map_err(|error| AppError::message(format!("删除搜索索引失败: {error}")))?;
    Ok(())
}

/// Rebuild the entire index from all jobs under the workspace.
pub fn rebuild_all(workspace_root: &Path) -> AppResult<u32> {
    let connection = open_connection(workspace_root)?;
    connection
        .execute("DELETE FROM documents", [])
        .map_err(|error| AppError::message(format!("清空搜索索引失败: {error}")))?;
    drop(connection);

    let jobs = workspace::list_jobs(workspace_root)?;
    let mut count = 0u32;
    for job in jobs {
        upsert_job(workspace_root, &job)?;
        count += 1;
    }
    Ok(count)
}

pub fn search(workspace_root: &Path, query: &str, limit: u32) -> AppResult<Vec<SearchHit>> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let limit = limit.clamp(1, 100);
    let connection = open_connection(workspace_root)?;
    let fts_query = build_fts_query(trimmed);

    let mut statement = connection
        .prepare(
            "SELECT job_id, title, kind, field, path,
                    snippet(documents, 5, '「', '」', '…', 24) AS snip
             FROM documents
             WHERE documents MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )
        .map_err(|error| AppError::message(format!("准备搜索查询失败: {error}")))?;

    let rows = statement
        .query_map(params![fts_query, limit as i64], |row| {
            Ok(SearchHit {
                job_id: row.get(0)?,
                title: row.get(1)?,
                kind: row.get(2)?,
                field: row.get(3)?,
                path: row.get(4)?,
                snippet: row.get(5)?,
            })
        })
        .map_err(|error| AppError::message(format!("搜索失败: {error}")))?;

    let mut hits = Vec::new();
    for row in rows {
        hits.push(row.map_err(|error| AppError::message(format!("读取搜索结果失败: {error}")))?);
    }
    Ok(hits)
}

fn build_fts_query(raw: &str) -> String {
    let tokens: Vec<String> = raw
        .split_whitespace()
        .map(|token| {
            let escaped = token.replace('"', "\"\"");
            format!("\"{escaped}\"")
        })
        .collect();
    if tokens.is_empty() {
        format!("\"{}\"", raw.replace('"', "\"\""))
    } else {
        tokens.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Job, JobKind, JobSource, PipelineOptions};
    use uuid::Uuid;

    #[test]
    fn indexes_and_searches_transcript() {
        let root = std::env::temp_dir().join(format!("video-tool-search-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("jobs")).unwrap();
        let job = Job::new(
            JobSource {
                kind: JobKind::ImportLocal,
                url: None,
                title: Some("测试任务".into()),
                local_path: Some("a.mp4".into()),
                segment_minutes: None,
                download_cookies_mode: None,
                download_cookies_file: None,
                download_cookies_from_browser: None,
            },
            PipelineOptions::default(),
        );
        let job_dir = root.join("jobs").join(&job.id);
        fs::create_dir_all(job_dir.join("transcript")).unwrap();
        fs::create_dir_all(job_dir.join("summary")).unwrap();
        fs::write(
            job_dir.join("transcript").join("plain.txt"),
            "这里有一段关于 OpenAI 和视频工具的讨论",
        )
        .unwrap();
        fs::write(
            job_dir.join("summary").join("summary.md"),
            "# 总结\n\n提到了 OpenAI API",
        )
        .unwrap();
        workspace::save_job(&root, &job).unwrap();
        upsert_job(&root, &job).unwrap();

        let hits = search(&root, "OpenAI", 10).unwrap();
        assert!(!hits.is_empty());
        assert!(hits.iter().any(|hit| hit.job_id == job.id));

        remove_job(&root, &job.id).unwrap();
        let after = search(&root, "OpenAI", 10).unwrap();
        assert!(after.iter().all(|hit| hit.job_id != job.id));
        let _ = fs::remove_dir_all(root);
    }
}
