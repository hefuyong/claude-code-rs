//! Binary entry point for Claude Code RS.

use cc_error::CcResult;

#[tokio::main]
async fn main() -> CcResult<()> {
    cc_cli::run().await
}
