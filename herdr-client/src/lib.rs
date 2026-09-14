//! HERDR observer client（change 1a）。
//!
//! 只懂 HERDR 協定的 observer 子集：transport（`connector`）、型別（`types`）、
//! request／subscribe（`client`，task 4.x 加入）。不依賴任何 `cockpit-*` crate（ADR-0003）。

pub mod client;
pub mod connector;
#[cfg(feature = "test-support")]
pub mod testing;
pub mod types;
