//! 假 HERDR（`FakeHerdr`）：程序內、監聽真實 transport 的假 server，供 change 1a 第 4 組
//! 與 change 1b 在 `test-support` feature 下重用（design D7）。
//!
//! 只在 `test-support` feature 開啟時編譯；本 crate 以 self dev-dependency 開這個 feature
//! 測自己（見 `Cargo.toml`）。

mod config;
mod connection;
mod listener;
mod script;

pub use config::{FakeHerdrConfig, MethodResponse, SubscribeMatcher, SubscribeRule};
pub use listener::FakeHerdr;
pub use script::Step;
