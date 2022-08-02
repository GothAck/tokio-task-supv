use anyhow::Result;
use tokio::select;
use tracing::info;

use crate::supv::TaskSupvRef;

pub async fn run(supv_ref: TaskSupvRef) -> Result<()> {
    loop {
        select! {
            _ = supv_ref.cancelled() => {
                info!("Cancelled");
                break;
            }
        }
    }
    Ok(())
}
