use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use crate::daemon::start_daemon;
use crate::{Backend, CommunicationErrorStrategy, Instances, InstancesState, LeaderStrategy};

#[derive(Default)]
pub struct Builder<B, T>
where
    T: Serialize + DeserializeOwned + Clone + 'static,
    B: Backend<T> + Send + Sync + 'static,
{
    interval: Option<Duration>,
    backend: Option<B>,
    info_extractor: Option<fn() -> T>,
    leader_strategy: Option<LeaderStrategy>,
    error_strategy: Option<CommunicationErrorStrategy>,
}

impl<B, T> Builder<B, T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
    B: Backend<T> + Send + Sync + 'static,
{
    pub fn with_update_interval(mut self, interval: Duration) -> Self {
        self.interval = Some(interval);
        self
    }

    pub fn with_backend(mut self, backend: B) -> Self {
        self.backend = Some(backend);
        self
    }

    pub fn with_info_extractor(mut self, extractor: fn() -> T) -> Self {
        self.info_extractor = Some(extractor);
        self
    }

    pub fn with_leader_strategy(mut self, strategy: LeaderStrategy) -> Self {
        self.leader_strategy = Some(strategy);
        self
    }

    pub fn with_error_strategy(mut self, strategy: CommunicationErrorStrategy) -> Self {
        self.error_strategy = Some(strategy);
        self
    }

    pub fn build(self) -> Arc<Instances<B, T>> {
        let interval = self
            .interval
            .expect("Missing required update interval configuration.");

        let service = Arc::new(Instances {
            instance_id: Uuid::new_v4(),
            backend: Arc::new(
                self.backend
                    .expect("Missing required backend configuration."),
            ),
            info_extractor: self
                .info_extractor
                .expect("Missing required info extractor configuration."),
            leader_strategy: self.leader_strategy.unwrap_or(LeaderStrategy::None),
            error_strategy: self
                .error_strategy
                .unwrap_or(CommunicationErrorStrategy::Error),

            state: Arc::new(RwLock::new(InstancesState {
                current_info: None,
                instances: Arc::new(vec![]),
            })),

            daemon: Arc::new(Mutex::new(None)),
        });

        let daemon = start_daemon(interval, service.clone());
        *service.daemon.lock().unwrap() = Some(daemon);

        service
    }
}

#[cfg(test)]
mod tests {
    use crate::backends::MockBackend;

    use super::*;

    #[test]
    #[should_panic(expected = "Missing required update interval configuration.")]
    fn should_require_a_update_interval_config() {
        let _ = Builder::default()
            .with_backend(MockBackend::new())
            .with_info_extractor(|| "data".to_string())
            .with_error_strategy(CommunicationErrorStrategy::UseLastInfo)
            .with_leader_strategy(LeaderStrategy::Oldest)
            .build();
    }

    #[test]
    #[should_panic(expected = "Missing required backend configuration.")]
    fn should_require_a_backend_config() {
        let _ = Builder::<MockBackend<String>, String>::default()
            .with_update_interval(Duration::from_secs(10))
            .with_info_extractor(|| "data".to_string())
            .with_error_strategy(CommunicationErrorStrategy::UseLastInfo)
            .with_leader_strategy(LeaderStrategy::Oldest)
            .build();
    }

    #[test]
    #[should_panic(expected = "Missing required info extractor configuration.")]
    fn should_require_an_info_extractor_config() {
        let _ = Builder::<MockBackend<String>, String>::default()
            .with_update_interval(Duration::from_secs(10))
            .with_backend(MockBackend::new())
            .with_error_strategy(CommunicationErrorStrategy::UseLastInfo)
            .with_leader_strategy(LeaderStrategy::Oldest)
            .build();
    }

    #[test]
    fn should_build_an_instance() {
        let instance = Builder::default()
            .with_update_interval(Duration::from_secs(10))
            .with_backend(MockBackend::new())
            .with_info_extractor(|| "data".to_string())
            .with_error_strategy(CommunicationErrorStrategy::UseLastInfo)
            .with_leader_strategy(LeaderStrategy::Oldest)
            .build();

        assert_eq!(
            CommunicationErrorStrategy::UseLastInfo,
            instance.error_strategy
        );
        assert_eq!(LeaderStrategy::Oldest, instance.leader_strategy);
    }

    #[test]
    fn should_build_an_instance_with_defaults() {
        let instance = Builder::default()
            .with_update_interval(Duration::from_secs(10))
            .with_backend(MockBackend::new())
            .with_info_extractor(|| "data".to_string())
            .build();

        assert_eq!(CommunicationErrorStrategy::Error, instance.error_strategy);
        assert_eq!(LeaderStrategy::None, instance.leader_strategy);
    }
}
