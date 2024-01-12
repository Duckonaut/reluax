use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LuaXError {
    InvalidStart, // used internally
    NonTableChildren,
    NonTableAttrs,
    NeededToken(String),
    ExpectedVar,
    ExpectedExpression,
    UnterminatedStringLiteral,
}

impl std::error::Error for LuaXError {}

impl Display for LuaXError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            LuaXError::InvalidStart => write!(f, "Invalid start"),
            LuaXError::NonTableChildren => write!(f, "Children must be tables"),
            LuaXError::NonTableAttrs => write!(f, "Attrs must be tables"),
            LuaXError::NeededToken(token) => write!(f, "Needed token: {}", token),
            LuaXError::ExpectedVar => write!(f, "Expected variable"),
            LuaXError::ExpectedExpression => write!(f, "Expected expression"),
            LuaXError::UnterminatedStringLiteral => write!(f, "Unterminated string literal"),
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
