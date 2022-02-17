use std::sync::Mutex;

use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use crate::backends::Backend;
use crate::models::{InstanceInfo, InstanceRole};

pub struct MemoryBackend {
    id: Uuid,
    data: Mutex<Option<String>>,
}

impl<T> Backend<T> for MemoryBackend where T: Serialize + DeserializeOwned {
    fn update_instance_info(&self, info: InstanceInfo<T>) {
        let data = serde_json::to_string(&info.data).unwrap();
        *self.data.lock().unwrap() = Some(data);
    }

    fn get_instance_info(&self) -> InstanceInfo<T> {
        let holder = &self.data.lock().unwrap();
        let json = holder.as_ref().unwrap();
        let data = serde_json::from_str(json.clone().as_ref()).unwrap();
        InstanceInfo {
            id: self.id.clone(),
            role: InstanceRole::Leader,
            data,
        }
    }

    fn instances_count(&self) -> usize {
        1
    }

    fn list_active_instances(&self) -> Vec<Box<InstanceInfo<T>>> {
        if let Some(_) = self.data.lock().unwrap().as_ref() {
            vec![Box::new(self.get_instance_info())]
        } else {
            vec![]
        }
    }
}