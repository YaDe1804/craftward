//! Errors produced while loading a project file.

use std::fmt;

/// One problem found while checking a project file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// Location of the problem, such as `plan.phases[0].tasks[1].due`.
    pub path: String,
    /// What is wrong at that location.
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

/// Failure to load a `craftward.json` file.
#[derive(Debug)]
pub enum LoadError {
    /// The file could not be read.
    Read(std::io::Error),
    /// The text is not valid JSON.
    Syntax {
        /// 1-based line number.
        line: usize,
        /// 1-based column number.
        column: usize,
        /// Parser message.
        message: String,
    },
    /// `schema_version` is newer than this build of Craftward understands.
    UnsupportedSchema {
        /// The `schema_version` value from the file.
        version: u64,
    },
    /// The file is a supported schema version, and one or more rules failed.
    Validation(Vec<ValidationError>),
}

impl fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "failed to read file: {error}"),
            Self::Syntax {
                line,
                column,
                message,
            } => write!(
                formatter,
                "syntax error at line {line}, column {column}: {message}"
            ),
            Self::UnsupportedSchema { .. } => {
                formatter.write_str("file was written by a newer Craftward version")
            }
            Self::Validation(errors) => {
                write!(formatter, "invalid project file ({} errors)", errors.len())
            }
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Syntax { .. } | Self::UnsupportedSchema { .. } | Self::Validation(_) => None,
        }
    }
}

impl PartialEq for LoadError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Syntax {
                    line,
                    column,
                    message,
                },
                Self::Syntax {
                    line: other_line,
                    column: other_column,
                    message: other_message,
                },
            ) => line == other_line && column == other_column && message == other_message,
            (Self::UnsupportedSchema { version }, Self::UnsupportedSchema { version: other }) => {
                version == other
            }
            (Self::Validation(errors), Self::Validation(other)) => errors == other,
            (Self::Read(error), Self::Read(other)) => error.kind() == other.kind(),
            _ => false,
        }
    }
}

impl Eq for LoadError {}
