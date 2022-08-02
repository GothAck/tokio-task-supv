pub mod common;
pub mod dummy;
pub mod handle;
pub mod supv;
pub mod stream;

use std::{fmt, sync::Arc};

use anyhow::Result;
pub use async_trait::async_trait;
use tokio::task::JoinHandle;
pub use tokio_util::sync::CancellationToken;
use tracing::Span;

pub use tokio_task_supv_macros::SupvTask;

use self::{handle::TaskExitPolicy, supv::TaskSupvRef};

#[async_trait]
pub trait SupvTask: Sized {
    const NAME: &'static str;
    const POLICY: TaskExitPolicy = TaskExitPolicy::CancelChildren;
    type Arg: Send;
    type Output: fmt::Debug;

    async fn spawn(
        self: &Arc<Self>,
        arg: Self::Arg,
        supv_ref: TaskSupvRef,
        span: Span,
    ) -> Result<JoinHandle<Self::Output>>;

    fn spawn_name_extra(&self) -> Option<String> {
        None
    }

    fn spawn_name(&self) -> String {
        if let Some(extra) = self.spawn_name_extra() {
            format!("{}<{}>", Self::NAME, extra)
        } else {
            Self::NAME.to_string()
        }
    }
}
