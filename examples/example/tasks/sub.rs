use std::time::Duration;

use tokio_task_supv::{common::LockedSpawnData, SupvTask};


#[derive(SupvTask)]
#[task()]
pub struct SubTask {}

#[derive(SupvTask)]
#[task(run_args(duration, supv_ref))]
#[task(spawn_name_extra(self.spawn_name_extra_impl()))]
pub struct TimedTask {
    millis: u64,
    #[task(spawn(let duration = self.duration.lock_take().await?))]
    duration: LockedSpawnData<Duration>,
}

mod impls {
    use std::sync::Arc;

    use anyhow::Result;
    use tokio::{select, time::{timeout, interval}};
    use tokio_task_supv::supv::TaskSupvRef;
    use rand::Rng;
    use tracing::info;

    use super::*;

    impl SubTask {
        pub fn new() -> Arc<Self> {
            Arc::new(Self {})
        }

        pub(super) async fn run(self: Arc<Self>, supv_ref: TaskSupvRef) -> Result<()> {
            let mut interval = interval(Duration::from_millis(1000));

            loop {
                select! {
                    _ = supv_ref.cancelled() => {
                        break;
                    }
                    _ = interval.tick() => {
                        let millis = (rand::thread_rng().gen::<f32>() * 10000.0) as u64;
                        let timed_task = TimedTask::new(millis);
                        supv_ref.spawn(&timed_task, ()).await?;
                    }
                }
            }

            Ok(())
        }
    }

    impl TimedTask {
        pub fn new(millis: u64) -> Arc<Self> {
            Arc::new(Self {
                millis,
                duration: LockedSpawnData::new(Duration::from_millis(millis)),
            })
        }

        #[inline]
        pub(super) fn spawn_name_extra_impl(&self) -> Option<String> {
            Some(format!("millis={}", self.millis))
        }

        pub(super) async fn run(self: Arc<Self>, duration: Duration, supv_ref: TaskSupvRef) -> Result<()> {
            select! {
                res = timeout(duration, supv_ref.cancelled()) => {
                    match res {
                        Ok(_) => {
                            info!("Cancelled");
                        }
                        Err(_) => {
                            info!("Timeout");
                        }
                    }
                }
            }

            Ok(())
        }
    }
}
