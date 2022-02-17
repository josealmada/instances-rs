mod memory;

use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;

use crate::models::InstanceInfo;

pub trait Backend<T> where T: Serialize + DeserializeOwned {
    fn update_instance_info(&self, info: InstanceInfo<T>);
    fn get_instance_info(&self) -> InstanceInfo<T>;
    fn instances_count(&self) -> usize;
    fn list_active_instances(&self) -> Vec<Box<InstanceInfo<T>>>;
}

#[derive(Debug)]
pub enum BackendType {
    Memory,
    #[cfg(feature = "backend-mysql")]
    MySQL,
    #[cfg(feature = "backend-dynamodb")]
    DynamoDB,
    #[cfg(feature = "backend-redis")]
    Redis,
}

#[derive(Error, Debug)]
pub enum BackendError {
    #[error(r#"Backend implementation '{0}' not found. The avaliable options are: Memory, MySQL (feature = "backend-mysql"), DynamoDB (feature = "backend-dynamodb") or Redis (feature = "backend-redis")."#)]
    BackendNotFound(String)
}

impl Display for BackendType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            BackendType::Memory => f.write_str("Memory"),
            #[cfg(feature = "backend-mysql")]
            BackendType::MySQL => f.write_str("MySQL"),
            #[cfg(feature = "backend-dynamodb")]
            BackendType::DynamoDB => f.write_str("DynamoDB"),
            #[cfg(feature = "backend-redis")]
            BackendType::Redis => f.write_str("Redis"),
        }
    }
}

impl FromStr for BackendType {
    type Err = BackendError;

    fn from_str(s: &str) -> Result<BackendType, BackendError> {
        match s.to_lowercase().as_ref() {
            "memory" => Ok(BackendType::Memory),
            #[cfg(feature = "backend-mysql")]
            "mysql" => Ok(BackendType::MySQL),
            #[cfg(feature = "backend-dynamodb")]
            "dynamodb" => Ok(BackendType::DynamoDB),
            #[cfg(feature = "backend-redis")]
            "redis" => Ok(BackendType::Redis),
            _ => Err(BackendError::BackendNotFound(s.to_owned())),
        }
    }
}