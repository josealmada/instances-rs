use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use uuid::Uuid;

use instances_rs::backends::{Backend, ConnectionError};
use instances_rs::config::Builder;
use instances_rs::models::InstanceRole;

#[derive(Default)]
struct InMemoryBackend {
    instance_id: Arc<Mutex<Option<Uuid>>>,
    data: Arc<Mutex<Option<String>>>,
}

impl Backend<String> for InMemoryBackend {
    fn update_instance_info(&self, instance_id: Uuid, data: String) -> Result<(), ConnectionError> {
        *self.instance_id.lock().unwrap() = Some(instance_id);
        *self.data.lock().unwrap() = Some(data);
        Ok(())
    }

    fn list_active_instances(&self) -> Result<Vec<(Uuid, SystemTime, String)>, ConnectionError> {
        let instance_id = (*self.instance_id.lock().unwrap()).unwrap();
        let data = self.data.lock().unwrap().clone().unwrap();
        Ok(vec![(instance_id, SystemTime::now(), data)])
    }
}

#[test]
fn test_api_usage() {
    let instances_rs = Builder::default()
        .with_update_interval(Duration::from_millis(5))
        .with_backend(InMemoryBackend::default())
        .with_info_extractor(|| "test".to_string())
        .build();

    instances_rs
        .wait_for_first_update(Duration::from_millis(50))
        .unwrap();

    assert_eq!(1, instances_rs.instances_count());
    assert_eq!(
        "test".to_string(),
        instances_rs.get_instance_info().unwrap().data
    );
    assert_eq!(
        InstanceRole::Unknown,
        instances_rs.get_instance_info().unwrap().role
    );
}
