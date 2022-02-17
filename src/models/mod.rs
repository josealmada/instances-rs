use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub enum InstanceRole {
    Leader,
    Follower,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstanceInfo<T> where T: Serialize + DeserializeOwned {
    pub id: Uuid,
    pub role: InstanceRole,
    #[serde(deserialize_with = "T::deserialize")]
    pub data: T,
}