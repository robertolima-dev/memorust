use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum MemorustError {
    InvalidCommand,
    MissingArgument,
    UnknownCommand,
    InvalidSyntax,
}

impl fmt::Display for MemorustError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemorustError::InvalidCommand => write!(f, "invalid command"),
            MemorustError::MissingArgument => write!(f, "missing argument"),
            MemorustError::UnknownCommand => write!(f, "unknown command"),
            MemorustError::InvalidSyntax => write!(f, "invalid syntax"),
        }
    }
}

impl std::error::Error for MemorustError {}
