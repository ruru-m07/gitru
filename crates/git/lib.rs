use crate::core::RepoServices;
use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

pub mod cache;
pub mod context;
pub mod core;
pub mod models;
pub mod parsers;
pub mod runner;
pub mod service;

pub struct AppState {
    pub services: RwLock<HashMap<String, Arc<RepoServices>>>,
}
