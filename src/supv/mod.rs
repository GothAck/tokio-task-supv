mod id;

use std::{time::Duration, sync::Arc};

use anyhow::Result;
pub use async_trait::async_trait;
use tokio::sync::mpsc;
pub use tokio_util::sync::CancellationToken;

use crate::{common::LockedSpawnData, handle::{TaskHandleCancel, TaskHandle, TaskId, TaskData}};

use self::id::*;

pub struct TaskSupv {
    id: u64,
    ct: TaskHandleCancel,
    shared: TaskSupvShared,
    spawn_data: LockedSpawnData<TaskSupvSpawnData>,
    event_rx: LockedSpawnData<mpsc::Receiver<TaskSupvEvent>>,

    print_stats: Option<Duration>,
}

struct TaskSupvSpawnData {
    task_handle_rx: mpsc::UnboundedReceiver<TaskHandle<Result<()>>>,
    event_tx: mpsc::Sender<TaskSupvEvent>,
}

#[derive(Clone)]
pub struct TaskSupvRef {
    id: TaskId,
    #[allow(dead_code)]
    name: String,
    name_path: Vec<String>,
    ct: TaskHandleCancel,
    shared: TaskSupvShared,
}

pub enum TaskSupvEvent {
    Spawned(Arc<TaskData>),
    Exited(Arc<TaskData>, Result<Result<()>, TaskSupvExitErr>),
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum TaskSupvExitErr {
    #[error("Cancelled")]
    Cancelled,
    #[error("Panic")]
    Panic,
    #[error("Unknown")]
    Unknown,
}

#[derive(Clone)]
struct TaskSupvShared {
    id_gen: TaskSupvIdGen,
    task_handle_tx: mpsc::UnboundedSender<TaskHandle<Result<()>>>,
}

mod impls {
    use std::{fmt, sync::Arc};

    use anyhow::{bail, Context};
    use futures::StreamExt;
    use tokio::{select, time::interval, task::JoinError};
    use tokio_stream::wrappers::ReceiverStream;
    use tracing::{error, info, info_span, instrument};

    use crate::{handle::TaskHandleOut, stream::{TaskHandles, TaskHandlesPolicy}, SupvTask};

    use super::*;

    impl TaskSupv {
        pub fn new() -> Self {
            let id_gen = TaskSupvIdGen::get();
            let id = id_gen.next();

            let (task_handle_tx, task_handle_rx) = mpsc::unbounded_channel();

            let (event_tx, event_rx) = mpsc::channel(16);

            Self {
                id,
                ct: TaskHandleCancel::new_root(),
                shared: TaskSupvShared { id_gen, task_handle_tx },
                spawn_data: LockedSpawnData::new(TaskSupvSpawnData { task_handle_rx, event_tx }),
                event_rx: LockedSpawnData::new(event_rx),
                print_stats: None,
            }
        }

        pub async fn spawn_task<T: SupvTask<Output = Result<()>>>(
            &mut self,
            task: &Arc<T>,
            arg: T::Arg,
        ) -> Result<()> {
            let name = task.spawn_name();
            let ct = self.ct.new_child();
            let supv_ref = TaskSupvRef::new(self.id, &name, &ct, &self.shared);

            let handle = spawn_task_common(task, arg, &name, supv_ref, ct).await?;

            if let Err(mpsc::error::SendError(handle)) = self.shared.task_handle_tx.send(handle) {
                handle.abort();
                bail!("tx is closed");
            }

            Ok(())
        }

        pub fn set_print_stats(&mut self, duration: Duration) {
            self.print_stats.replace(duration);
        }

        pub async fn event_stream(&self) -> Result<ReceiverStream<TaskSupvEvent>> {
            self.event_rx.lock_take().await.map(ReceiverStream::new)
        }

        #[instrument]
        pub async fn run(self) -> Result<()> {
            info!("Running");
            use tokio::signal::ctrl_c;

            let ct = self.ct.root.clone();

            let TaskSupvSpawnData { mut task_handle_rx, event_tx } = self.spawn_data.lock_take().await.with_context(|| "Already running")?;

            let mut handles = TaskHandles::new(TaskHandlesPolicy::OnPanic);

            let mut print_stats = self.print_stats.map(interval);

            while let Ok(handle) = task_handle_rx.try_recv() {
                event_tx.send(From::from(&handle)).await.ok();
                handles.push(handle);
            }

            loop {
                select! {
                    _ = ctrl_c() => {
                        info!("Quit request received");
                        ct.cancel();
                    }
                    _ = ct.cancelled(), if !ct.is_cancelled() => {
                        info!("Cancellation received");
                    }
                    res = task_handle_rx.recv() => {
                        match res {
                            None => {
                                info!("rx closed");
                                ct.cancel();
                            }
                            Some(handle) => {
                                if ct.is_cancelled() {
                                    error!("New handle {} received whilst shutting down", handle);
                                }
                                event_tx.send(From::from(&handle)).await.ok();
                                handles.push(handle);
                            }
                        }
                    }
                    res = handles.next() => {
                        match res {
                            None => {
                                info!("All tasks exited");
                                break;
                            }
                            Some(res) => {
                                match &res {
                                    TaskHandleOut { data, result: Err(e), .. } => {
                                        error!("Task '{}' join error '{}'", data.name(), e);
                                    }
                                    TaskHandleOut { data, result: Ok(Err(e)), .. } => {
                                        error!("Task '{}' return error '{}'", data.name(), e);
                                    }
                                    TaskHandleOut { data, result: Ok(Ok(())), .. } => {
                                        info!("Task '{}' exited successfully", data.name());
                                    }
                                }
                                event_tx.send(From::from(res)).await.ok();
                            }
                        }
                    }
                    _ = print_stats.as_mut().unwrap().tick(), if print_stats.is_some() => {
                        info!("Running tasks:");
                        handles.iter().for_each(|handle| info!("{:?}", handle.data()));
                        info!("End running tasks");
                    }
                }
            }

            Ok(())
        }
    }

    impl TaskSupvRef {
        fn new(parent_id: u64, name: &str, ct: &TaskHandleCancel, shared: &TaskSupvShared) -> Self {
            let id = TaskId::new(parent_id, shared.id_gen.next());

            let name_path = vec![name.to_string()];

            Self {
                id,
                name: name.to_string(),
                name_path,
                ct: ct.clone(),
                shared: shared.clone(),
            }
        }

        fn new_child(&self, name: &str) -> Self {
            let id = TaskId::new(self.id.id, self.shared.id_gen.next());

            let mut name_path = self.name_path.clone();
            name_path.push(name.to_string());

            Self {
                id,
                name: name.to_string(),
                name_path,
                ct: self.ct.new_child(),
                shared: self.shared.clone(),
            }
        }

        pub async fn spawn<T: SupvTask<Output = Result<()>>>(
            &self,
            task: &Arc<T>,
            arg: T::Arg,
        ) -> Result<()> {
            let name = task.spawn_name();
            let supv_ref = self.new_child(&name);
            let ct = supv_ref.ct.clone();

            let handle = spawn_task_common(task, arg, &name, supv_ref, ct).await?;

            if let Err(mpsc::error::SendError(handle)) = self.shared.task_handle_tx.send(handle) {
                handle.abort();
                bail!("tx is closed");
            }

            Ok(())
        }

        pub fn cancel(&self) {
            self.ct.self_and_children.cancel()
        }

        pub fn quit(&self) {
            self.ct.root.cancel()
        }

        pub fn is_cancelled(&self) -> bool {
            self.ct.self_and_children.is_cancelled()
        }

        pub async fn cancelled(&self) {
            select! {
                _ = self.ct.siblings.cancelled() => {
                    info!("siblings cancelled, cancelling self_and_children");
                    self.ct.self_and_children.cancel();
                },
                _ = self.ct.self_and_children.cancelled() => {
                    info!("self_and_children cancelled");
                },
            }
        }
    }

    impl From<&TaskHandle<Result<()>>> for TaskSupvEvent {
        fn from(value: &TaskHandle<Result<()>>) -> Self {
            Self::Spawned(value.data().clone())
        }
    }

    impl From<TaskHandleOut<Result<()>>> for TaskSupvEvent {
        fn from(value: TaskHandleOut<Result<()>>) -> Self {
            let TaskHandleOut { data, result } = value;

            Self::Exited(data, result.map_err(Into::into))
        }
    }

    impl From<JoinError> for TaskSupvExitErr {
        fn from(value: JoinError) -> Self {
            if value.is_cancelled() {
                Self::Cancelled
            } else if value.is_panic() {
                Self::Panic
            } else {
                Self::Unknown
            }
        }
    }

    async fn spawn_task_common<T: SupvTask<Output = Result<()>>>(
        task: &Arc<T>,
        arg: T::Arg,
        name: &str,
        supv_ref: TaskSupvRef,
        ct: TaskHandleCancel,
    ) -> Result<TaskHandle<Result<()>>> {
        let span = info_span!("run", task_name = name);

        let id = supv_ref.id;
        Ok(TaskHandle::new(
            id,
            name,
            task.spawn(arg, supv_ref, span).await?,
            T::POLICY,
            ct,
        ))
    }

    impl Default for TaskSupv {
        fn default() -> Self {
            Self::new()
        }
    }

    impl fmt::Debug for TaskSupv {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TaskManager").finish_non_exhaustive()
        }
    }

    impl fmt::Debug for TaskSupvRef {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TaskManagerHandle")
                .field("name", &self.name_path.join("::"))
                .finish_non_exhaustive()
        }
    }
}
