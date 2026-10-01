//! HERDR runtime adapter 骨架：串接 `cockpit-core` 的 Runtime 型別與 `herdr-client`。

pub mod ansi;
pub mod factory;
pub mod probe;
pub mod runtime;
pub mod translate;

pub use factory::{BuildError, BuildOptions, BuiltRuntime, HerdrEndpoint, build};
pub use runtime::HerdrRuntime;
