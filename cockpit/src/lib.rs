//! Cockpit 服務骨架：config、runtimes、http 模組，加上把它們組起來的 app（`main` 的本體）。

pub mod agent;
pub mod app;
pub mod config;
pub mod files;
pub mod git;
pub mod http;
pub mod launch;
pub mod progress;
pub mod progress_service;
pub mod runtimes;
pub mod source_check;
pub mod vendor;
