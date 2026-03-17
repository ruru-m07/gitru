use std::sync::Arc;

use tokio::sync::RwLock;

use tauri::Emitter;
use uuid::Uuid;

pub struct DiffEngineState {
    pub services: RwLock<Option<Arc<DiffEngine>>>,
}

pub struct DiffEngine {
    pub ctx: Arc<DiffEngineContext>,
}

pub struct DiffEngineContext {
    pub repo_path: String,
}

impl DiffEngineContext {
    pub fn new(repo_path: &str) -> Result<Self, String> {
        Ok(Self {
            repo_path: repo_path.to_string(),
        })
    }
}

/// the diff engine is the responsible for end to end sourse to delivery of the diff
/// from the user we get the file path and a optional commit id,
/// every time user request a diff we spawn a prosses and quickly return a unique id.
/// rest of the things will be done by the worker thread,
/// the worker thread will get the file content, generate the diff and emit an event with the diff and the unique id,
///
/// architecture:
///                  ┌──────────────────────┐
///                  │      Frontend        │
///                  │      (React)         │
///                  └─────────┬────────────┘
///                            │
///                            │ invoke("create_diff_job")
///                            ▼
///              ┌──────────────────────────────┐
///              │   Tauri Command (Rust)       │
///              └─────────┬────────────────────┘
///                        │
///                        │ generate job_id
///                        │ push to queue
///                        ▼
///             ┌───────────────────────────────┐
///             │        Diff Job Queue         │
///             │  (channel / crossbeam / mpsc) │
///             └─────────────────┬─────────────┘
///                               │
///                 ┌─────────────┼─────────────┐
///                 │             │             │
///                 ▼             ▼             ▼
///         ┌────────────┐ ┌────────────┐ ┌────────────┐
///         │  Worker 1  │ │  Worker 2  │ │  Worker N  │
///         └─────┬──────┘ └─────┬──────┘ └─────┬──────┘
///               │              │              │
///               └──────┬───────┴───────┬──────┘
///                      │               │
///                      ▼               ▼
///              ┌───────────────────────────────┐
///              │     Diff Processing Pipeline  │
///              │                               │
///              │  read files                   │
///              │       │                       │
///              │       ▼                       │
///              │  detect language              │
///              │       │                       │
///              │       ▼                       │
///              │  Tree-sitter parse (AST)      │
///              │       │                       │
///              │       ▼                       │
///              │  normalize nodes              │
///              │       │                       │
///              │       ▼                       │
///              │  semantic diff                │
///              │       │                       │
///              │       ▼                       │
///              │  build result payload         │
///              └──────────────┬────────────────┘
///                             │
///                             │ emit("diff-result", { job_id, data })
///                             ▼
///                ┌──────────────────────────┐
///                │     Event Bus (Tauri)    │
///                └──────────┬───────────────┘
///                           │
///                           ▼
///                  ┌───────────────────────┐
///                  │      Frontend         │
///                  │  listen("diff-result")│
///                  │                       │
///                  │  render diff UI       │
///                  └───────────────────────┘
///
/// This architecture allows us to handle multiple diff requests concurrently.
/// The frontend can easily correlate diff results with their requests using the unique job_id.
///
/// we will use tree-sitter to parse the code and generate the diff,
/// the diff will be semantic and not just text based,
///
/// Diff engine will return/stream the diff result. in a was so it not just return the
///
/// Rust backend
/// │
/// ├─ read old + new full text
/// ├─ syntect highlight old → old_tokens[][]   (stateful, correct multiline)
/// ├─ syntect highlight new → new_tokens[][]
/// ├─ similar line diff → Vec<LineOp>
/// ├─ tree-sitter AST → semantic_changes[]
/// ├─ group into hunks
/// │
/// └─ emit single DiffPayload {
///        lines: DiffLine[],          ← tokens already inside
///        hunks: Hunk[],
///        semantic_changes: Change[],
///    }
///
impl DiffEngine {
    pub fn new(repo_path: &str) -> Result<Self, String> {
        Ok(Self {
            ctx: Arc::new(DiffEngineContext::new(repo_path)?),
        })
    }

    pub fn spawn_worker(&self, file_path: &str, app: tauri::AppHandle) -> String {
        let id = Uuid::new_v4();

        println!(
            "Spawning diff worker for file: {}, with id: {}",
            file_path, self.ctx.repo_path
        );

        std::thread::spawn(move || {
            app.emit(
                "diff_event",
                format!("Diff ready for with id {}", id.clone().to_string(),),
            )
            .expect("Failed to emit diff event");
        });

        return id.to_string();
    }

    fn get_file_content(&self, file_path: &str, commitId: Option<&str>) -> String {
        todo!()
    }

    fn generate_tree_sitter_diff(&self, old_content: &str, new_content: &str) -> String {
        todo!()
    }
}
