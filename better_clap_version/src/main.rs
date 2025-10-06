//! When developing CLI tools, we have gotten used to querying version information
//! with command like `cli_tool -V`.
//!
//! And many tool will show its commit hash.
//! e.g. vscode:
//! ```ignore
//! code --version
//! 1.102.3
//! 488a1f239235055e34e673291fb8d8c810886f81
//! arm64
//! ```
//! This is helpful for maintainer to locate bug.
//!
//! But how can we implement this in Rust? Let me tell you.

#![allow(unused)]

use clap::Parser;

fn main() {
    let _ = Cli::parse();
}

/// Run `cargo run -F not_good -- -V`,
/// got `better_clap_version 0.1.0`.
///
/// This lacks of commit tag and commit hash.
#[cfg(feature = "not_good")]
#[derive(Debug, Parser)]
#[command(version)]
struct Cli {}

/// Run `cargo run -F not_best -- -V`,
/// got `better_clap_version v0.1.0 a11569b`.
///
/// But the commit hash is hard coded, and it's impossible for maintainers to update it
/// in nightly building without mistake.
#[cfg(feature = "not_best")]
#[derive(Debug, Parser)]
#[command(version = "v0.1.0 a11569b")]
struct Cli {}

/// This time we add the following to `Cargo.toml`:
/// ```ignore
/// [build-dependencies]
/// anyhow = "1.0.100"
/// vergen = { version = "9.0.6", features = ["rustc"] }
/// vergen-git2 = "1.0.7"
/// ```
/// Then add the following to `build.rs`:
/// ```ignore
/// use anyhow::Result;
/// use vergen::{Emitter, RustcBuilder};
/// use vergen_git2::Git2Builder;
///
/// fn main() -> Result<()> {
///     let rustc = RustcBuilder::default()
///         .host_triple(true)
///         .semver(true)
///         .build()?;
///     let git = Git2Builder::default().describe(true, true, None).build()?;
///     Emitter::default()
///         .add_instructions(&rustc)?
///         .add_instructions(&git)?
///         .emit()?;
///     Ok(())
/// }
/// ```
/// Then we can build const str with
/// ```ignore
/// concat!(
///     "\nversion: ",
///     env!("CARGO_PKG_VERSION"),
///     " ",
///     env!("VERGEN_GIT_DESCRIBE"),
///     "\nrustc: ",
///     env!("VERGEN_RUSTC_SEMVER"),
///     " ",
///     env!("VERGEN_RUSTC_HOST_TRIPLE"),
/// )
/// ```
/// to be used as version information.
///
/// Now run `cargo run -- -V`,
/// got
/// ```ignore
/// better_clap_version
/// version: 0.1.0 a11569b-dirty
/// rustc: 1.90.0 aarch64-apple-darwin
/// ```
/// Run `cargo +nightly run -- -V`,
/// got `... rustc: 1.92.0-nightly aarch64-apple-darwin`
///
/// After `git commit` and `git tag v0.1.0`,
/// run again, got `version: 0.1.0 v0.1.0 ...`
#[cfg(not(any(feature = "not_good", feature = "not_best")))]
#[derive(Debug, Parser)]
#[command(version = concat!(
    "\nversion: ",
    env!("CARGO_PKG_VERSION"),
    " ",
    env!("VERGEN_GIT_DESCRIBE"),
    "\nrustc: ",
    env!("VERGEN_RUSTC_SEMVER"),
    " ",
    env!("VERGEN_RUSTC_HOST_TRIPLE"),
))]
struct Cli {}
