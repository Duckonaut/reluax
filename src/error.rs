use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LuaXError {
    InvalidStart, // used internally
    MissingField(String),
    WrongFieldType(String),
    NonTableChildren,
    UnexpectedCharacter(char),
    UnexpectedToken(String),
    NeededToken(String),
    ExpectedVar,
    ExpectedExpression,
    UnterminatedStringLiteral,
    InvalidEscapeSequence(char),
    InvalidAssignmentTarget,
}

impl std::error::Error for LuaXError {}

impl Display for LuaXError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            LuaXError::InvalidStart => write!(f, "Invalid start"),
            LuaXError::MissingField(field) => write!(f, "Missing required field: {}", field),
            LuaXError::WrongFieldType(field) => write!(f, "Wrong field type: field {}", field),
            LuaXError::NonTableChildren => write!(f, "Children must be tables"),
            LuaXError::UnexpectedCharacter(c) => write!(f, "Unexpected character: '{}'", c),
            LuaXError::UnexpectedToken(token) => write!(f, "Unexpected token: {}", token),
            LuaXError::NeededToken(token) => write!(f, "Needed token: {}", token),
            LuaXError::ExpectedVar => write!(f, "Expected variable"),
            LuaXError::ExpectedExpression => write!(f, "Expected expression"),
            LuaXError::UnterminatedStringLiteral => write!(f, "Unterminated string literal"),
            LuaXError::InvalidEscapeSequence(c) => write!(f, "Invalid escape sequence: {}", c),
            LuaXError::InvalidAssignmentTarget => write!(f, "Invalid assignment target"),
        }
    }
}

#[derive(Debug)]
pub enum ReluaxError {
    LuaX(LuaXError),
    Lua(rlua::Error),
    Server(String),
}

impl std::error::Error for ReluaxError {}

impl Display for ReluaxError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ReluaxError::LuaX(err) => write!(f, "{}", err),
            ReluaxError::Lua(err) => write!(f, "{}", err),
            ReluaxError::Server(err) => write!(f, "{}", err),
        }
    }
}

impl From<LuaXError> for ReluaxError {
    fn from(err: LuaXError) -> Self {
        ReluaxError::LuaX(err)
    }
}

impl From<rlua::Error> for ReluaxError {
    fn from(err: rlua::Error) -> Self {
        ReluaxError::Lua(err)
    }
}
