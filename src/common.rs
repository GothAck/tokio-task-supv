use tokio::sync::Mutex;

pub struct LockedSpawnData<T>(Mutex<Option<T>>);

mod impls {
    use anyhow::{anyhow, Result};
    use tokio::sync::MutexGuard;

    use super::*;

    impl<T> LockedSpawnData<T> {
        pub fn new(data: T) -> Self {
            Self(Mutex::new(Some(data)))
        }

        pub async fn lock(&self) -> MutexGuard<'_, Option<T>> {
            self.0.lock().await
        }

        pub fn blocking_lock(&self) -> MutexGuard<'_, Option<T>> {
            self.0.blocking_lock()
        }

        pub async fn lock_take(&self) -> Result<T> {
            self.0
                .lock()
                .await
                .take()
                .ok_or_else(|| anyhow!("Spawn data already taken"))
        }

        pub fn blocking_lock_take(&self) -> Result<T> {
            self.0
                .blocking_lock()
                .take()
                .ok_or_else(|| anyhow!("Spawn data already taken"))
        }
    }
}
