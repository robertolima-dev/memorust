use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum MemorsError {
    InvalidCommand,
    MissingArgument,
    UnknownCommand,
    InvalidSyntax,
}

impl fmt::Display for MemorsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemorsError::InvalidCommand => write!(f, "invalid command"),
            MemorsError::MissingArgument => write!(f, "missing argument"),
            MemorsError::UnknownCommand => write!(f, "unknown command"),
            MemorsError::InvalidSyntax => write!(f, "invalid syntax"),
        }
    }
}

impl std::error::Error for MemorsError {}
