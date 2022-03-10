use super::*;

/// For when a command could not be parsed.
#[derive(Debug)]
pub enum CommandParsingError {
    Prefix(&'static str),
    Clap(clap::Error),
}

impl From<&'static str> for CommandParsingError {
    fn from(s: &'static str) -> Self {
        CommandParsingError::Prefix(s)
    }
}

impl From<clap::Error> for CommandParsingError {
    fn from(e: clap::Error) -> Self {
        CommandParsingError::Clap(e)
    }
}

impl std::fmt::Display for CommandParsingError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CommandParsingError::Prefix(e) => write!(f, "{}", e),
            CommandParsingError::Clap(e) => write!(f, "{}", e),
        }
    }
}
