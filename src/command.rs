use crate::error::MemorustError;
use crate::tokenizer::tokenize;

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Set {
        key: String,
        value: String,
    },
    SetEx {
        key: String,
        seconds: u64,
        value: String,
    },
    Get {
        key: String,
    },
    Del {
        key: String,
    },
    Exists {
        key: String,
    },
    Expire {
        key: String,
        seconds: u64,
    },
    Ttl {
        key: String,
    },
    Ping,
    AofRewrite,
    Info,
    FlushAll,
}

impl Command {
    /// Whether this command only reads state and can run under a shared lock.
    pub fn is_read_only(&self) -> bool {
        matches!(
            self,
            Command::Get { .. } | Command::Exists { .. } | Command::Ttl { .. } | Command::Ping
        )
    }

    pub fn parse(input: &str) -> Result<Self, MemorustError> {
        let parts = tokenize(input)?;

        if parts.is_empty() {
            return Err(MemorustError::InvalidCommand);
        }

        let command = parts[0].to_uppercase();

        match command.as_str() {
            "SET" => {
                if parts.len() < 3 {
                    return Err(MemorustError::MissingArgument);
                }

                Ok(Command::Set {
                    key: parts[1].to_string(),
                    value: parts[2..].join(" "),
                })
            }

            "SETEX" => {
                if parts.len() < 4 {
                    return Err(MemorustError::MissingArgument);
                }

                let seconds = parts[2]
                    .parse::<u64>()
                    .map_err(|_| MemorustError::InvalidSyntax)?;

                Ok(Command::SetEx {
                    key: parts[1].clone(),
                    seconds,
                    value: parts[3..].join(" "),
                })
            }

            "GET" => {
                if parts.len() < 2 {
                    return Err(MemorustError::MissingArgument);
                }

                Ok(Command::Get {
                    key: parts[1].to_string(),
                })
            }

            "DEL" => {
                if parts.len() < 2 {
                    return Err(MemorustError::MissingArgument);
                }

                Ok(Command::Del {
                    key: parts[1].to_string(),
                })
            }

            "EXISTS" => {
                if parts.len() < 2 {
                    return Err(MemorustError::MissingArgument);
                }

                Ok(Command::Exists {
                    key: parts[1].to_string(),
                })
            }

            "EXPIRE" => {
                if parts.len() < 3 {
                    return Err(MemorustError::MissingArgument);
                }

                let seconds = parts[2]
                    .parse::<u64>()
                    .map_err(|_| MemorustError::InvalidSyntax)?;

                Ok(Command::Expire {
                    key: parts[1].clone(),
                    seconds,
                })
            }

            "TTL" => {
                if parts.len() < 2 {
                    return Err(MemorustError::MissingArgument);
                }

                Ok(Command::Ttl {
                    key: parts[1].clone(),
                })
            }

            "PING" => Ok(Command::Ping),

            "AOFREWRITE" => Ok(Command::AofRewrite),

            "INFO" => Ok(Command::Info),

            "FLUSHALL" => Ok(Command::FlushAll),

            _ => Err(MemorustError::UnknownCommand),
        }
    }
}
