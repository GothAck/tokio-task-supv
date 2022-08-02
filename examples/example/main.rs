mod logging;
mod tasks;

use std::time::Duration;

use anyhow::Result;

use tokio_task_supv::supv::TaskSupv;

use self::tasks::main::*;

#[tokio::main]
async fn main() -> Result<()> {
    logging::init()?;

    let main = MainTask::new();

    let mut supv = TaskSupv::new();
    supv.set_print_stats(Duration::from_secs(5));
    supv.spawn_task(&main, ()).await?;

    supv.run().await
}
