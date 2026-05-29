use crate::command::Command;
use crate::store::Store;

/// Typed result of executing a command.
///
/// The executor returns the *kind* of reply instead of a pre-formatted string,
/// so the protocol layer can encode it correctly (RESP vs plain text) without
/// having to guess the type from the value's content.
#[derive(Debug, PartialEq, Eq)]
pub enum Reply {
    Ok,
    Pong,
    Nil,
    Integer(i64),
    Bulk(String),
}

pub struct Executor;

impl Executor {
    /// Executes a read-only command against a shared (`&Store`) reference.
    ///
    /// Splitting reads out lets the server hold only a read lock for these,
    /// allowing concurrent reads instead of serializing every command.
    pub fn execute_read(store: &Store, command: &Command) -> Reply {
        match command {
            Command::Get { key } => match store.get(key) {
                Some(value) => Reply::Bulk(value.clone()),
                None => Reply::Nil,
            },

            Command::Exists { key } => Reply::Integer(if store.exists(key) { 1 } else { 0 }),

            Command::Ttl { key } => Reply::Integer(store.ttl(key)),

            Command::Ping => Reply::Pong,

            _ => unreachable!("execute_read called with a non-read command"),
        }
    }

    pub fn execute(store: &mut Store, command: Command) -> Reply {
        match command {
            Command::Set { key, value } => {
                store.set(key, value);
                Reply::Ok
            }

            Command::SetEx {
                key,
                seconds,
                value,
            } => {
                store.set_ex(key, value, seconds);
                Reply::Ok
            }

            Command::Get { key } => match store.get(&key) {
                Some(value) => Reply::Bulk(value.clone()),
                None => Reply::Nil,
            },

            Command::Del { key } => Reply::Integer(if store.del(&key) { 1 } else { 0 }),

            Command::Exists { key } => Reply::Integer(if store.exists(&key) { 1 } else { 0 }),

            Command::Expire { key, seconds } => {
                Reply::Integer(if store.expire(&key, seconds) { 1 } else { 0 })
            }

            Command::Ttl { key } => Reply::Integer(store.ttl(&key)),

            Command::Ping => Reply::Pong,

            // These are intercepted by the command handler before reaching the
            // executor; the arms exist only to keep the match exhaustive.
            Command::AofRewrite => Reply::Ok,

            Command::Info => Reply::Ok,

            Command::FlushAll => Reply::Ok,
        }
    }
}
