extern crate core;

use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::backends::{Backend, ConnectionError};
use crate::daemon::UpdateDaemon;
use crate::models::{CommunicationErrorStrategy, InstanceInfo, InstanceRole, LeaderStrategy};
use crate::InstanceRole::{Follower, Leader, Unknown};

pub mod backends;
pub mod config;
pub mod daemon;
pub mod models;

pub struct Instances<B, T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
    B: Backend<T> + Send + Sync + 'static,
{
    instance_id: Uuid,
    backend: Arc<B>,
    info_extractor: fn() -> T,
    leader_strategy: LeaderStrategy,
    error_strategy: CommunicationErrorStrategy,

    state: Arc<RwLock<InstancesState<T>>>,

    daemon: Arc<Mutex<Option<UpdateDaemon>>>,
}

struct InstancesState<T>
where
    T: Serialize + DeserializeOwned + Clone + 'static,
{
    current_info: Option<Arc<InstanceInfo<T>>>,
    instances: Arc<Vec<InstanceInfo<T>>>,
}

impl<B, T> Instances<B, T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
    B: Backend<T> + Send + Sync + 'static,
{
    pub fn get_instance_info(&self) -> Option<Arc<InstanceInfo<T>>> {
        let guard = self.state.read().unwrap();
        guard.current_info.as_ref().cloned()
    }

    pub fn instances_count(&self) -> usize {
        let guard = self.state.read().unwrap();
        guard.instances.len()
    }

    pub fn list_active_instances(&self) -> Arc<Vec<InstanceInfo<T>>> {
        let guard = self.state.read().unwrap();
        guard.instances.clone()
    }

    pub fn wait_for_first_update(&self, duration: Duration) -> Result<(), InstancesError> {
        let end = Instant::now() + duration;
        while Instant::now() < end && self.get_instance_info().is_none() {
            thread::sleep(Duration::from_millis(5));
        }
        if Instant::now() < end {
            Ok(())
        } else {
            Err(InstancesError::Timeout)
        }
    }

    fn update_instance_info(&self) -> Result<(), ConnectionError> {
        let data = (self.info_extractor)();
        let instances = self.update_instance_info_and_retrieve(data);

        match instances {
            Ok(instances) => {
                let instances = self.add_leadership(instances);

                let current =
                    (*instances.iter().find(|i| i.id == self.instance_id).unwrap()).clone();

                *self.state.write().unwrap() = InstancesState {
                    instances: Arc::new(instances),
                    current_info: Some(Arc::new(current)),
                };
                Ok(())
            }
            Err(error) => match self.error_strategy {
                CommunicationErrorStrategy::Error => Err(error),
                CommunicationErrorStrategy::UseLastInfo => Ok(()),
            },
        }
    }

    fn update_instance_info_and_retrieve(
        &self,
        data: T,
    ) -> Result<Vec<(Uuid, SystemTime, T)>, ConnectionError> {
        self.backend.update_instance_info(self.instance_id, data)?;
        self.backend.list_active_instances()
    }

    fn add_leadership(&self, mut instances: Vec<(Uuid, SystemTime, T)>) -> Vec<InstanceInfo<T>> {
        let leader = match self.leader_strategy {
            LeaderStrategy::None => None,
            LeaderStrategy::Oldest => instances.iter().min_by_key(|i| i.1),
            LeaderStrategy::Newest => instances.iter().max_by_key(|i| i.1),
        }
        .map(|v| v.0);

        let mut result = Vec::with_capacity(instances.len());

        while let Some(i) = instances.pop() {
            result.push(InstanceInfo {
                id: i.0,
                role: self.check_leader(&leader, &i.0),
                data: i.2,
            })
        }

        result
    }

    fn check_leader(&self, leader: &Option<Uuid>, current: &Uuid) -> InstanceRole {
        match self.leader_strategy {
            LeaderStrategy::None => Unknown,
            _ => {
                if *leader == Some(*current) {
                    Leader
                } else {
                    Follower
                }
            }
        }
    }
}

#[derive(Error, PartialEq, Debug)]
pub enum InstancesError {
    #[error(r#"BacTimeout waiting for the first update."#)]
    Timeout,
}

#[cfg(test)]
mod tests {
    use std::ops::{Add, Deref};
    use std::time::Duration;

    use mockall::predicate::eq;

    use crate::backends::MockBackend;

    use super::*;

    #[test]
    fn should_not_return_any_info_before_any_update() {
        let backend = MockBackend::<String>::new();

        let instance = Instances {
            instance_id: Uuid::new_v4(),
            backend: Arc::new(backend),
            info_extractor: || "data".to_string(),
            leader_strategy: LeaderStrategy::None,
            error_strategy: CommunicationErrorStrategy::Error,
            state: new_state(),
            daemon: Arc::new(Mutex::new(None)),
        };

        assert!(instance.get_instance_info().is_none());
        assert_eq!(0, instance.instances_count());
        assert_eq!(0, instance.list_active_instances().len());
    }

    #[test]
    fn should_return_info_after_update_success() {
        let mut backend = MockBackend::<String>::new();
        let id = Uuid::new_v4();

        backend
            .expect_update_instance_info()
            .with(eq(id), eq("data".to_string()))
            .times(1)
            .returning(|_, _| Ok(()));

        backend
            .expect_list_active_instances()
            .times(1)
            .returning(move || Ok(vec![(id, SystemTime::now(), "data".to_string())]));

        let instance = Instances {
            instance_id: id,
            backend: Arc::new(backend),
            info_extractor: || "data".to_string(),
            leader_strategy: LeaderStrategy::None,
            error_strategy: CommunicationErrorStrategy::Error,
            state: new_state(),
            daemon: Arc::new(Mutex::new(None)),
        };

        instance.update_instance_info().unwrap();

        validate(instance.get_instance_info(), id, Unknown);

        assert_eq!(1, instance.instances_count());

        let instances = instance.list_active_instances();
        let single_instance = instances.deref().first().unwrap();
        assert_eq!(id, single_instance.id);
        assert_eq!(Unknown, single_instance.role);
        assert_eq!("data".to_string(), single_instance.data);
    }

    #[test]
    fn should_correctly_select_leader_when_disabled() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let data = mock_data_for(vec![id1, id2, id3]);

        let instance = instance_service_for(LeaderStrategy::None);

        let result = instance.add_leadership(data);

        assert_eq!(Unknown, result.iter().find(|i| i.id == id1).unwrap().role);
        assert_eq!(Unknown, result.iter().find(|i| i.id == id2).unwrap().role);
        assert_eq!(Unknown, result.iter().find(|i| i.id == id3).unwrap().role);
    }

    #[test]
    fn should_correctly_select_leader_for_newest() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let data = mock_data_for(vec![id1, id2, id3]);

        let instance = instance_service_for(LeaderStrategy::Newest);

        let result = instance.add_leadership(data);

        assert_eq!(Follower, result.iter().find(|i| i.id == id1).unwrap().role);
        assert_eq!(Follower, result.iter().find(|i| i.id == id2).unwrap().role);
        assert_eq!(Leader, result.iter().find(|i| i.id == id3).unwrap().role);
    }

    #[test]
    fn should_correctly_select_leader_for_oldest() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let data = mock_data_for(vec![id1, id2, id3]);

        let instance = instance_service_for(LeaderStrategy::Oldest);

        let result = instance.add_leadership(data);

        assert_eq!(Leader, result.iter().find(|i| i.id == id1).unwrap().role);
        assert_eq!(Follower, result.iter().find(|i| i.id == id2).unwrap().role);
        assert_eq!(Follower, result.iter().find(|i| i.id == id3).unwrap().role);
    }

    #[test]
    fn should_return_old_info_after_update_failure() {
        let mut backend = MockBackend::<String>::new();
        let id = Uuid::new_v4();

        backend
            .expect_update_instance_info()
            .with(eq(id), eq("data".to_string()))
            .times(1)
            .returning(|_, _| Ok(()));

        backend
            .expect_update_instance_info()
            .with(eq(id), eq("data".to_string()))
            .times(1)
            .returning(|_, _| Err(ConnectionError::FailedToUpdate("error".to_string())));

        backend
            .expect_list_active_instances()
            .times(1)
            .returning(move || Ok(vec![(id, SystemTime::now(), "data".to_string())]));

        let instance = Instances {
            instance_id: id,
            backend: Arc::new(backend),
            info_extractor: || "data".to_string(),
            leader_strategy: LeaderStrategy::None,
            error_strategy: CommunicationErrorStrategy::UseLastInfo,
            state: new_state(),
            daemon: Arc::new(Mutex::new(None)),
        };

        instance.update_instance_info().unwrap();

        validate(instance.get_instance_info(), id, Unknown);

        instance.update_instance_info().unwrap();

        validate(instance.get_instance_info(), id, Unknown);
    }

    #[test]
    fn should_return_error_after_update_failure() {
        let mut backend = MockBackend::<String>::new();
        let id = Uuid::new_v4();

        backend
            .expect_update_instance_info()
            .with(eq(id), eq("data".to_string()))
            .times(1)
            .returning(|_, _| Ok(()));

        backend
            .expect_update_instance_info()
            .with(eq(id), eq("data".to_string()))
            .times(1)
            .returning(|_, _| Err(ConnectionError::FailedToUpdate("error".to_string())));

        backend
            .expect_list_active_instances()
            .times(1)
            .returning(move || Ok(vec![(id, SystemTime::now(), "data".to_string())]));

        let instance = Instances {
            instance_id: id,
            backend: Arc::new(backend),
            info_extractor: || "data".to_string(),
            leader_strategy: LeaderStrategy::None,
            error_strategy: CommunicationErrorStrategy::Error,
            state: new_state(),
            daemon: Arc::new(Mutex::new(None)),
        };

        instance.update_instance_info().unwrap();

        validate(instance.get_instance_info(), id, Unknown);

        let result = instance.update_instance_info();

        assert_eq!(
            Err(ConnectionError::FailedToUpdate("error".to_string())),
            result
        )
    }

    #[test]
    fn should_fail_waiting_on_timeout() {
        let backend = MockBackend::<String>::new();

        let instance = Instances {
            instance_id: Uuid::new_v4(),
            backend: Arc::new(backend),
            info_extractor: || "data".to_string(),
            leader_strategy: LeaderStrategy::None,
            error_strategy: CommunicationErrorStrategy::Error,
            state: new_state(),
            daemon: Arc::new(Mutex::new(None)),
        };

        assert!(instance
            .wait_for_first_update(Duration::from_millis(5))
            .is_err());
        assert!(instance.get_instance_info().is_none());
    }

    #[test]
    fn should_resume_after_first_update() {
        let mut backend = MockBackend::<String>::new();
        let id = Uuid::new_v4();

        backend
            .expect_update_instance_info()
            .with(eq(id), eq("data".to_string()))
            .times(1)
            .returning(|_, _| Ok(()));

        backend
            .expect_list_active_instances()
            .times(1)
            .returning(move || Ok(vec![(id, SystemTime::now(), "data".to_string())]));

        let instance = Instances {
            instance_id: id,
            backend: Arc::new(backend),
            info_extractor: || "data".to_string(),
            leader_strategy: LeaderStrategy::None,
            error_strategy: CommunicationErrorStrategy::Error,
            state: new_state(),
            daemon: Arc::new(Mutex::new(None)),
        };

        assert!(instance
            .wait_for_first_update(Duration::from_millis(5))
            .is_err());
        assert!(instance.get_instance_info().is_none());

        instance.update_instance_info().unwrap();

        assert!(instance
            .wait_for_first_update(Duration::from_millis(5))
            .is_ok());
        assert!(instance.get_instance_info().is_some());
    }

    fn new_state() -> Arc<RwLock<InstancesState<String>>> {
        Arc::new(RwLock::new(InstancesState {
            current_info: None,
            instances: Arc::new(Vec::new()),
        }))
    }

    fn mock_data_for(ids: Vec<Uuid>) -> Vec<(Uuid, SystemTime, String)> {
        ids.iter()
            .enumerate()
            .map(|(i, id)| {
                (
                    *id,
                    SystemTime::now().add(Duration::from_secs(i as u64)),
                    "data".to_string(),
                )
            })
            .collect()
    }

    fn validate(info: Option<Arc<InstanceInfo<String>>>, id: Uuid, role: InstanceRole) {
        match info {
            None => panic!("Should return a valid value"),
            Some(current) => {
                assert_eq!(id, current.id);
                assert_eq!(role, current.role);
                assert_eq!("data".to_string(), current.data);
            }
        }
    }

    fn instance_service_for(
        leader_strategy: LeaderStrategy,
    ) -> Instances<MockBackend<String>, String> {
        Instances {
            instance_id: Uuid::new_v4(),
            backend: Arc::new(MockBackend::<String>::new()),
            info_extractor: || "data".to_string(),
            leader_strategy,
            error_strategy: CommunicationErrorStrategy::Error,
            state: new_state(),
            daemon: Arc::new(Mutex::new(None)),
        }
    }
}
