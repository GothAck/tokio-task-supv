use std::sync::{atomic::AtomicU64, Arc};

use static_init::dynamic;

#[dynamic]
pub(super) static TASK_SUPV_ID_GEN: TaskSupvIdGen = TaskSupvIdGen(Arc::new(AtomicU64::new(0)));

#[derive(Clone)]
#[repr(transparent)]
pub(super) struct TaskSupvIdGen(Arc<AtomicU64>);

mod impls {
    use std::sync::atomic::Ordering;

    use super::*;

    impl TaskSupvIdGen {
        pub(crate) fn get() -> Self {
            TASK_SUPV_ID_GEN.clone()
        }

        pub(crate) fn next(&self) -> u64 {
            self.0.fetch_add(1, Ordering::Relaxed)
        }
    }
}
