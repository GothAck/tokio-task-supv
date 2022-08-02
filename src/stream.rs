pub use async_trait::async_trait;
use futures::stream::FuturesUnordered;
pub use tokio_util::sync::CancellationToken;

use crate::handle::{TaskHandle, TaskHandleOut};

pub struct TaskHandles<T> {
    policy: TaskHandlesPolicy<T>,
    cancel_next: bool,
    futures: FuturesUnordered<TaskHandle<T>>,
}

type TaskCancellationOn<T> = fn(&T) -> bool;

pub enum TaskHandlesPolicy<T> {
    Never,
    OnAnyExitOverrideTaskExitPolicy,
    OnPanic,
    On(TaskCancellationOn<TaskHandleOut<T>>),
}

mod impls {
    use std::{mem, pin, task};

    use futures::{Stream, StreamExt};
    use futures::stream::futures_unordered;
    // use tracing::{info, error, instrument};

    use super::*;

    impl<T> TaskHandles<T> {
        pub fn new(policy: TaskHandlesPolicy<T>) -> Self {
            Self {
                policy,
                cancel_next: true,
                futures: FuturesUnordered::new(),
            }
        }

        pub fn len(&self) -> usize {
            self.futures.len()
        }

        pub fn is_empty(&self) -> bool {
            self.futures.is_empty()
        }

        pub fn push(&mut self, future: TaskHandle<T>) {
            self.futures.push(future)
        }

        pub fn cancel(&mut self) {
            self.futures.iter().for_each(TaskHandle::cancel);
            self.cancel_next = false;
        }

        pub async fn next_no_cancel(&mut self) -> Option<TaskHandleOut<T>> {
            self.cancel_next = false;
            self.next().await
        }

        pub async fn next_with_cancel(&mut self) -> Option<TaskHandleOut<T>> {
            self.cancel_next = true;
            self.next().await
        }

        pub async fn collect_no_cancel<I>(mut self) -> I
        where
            I: Default + Extend<TaskHandleOut<T>>,
        {
            self.cancel_next = false;
            self.collect().await
        }

        pub async fn collect_with_cancel<I>(mut self) -> I
        where
            I: Default + Extend<TaskHandleOut<T>>,
        {
            self.cancel_next = true;
            self.collect().await
        }

        pub fn iter(&self) -> futures_unordered::Iter<TaskHandle<T>> {
            self.futures.iter()
        }

        pub async fn all_no_cancel<I>(&mut self) -> I
        where
            I: Default + Extend<TaskHandleOut<T>>,
        {
            let mut this = Self::new(self.policy);
            mem::swap(self, &mut this);
            this.collect_no_cancel().await
        }

        fn cancel_if_should(&mut self, next: &TaskHandleOut<T>) {
            if self.cancel_next && self.should_cancel(next) {
                self.cancel();
            }
            next.cancel_if_should();
        }

        fn should_cancel(&self, next: &TaskHandleOut<T>) -> bool {
            use TaskHandlesPolicy::*;
            match &self.policy {
                Never => false,
                OnAnyExitOverrideTaskExitPolicy => true,
                OnPanic => {
                    if let Err(e) = &next.result {
                        e.is_panic()
                    } else {
                        false
                    }
                }
                On(op) => op(next),
            }
        }
    }

    impl<T> From<TaskHandle<T>> for TaskHandles<T> {
        fn from(handle: TaskHandle<T>) -> Self {
            let mut this = Self::new(TaskHandlesPolicy::OnAnyExitOverrideTaskExitPolicy);
            this.push(handle);
            this
        }
    }

    impl<T> Clone for TaskHandlesPolicy<T> {
        fn clone(&self) -> Self {
            match self {
                Self::Never => Self::Never,
                Self::OnAnyExitOverrideTaskExitPolicy => Self::OnAnyExitOverrideTaskExitPolicy,
                Self::OnPanic => Self::OnPanic,
                Self::On(op) => Self::On(*op),
            }
        }
    }

    impl<T> Copy for TaskHandlesPolicy<T> {}

    impl<T> Stream for TaskHandles<T> {
        type Item = TaskHandleOut<T>;

        fn poll_next(
            mut self: pin::Pin<&mut Self>,
            cx: &mut task::Context<'_>,
        ) -> task::Poll<Option<Self::Item>> {
            use task::Poll::*;
            match self.futures.poll_next_unpin(cx) {
                Pending => Pending,
                Ready(next) => {
                    if let Some(next) = &next {
                        self.cancel_if_should(next)
                    }
                    Ready(next)
                }
            }
        }
    }
}
