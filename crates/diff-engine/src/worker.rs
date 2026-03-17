use std::{sync::Arc, time::Instant};

use tauri::Emitter;
use tokio::task;

use crate::{
    DiffEngineContext,
    highlighter::highlight_text,
    hunk::build_hunks,
    models::{DiffJob, DiffPayload, DiffStatus},
    queue::{JobQueue, JobReceiver},
    reader::FileContentReader,
    semantic::SemanticAnalyzer,
};

/// Entry point for each worker task. Loops indefinitely, pulling the latest
/// job from the shared queue until shutdown. Workers always process the most
/// recent job, ignoring any older pending jobs.
pub async fn run_worker(
    id: usize,
    receiver: Arc<JobReceiver>,
    ctx: Arc<DiffEngineContext>,
    app: tauri::AppHandle,
    queue: Arc<JobQueue>,
) {
    log::debug!("diff-engine worker {id} started (repo: {})", ctx.repo_path);

    loop {
        // Wait for a job to be available
        receiver.notify.notified().await;

        // Try to fetch the pending job
        let job = {
            let mut pending = receiver.pending_job.lock().await;
            pending.take()
        };

        if let Some(job) = job {
            let job_id = job.id;

            // Check if this job was cancelled before we start processing
            if queue.is_job_cancelled(job_id).await {
                log::warn!("Worker {id} skipped cancelled job {job_id}");
                continue;
            }

            let start = Instant::now();
            process_job(job, &ctx, &app, &queue).await;
            let duration = start.elapsed();
            log::info!("Worker {id} processed job in: {:?}", duration);
        }
        // If no job was available, just loop and wait again
    }
}

async fn process_job(
    job: DiffJob,
    ctx: &Arc<DiffEngineContext>,
    app: &tauri::AppHandle,
    queue: &Arc<JobQueue>,
) {
    let job_id = job.id;
    let job_id_str = job_id.to_string();
    let file_path = job.request.file_path.clone();

    // 1. Immediately acknowledge — frontend can show a loading state.
    emit(
        app,
        DiffPayload {
            job_id: job_id_str.clone(),
            file_path: file_path.clone(),
            status: DiffStatus::Processing,
            hunks: vec![],
            semantic_changes: vec![],
            old_lines: vec![],
            new_lines: vec![],
        },
    );

    // Check if job was cancelled before we start heavy work
    if queue.is_job_cancelled(job_id).await {
        log::warn!("Job {job_id_str} was cancelled, aborting");
        return;
    }

    // ── Phase 2: read files ─────────────────────────────────────────
    let phase2 = Instant::now();
    let old_fut = FileContentReader::read_index_content(&ctx.repo_path, &file_path);

    let new_fut = task::spawn_blocking({
        let repo_path = ctx.repo_path.clone();
        let file_path = file_path.clone();
        move || FileContentReader::read_working_tree_content(&repo_path, &file_path)
    });

    let (old_res, new_res) = tokio::join!(old_fut, new_fut);

    let old = match old_res {
        Ok(content) => content,
        Err(e) => {
            let err = format!("index read failed: {e}");
            log::error!("{err}");
            emit(
                app,
                DiffPayload {
                    job_id: job_id_str.clone(),
                    file_path,
                    status: DiffStatus::Error(e),
                    hunks: vec![],
                    semantic_changes: vec![],
                    old_lines: vec![],
                    new_lines: vec![],
                },
            );
            return;
        }
    };

    let new = match new_res {
        Ok(Ok(content)) => content,
        Ok(Err(e)) => {
            log::error!("failed to read working tree content for {file_path}: {e}");
            emit(
                app,
                DiffPayload {
                    job_id: job_id_str.clone(),
                    file_path,
                    status: DiffStatus::Error(e),
                    hunks: vec![],
                    semantic_changes: vec![],
                    old_lines: vec![],
                    new_lines: vec![],
                },
            );
            return;
        }
        Err(e) => {
            let err = format!("task join error: {e}");
            log::error!("{err}");
            emit(
                app,
                DiffPayload {
                    job_id: job_id_str.clone(),
                    file_path,
                    status: DiffStatus::Error(err),
                    hunks: vec![],
                    semantic_changes: vec![],
                    old_lines: vec![],
                    new_lines: vec![],
                },
            );
            return;
        }
    };

    let old_text = old.map(|c| c.text).unwrap_or_default();
    let new_text = new.map(|c| c.text).unwrap_or_default();

    let phase2_duration = phase2.elapsed();
    log::debug!("phase2 (file reading) takes: {:?}", phase2_duration);

    // Bail early if superseded by a newer request — no point highlighting stale content
    if queue.is_job_cancelled(job_id).await {
        log::debug!("Job {job_id_str} superseded after file read, aborting");
        return;
    }

    log::debug!(
        "job {job_id_str}: old={} bytes, new={} bytes",
        old_text.len(),
        new_text.len()
    );

    let highlight_time = Instant::now();

    let theme = match ctx.theme_manager.get_theme() {
        Ok(t) => t,
        Err(e) => {
            log::error!("failed to get theme: {e}");
            emit(
                app,
                DiffPayload {
                    job_id: job_id_str.clone(),
                    file_path,
                    status: DiffStatus::Error(e),
                    hunks: vec![],
                    semantic_changes: vec![],
                    old_lines: vec![],
                    new_lines: vec![],
                },
            );
            return;
        }
    };

    // High-performance parallel highlighting with shared context
    let old_highlight = {
        let file_path = file_path.clone();
        let old_text = old_text.clone();
        let syntax_set = ctx.syntax_set.clone();
        let theme = theme.clone();
        let cache = ctx.syntax_cache.clone();
        task::spawn_blocking(move || {
            highlight_text(&file_path, &old_text, &syntax_set, &theme, Some(&cache))
        })
    };

    let new_highlight = {
        let file_path = file_path.clone();
        let new_text = new_text.clone();
        let syntax_set = ctx.syntax_set.clone();
        let theme = theme.clone();
        let cache = ctx.syntax_cache.clone();
        task::spawn_blocking(move || {
            highlight_text(&file_path, &new_text, &syntax_set, &theme, Some(&cache))
        })
    };

    let (old_highlight_res, new_highlight_res) = tokio::join!(old_highlight, new_highlight);

    let old_lines = match old_highlight_res {
        Ok(Ok(lines)) => lines,
        Ok(Err(e)) => {
            log::error!("failed to highlight old content for {file_path}: {e}");
            emit(
                app,
                DiffPayload {
                    job_id: job_id_str.clone(),
                    file_path,
                    status: DiffStatus::Error(e),
                    hunks: vec![],
                    semantic_changes: vec![],
                    old_lines: vec![],
                    new_lines: vec![],
                },
            );
            return;
        }
        Err(e) => {
            let err = format!("old highlight task join error: {e}");
            log::error!("{err}");
            emit(
                app,
                DiffPayload {
                    job_id: job_id_str.clone(),
                    file_path,
                    status: DiffStatus::Error(err),
                    hunks: vec![],
                    semantic_changes: vec![],
                    old_lines: vec![],
                    new_lines: vec![],
                },
            );
            return;
        }
    };

    let new_lines = match new_highlight_res {
        Ok(Ok(lines)) => lines,
        Ok(Err(e)) => {
            log::error!("failed to highlight new content for {file_path}: {e}");
            emit(
                app,
                DiffPayload {
                    job_id: job_id_str.clone(),
                    file_path,
                    status: DiffStatus::Error(e),
                    hunks: vec![],
                    semantic_changes: vec![],
                    old_lines: vec![],
                    new_lines: vec![],
                },
            );
            return;
        }
        Err(e) => {
            let err = format!("new highlight task join error: {e}");
            log::error!("{err}");
            emit(
                app,
                DiffPayload {
                    job_id: job_id_str.clone(),
                    file_path,
                    status: DiffStatus::Error(err),
                    hunks: vec![],
                    semantic_changes: vec![],
                    old_lines: vec![],
                    new_lines: vec![],
                },
            );
            return;
        }
    };

    let highlight_duration = highlight_time.elapsed();
    log::debug!("highlighting takes: {:?}", highlight_duration);

    let diffing_time = Instant::now();

    let hunk_task = task::spawn_blocking({
        let old_text = old_text.clone();
        let new_text = new_text.clone();
        let old_lines = old_lines.clone();
        let new_lines = new_lines.clone();
        move || build_hunks(&old_text, &new_text, &old_lines, &new_lines, 3)
    });

    let semantic_task = task::spawn_blocking({
        let old_text = old_text.clone();
        let new_text = new_text.clone();
        let file_path = file_path.clone();
        move || SemanticAnalyzer::analyze(&old_text, &new_text, &file_path)
    });

    let (hunk_res, semantic_res) = tokio::join!(hunk_task, semantic_task);

    let hunks = match hunk_res {
        Ok(hunks) => hunks,
        Err(e) => {
            let err = format!("hunk task join error: {e}");
            log::error!("{err}");
            emit(
                app,
                DiffPayload {
                    job_id: job_id_str.clone(),
                    file_path,
                    status: DiffStatus::Error(err),
                    hunks: vec![],
                    semantic_changes: vec![],
                    old_lines: vec![],
                    new_lines: vec![],
                },
            );
            return;
        }
    };

    let semantic_changes = match semantic_res {
        Ok(changes) => changes,
        Err(e) => {
            let err = format!("semantic analysis task join error: {e}");
            log::error!("{err}");
            emit(
                app,
                DiffPayload {
                    job_id: job_id_str.clone(),
                    file_path,
                    status: DiffStatus::Error(err),
                    hunks: vec![],
                    semantic_changes: vec![],
                    old_lines: vec![],
                    new_lines: vec![],
                },
            );
            return;
        }
    };

    let diffing_duration = diffing_time.elapsed();
    log::debug!("diffing takes: {:?}", diffing_duration);

    // Final guard: if a newer request arrived while we were processing, discard
    // this result. The newer job is already queued and will emit its own event.
    if queue.is_job_cancelled(job_id).await {
        log::debug!("Job {job_id_str} superseded before emit, discarding result");
        return;
    }

    emit(
        app,
        DiffPayload {
            job_id: job_id_str,
            file_path,
            status: DiffStatus::Ready,
            hunks,
            semantic_changes,
            old_lines,
            new_lines,
        },
    );
}

fn emit(app: &tauri::AppHandle, payload: DiffPayload) {
    if let Err(e) = app.emit("diff_event", &payload) {
        log::error!("failed to emit diff_event: {e}");
    }
}
