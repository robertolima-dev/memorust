use crate::command::Command;
use crate::store::Store;

pub struct Executor;

impl Executor {
    pub fn execute(store: &mut Store, command: Command) -> String {
        match command {
            Command::Set { key, value } => {
                store.set(key, value);
                "OK".to_string()
            }

            Command::SetEx {
                key,
                seconds,
                value,
            } => {
                store.set_ex(key, value, seconds);
                "OK".to_string()
            }

            Command::Get { key } => match store.get(&key) {
                Some(value) => value.clone(),
                None => "NIL".to_string(),
            },

            Command::Del { key } => {
                if store.del(&key) {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }

            Command::Exists { key } => {
                if store.exists(&key) {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }

            Command::Expire { key, seconds } => {
                if store.expire(&key, seconds) {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }

            Command::Ttl { key } => store.ttl(&key).to_string(),

            Command::Ping => "PONG".to_string(),

            Command::AofRewrite => "AOFREWRITE".to_string(),

            Command::Info => "INFO".to_string(),

            Command::FlushAll => "FLUSHALL".to_string(),
        }
    }
}
