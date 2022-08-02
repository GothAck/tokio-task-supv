use std::sync::Arc;

pub use async_trait::async_trait;
use tokio::task::{JoinError, JoinHandle};
pub use tokio_util::sync::CancellationToken;

use super::stream::TaskHandles;

#[derive(Clone, Copy)]
pub struct TaskId {
    pub(crate) parent_id: u64,
    pub(crate) id: u64,
}

#[derive(Clone)]
pub struct TaskData {
    id: TaskId,
    name: String,
    policy: TaskExitPolicy,
    ct: TaskHandleCancel,
}

pub struct TaskHandle<T> {
    data: Arc<TaskData>,
    handle: JoinHandle<T>,
}

#[derive(Clone)]
pub struct TaskHandleCancel {
    pub(crate) root: CancellationToken,
    pub(crate) parent: CancellationToken,
    pub(crate) siblings: CancellationToken,
    pub(crate) self_and_children: CancellationToken,
    pub(crate) children: CancellationToken,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskExitPolicy {
    Nothing,
    CancelRoot,
    CancelParent,
    CancelSiblings,
    CancelChildren,
}

type JoinHandleOut<T> = std::result::Result<T, JoinError>;

pub struct TaskHandleOut<T> {
    pub(crate) data: Arc<TaskData>,
    pub(crate) result: JoinHandleOut<T>,
}

mod impls {
    use std::{fmt, pin, task};

    use futures::{Future, FutureExt};
    // use tracing::{info, error, instrument};

    use super::*;

    impl TaskId {
        pub(crate) fn new(parent_id: u64, id: u64) -> Self {
            Self { parent_id, id }
        }

        pub fn parent_id(&self) -> u64 {
            self.parent_id
        }

        pub fn id(&self) -> u64 {
            self.id
        }
    }

    impl TaskData {
        pub fn id(&self) -> TaskId {
            self.id
        }

        pub fn name(&self) -> &str {
            &self.name
        }

        pub fn policy(&self) -> TaskExitPolicy {
            self.policy
        }

        // pub fn ct(&self) -> &TaskHandleCancel {
        //     &self.ct
        // }
    }

    impl<T> TaskHandle<T> {
        pub(crate) fn new(
            id: TaskId,
            name: &str,
            handle: JoinHandle<T>,
            policy: TaskExitPolicy,
            ct: TaskHandleCancel,
        ) -> Self {
            Self {
                data: Arc::new(TaskData {
                    id,
                    name: name.to_string(),
                    policy,
                    ct,
                 }),
                handle,
            }
        }

        pub fn data(&self) -> &Arc<TaskData> {
            &self.data
        }

        pub fn id(&self) -> TaskId {
            self.data.id
        }

        pub fn cancel(&self) {
            self.data.ct.self_and_children.cancel()
        }

        pub fn abort(&self) {
            self.handle.abort()
        }

        pub fn into_handles(self) -> TaskHandles<T> {
            self.into()
        }
    }

    impl TaskHandleCancel {
        pub(crate) fn new_root() -> Self {
            let root = CancellationToken::new();
            let parent = root.clone();
            let siblings = parent.clone();
            let self_and_children = siblings.child_token();
            let children = self_and_children.child_token();

            Self {
                root,
                parent,
                siblings,
                self_and_children,
                children,
            }
        }

        pub(crate) fn new_child(&self) -> Self {
            let root = self.root.clone();
            let parent = self.self_and_children.clone();
            let siblings = self.children.clone();
            let self_and_children = siblings.child_token();
            let children = self_and_children.child_token();

            Self {
                root,
                parent,
                siblings,
                self_and_children,
                children,
            }
        }
    }

    impl<T> TaskHandleOut<T> {
        fn new(
            data: Arc<TaskData>,
            result: JoinHandleOut<T>,
        ) -> Self {
            Self {
                data,
                result,
            }
        }

        fn from(handle: &TaskHandle<T>, result: JoinHandleOut<T>) -> Self {
            Self::new(handle.data.clone(), result)
        }

        pub(crate) fn cancel_if_should(&self) {
            let ct = &self.data.ct;

            match self.data.policy {
                TaskExitPolicy::Nothing => {}
                TaskExitPolicy::CancelRoot => ct.root.cancel(),
                TaskExitPolicy::CancelParent => ct.parent.cancel(),
                TaskExitPolicy::CancelSiblings => ct.siblings.cancel(),
                TaskExitPolicy::CancelChildren => ct.self_and_children.cancel(),
            }
        }

        pub fn data(&self) -> &Arc<TaskData> {
            &self.data
        }

        pub fn id(&self) -> TaskId {
            self.data.id
        }

        pub fn policy(&self) -> TaskExitPolicy {
            self.data.policy
        }

        pub fn result(&self) -> &JoinHandleOut<T> {
            &self.result
        }
    }

    impl<T> Future for TaskHandle<T> {
        type Output = TaskHandleOut<T>;

        fn poll(
            mut self: pin::Pin<&mut Self>,
            cx: &mut task::Context<'_>,
        ) -> task::Poll<Self::Output> {
            use task::Poll::*;
            match self.handle.poll_unpin(cx) {
                Pending => Pending,
                Ready(result) => Ready(TaskHandleOut::from(&self, result)),
            }
        }
    }

    impl fmt::Debug for TaskId {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_fmt(format_args!("{} [p {}]", self.id, self.parent_id))
        }
    }

    impl fmt::Display for TaskId {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.id.fmt(f)
        }
    }

    impl fmt::Debug for TaskData {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("")
                .field("id", &self.id.to_string())
                .field("name", &self.name)
                .field("policy", &self.policy)
                .finish_non_exhaustive()
        }
    }

    impl fmt::Display for TaskData {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_fmt(format_args!("{:?}@{}", self.name, self.id))
        }
    }

    impl<T> fmt::Display for TaskHandle<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.data.fmt(f)
        }
    }

    impl<T> fmt::Display for TaskHandleOut<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.data.fmt(f)
        }
    }
}
