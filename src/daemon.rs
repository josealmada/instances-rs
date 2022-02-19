use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::{Backend, Instances};

pub struct UpdateDaemon {
    running: Arc<AtomicBool>,
}

pub fn start_daemon<B, T>(update_interval: Duration, service: Arc<Instances<B, T>>) -> UpdateDaemon
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
    B: Backend<T> + Send + Sync + 'static,
{
    let running = Arc::new(AtomicBool::new(true));

    spawn_daemon(update_interval, running.clone(), service);

    UpdateDaemon { running }
}

fn spawn_daemon<B, T>(
    update_interval: Duration,
    is_running: Arc<AtomicBool>,
    service: Arc<Instances<B, T>>,
) -> JoinHandle<()>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
    B: Backend<T> + Send + Sync + 'static,
{
    let ticker = crossbeam_channel::tick(update_interval);

    thread::spawn(move || {
        while is_running.fetch_and(true, Ordering::SeqCst) {
            service.update_instance_info().unwrap();
            ticker.recv().unwrap();
        }
    })
}

impl Drop for UpdateDaemon {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, RwLock};
    use std::time::SystemTime;

    use mockall::predicate::eq;
    use uuid::Uuid;

    use crate::backends::MockBackend;
    use crate::{CommunicationErrorStrategy, InstancesState, LeaderStrategy};

    use super::*;

    #[test]
    fn should_execute_the_first_update_immediately() {
        let mut backend = MockBackend::<String>::new();
        let id = Uuid::new_v4();

        backend
            .expect_update_instance_info()
            .with(eq(id), eq("data".to_string()))
            .returning(|_, _| Ok(()));

        backend
            .expect_list_active_instances()
            .returning(move || Ok(vec![(id, SystemTime::now(), "data".to_string())]));

        let instances = Arc::new(Instances {
            instance_id: id,
            backend: Arc::new(backend),
            info_extractor: || "data".to_string(),
            leader_strategy: LeaderStrategy::None,
            error_strategy: CommunicationErrorStrategy::Error,
            state: Arc::new(RwLock::new(InstancesState {
                current_info: None,
                instances: Arc::new(vec![]),
            })),
            daemon: Arc::new(Mutex::new(None)),
        });

        assert!(instances.get_instance_info().is_none());

        let _daemon = start_daemon(Duration::from_secs(5), instances.clone());
        thread::sleep(Duration::from_millis(10));

        assert!(instances.get_instance_info().is_some());
        drop(_daemon);
    }

    #[test]
    fn should_execute_the_update_5_times() {
        let mut backend = MockBackend::<String>::new();
        let id = Uuid::new_v4();

        backend
            .expect_update_instance_info()
            .with(eq(id), eq("data".to_string()))
            .times(5)
            .returning(|_, _| Ok(()));

        backend
            .expect_list_active_instances()
            .times(5)
            .returning(move || Ok(vec![(id, SystemTime::now(), "data".to_string())]));

        let instances = Arc::new(Instances {
            instance_id: id,
            backend: Arc::new(backend),
            info_extractor: || "data".to_string(),
            leader_strategy: LeaderStrategy::None,
            error_strategy: CommunicationErrorStrategy::Error,
            state: Arc::new(RwLock::new(InstancesState {
                current_info: None,
                instances: Arc::new(vec![]),
            })),
            daemon: Arc::new(Mutex::new(None)),
        });

        assert!(instances.get_instance_info().is_none());

        let _daemon = start_daemon(Duration::from_millis(50), instances.clone());
        thread::sleep(Duration::from_millis(230));
        drop(_daemon);

        assert!(instances.get_instance_info().is_some());
    }
}
