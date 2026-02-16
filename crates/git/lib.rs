use crate::service::core::RepoServices;
use std::sync::Arc;

use tokio::sync::RwLock;

pub mod models;
pub mod parsers;
pub mod service;

pub struct AppState {
    pub services: RwLock<Option<Arc<RepoServices>>>,
}
