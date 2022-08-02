use std::sync::Arc;

use tokio_task_supv::SupvTask;

use crate::tasks::sub::SubTask;

#[derive(SupvTask)]
pub struct MainTask {
    #[task(spawn(task(())))]
    sub_task: Arc<SubTask>,
}

mod impls {
    use anyhow::Result;
    use tokio::select;
    use tokio_task_supv::supv::TaskSupvRef;
    use tracing::info;

    use super::*;

    impl MainTask {
        pub fn new() -> Arc<Self> {
            Arc::new(Self {
                sub_task: SubTask::new(),
            })
        }

        pub(super) async fn run(self: Arc<Self>, supv_ref: TaskSupvRef) -> Result<()> {
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
    }
}
