use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(PartialEq, Debug)]
pub enum LeaderStrategy {
    None,
    Oldest,
    Newest,
}

#[derive(PartialEq, Debug)]
pub enum CommunicationErrorStrategy {
    Error,
    UseLastInfo,
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub enum InstanceRole {
    Leader,
    Follower,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstanceInfo<T>
where
    T: Serialize + DeserializeOwned + Clone,
{
    pub id: Uuid,
    pub role: InstanceRole,
    #[serde(deserialize_with = "T::deserialize")]
    pub data: T,
}
