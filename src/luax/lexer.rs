use crate::error::LuaXError;
use color_eyre::Result;

use super::tokens::{StringType, Token};

// Macro for trying to match with multiple functions
// If a function returns a token, the token is returned
// If a function returns an error, the error is returned
// If a function returns None, the next function is tried
macro_rules! try_all_paths {
    () => {
        TokenizeResult::None
    };
    ($e:expr) => {
        $e
    };
    ($head:expr $(,$tail:expr)* $(,)?) => {
        match $head {
            TokenizeResult::Some(t) => TokenizeResult::Some(t),
            TokenizeResult::Error(e) => TokenizeResult::Error(e),
            TokenizeResult::None => try_all_paths!($($tail,)*),
        }
    };
}

pub trait TokenProducer {
    fn next(&mut self) -> Option<Token>;
}

#[derive(Debug)]
pub struct Lexer<'s> {
    src: &'s str,
    chars: std::str::Chars<'s>,
    // Current character
    current: Option<char>,
    // Positioning
    current_pos_in_bytes: usize,
    // EOF
    emitted_eof: bool,
    html_text_mode: usize,
}

#[derive(Debug)]
enum TokenizeResult<'s> {
    // A token was produced
    Some(Token<'s>),
    // No valid token found
    None,
    // An error occurred
    Error(LuaXError),
}

impl<'s> Lexer<'s> {
    pub fn new(src: &'s str) -> Self {
        let mut chars = src.chars();
        let current = chars.next();
        Self {
            src,
            chars,
            current,
            current_pos_in_bytes: 0,
            emitted_eof: false,
            html_text_mode: 0,
        }
    }

    pub fn next_token(&mut self) -> Result<Option<Token<'s>>> {
        if self.html_text_mode > 0 {
            let c = self.current;
            self.advance();

            match c {
                Some('<') => {
                    if self.match_char('/') {
                        Ok(Some(Token::OpenClosingTag))
                    } else {
                        Ok(Some(Token::Lt))
                    }
                }
                Some(' ' | '\t' | '\n') => Ok(Some(Token::Whitespace)),
                Some('{') => {
                    if self.match_char('$') {
                        Ok(Some(Token::LuaStart))
                    } else {
                        Ok(Some(Token::HtmlTextChar('{')))
                    }
                }
                Some(c) => Ok(Some(Token::HtmlTextChar(c))),
                None => {
                    if self.emitted_eof {
                        Ok(None)
                    } else {
                        self.emitted_eof = true;
                        Ok(Some(Token::Eof))
                    }
                }
            }
        } else {
            let token = self.lex();

            match token {
                TokenizeResult::Some(token) => Ok(Some(token)),
                TokenizeResult::Error(error) => Err(error.into()),
                TokenizeResult::None => match self.current {
                    Some(c) => Err(LuaXError::UnexpectedCharacter(c).into()),
                    None => {
                        if self.emitted_eof {
                            Ok(None)
                        } else {
                            self.emitted_eof = true;
                            Ok(Some(Token::Eof))
                        }
                    }
                },
            }
        }
    }

    pub fn enable_html_text_mode(&mut self) {
        self.html_text_mode += 1;
    }

    pub fn disable_html_text_mode(&mut self) {
        self.html_text_mode -= 1;
    }

    fn lex(&mut self) -> TokenizeResult<'s> {
        self.skip_whitespace();

        try_all_paths!(
            self.single_char_token(),
            self.double_char_token(),
            self.triple_char_token(),
            self.string(),
            self.number(),
            self.comment(),
            self.identifier_or_keyword(),
        )
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.current {
            match c {
                ' ' | '\t' | '\n' => {
                    self.advance();
                }
                _ => break,
            }
        }
    }

    fn comment(&mut self) -> TokenizeResult<'s> {
        if self.match_char('-') {
            if self.match_char('-') {
                while let Some(c) = self.current {
                    if c == '\n' {
                        break;
                    }
                    self.advance();
                }
                self.lex()
            } else {
                TokenizeResult::Some(Token::Minus)
            }
        } else {
            TokenizeResult::None
        }
    }

    fn single_char_token_case(&mut self, c: char, kind: Token<'s>) -> TokenizeResult<'s> {
        if self.current == Some(c) {
            self.advance();
            TokenizeResult::Some(kind)
        } else {
            TokenizeResult::None
        }
    }

    fn single_char_token(&mut self) -> TokenizeResult<'s> {
        try_all_paths!(
            self.single_char_token_case('+', Token::Plus),
            self.single_char_token_case('*', Token::Star),
            self.single_char_token_case('^', Token::Hat),
            self.single_char_token_case('%', Token::Percent),
            self.single_char_token_case('&', Token::Amp),
            self.single_char_token_case('|', Token::Pipe),
            self.single_char_token_case('#', Token::Hash),
            self.single_char_token_case('(', Token::OpenParen),
            self.single_char_token_case(')', Token::CloseParen),
            self.single_char_token_case('}', Token::CloseBrace),
            self.single_char_token_case(']', Token::CloseBracket),
            self.single_char_token_case(';', Token::Semicolon),
            self.single_char_token_case(',', Token::Comma),
            self.single_char_token_case('!', Token::Bang),
        )
    }

    fn double_char_token_case(
        &mut self,
        first: char,
        second: char,
        short: Option<Token<'s>>,
        long: Token<'s>,
    ) -> TokenizeResult<'s> {
        if self.match_char(first) {
            if self.match_char(second) {
                TokenizeResult::Some(long)
            } else if let Some(short) = short {
                TokenizeResult::Some(short)
            } else {
                TokenizeResult::None
            }
        } else {
            TokenizeResult::None
        }
    }

    fn double_char_token_case_with_alts<const N: usize>(
        &mut self,
        first: char,
        second: [char; N],
        short: Option<Token<'s>>,
        long: [Token<'s>; N],
    ) -> TokenizeResult<'s> {
        if self.match_char(first) {
            for i in 0..N {
                if self.match_char(second[i]) {
                    return TokenizeResult::Some(long[i]);
                }
            }
            if let Some(short) = short {
                TokenizeResult::Some(short)
            } else {
                TokenizeResult::None
            }
        } else {
            TokenizeResult::None
        }
    }

    fn double_char_token(&mut self) -> TokenizeResult<'s> {
        try_all_paths!(
            self.double_char_token_case_with_alts(
                '<',
                ['=', '<', '/'],
                Some(Token::Lt),
                [Token::Le, Token::LtLt, Token::OpenClosingTag]
            ),
            self.double_char_token_case_with_alts(
                '>',
                ['=', '>'],
                Some(Token::Gt),
                [Token::Ge, Token::GtGt]
            ),
            self.double_char_token_case('{', '$', Some(Token::OpenBrace), Token::LuaStart),
            self.double_char_token_case('$', '}', None, Token::LuaEnd),
            self.double_char_token_case('=', '=', Some(Token::Eq), Token::EqEq),
            self.double_char_token_case('~', '=', Some(Token::Tilde), Token::TildeEq),
            self.double_char_token_case('/', '/', Some(Token::Slash), Token::SlashSlash),
            self.double_char_token_case(':', ':', Some(Token::Colon), Token::ColonColon),
        )
    }

    fn triple_char_token_case(
        &mut self,
        first: char,
        second: char,
        third: char,
        short: Token<'s>,
        mid: Token<'s>,
        long: Token<'s>,
    ) -> TokenizeResult<'s> {
        if self.match_char(first) {
            if self.match_char(second) {
                if self.match_char(third) {
                    TokenizeResult::Some(long)
                } else {
                    TokenizeResult::Some(mid)
                }
            } else {
                TokenizeResult::Some(short)
            }
        } else {
            TokenizeResult::None
        }
    }

    fn triple_char_token(&mut self) -> TokenizeResult<'s> {
        try_all_paths!(self.triple_char_token_case(
            '.',
            '.',
            '.',
            Token::Dot,
            Token::DotDot,
            Token::DotDotDot
        ),)
    }

    fn number(&mut self) -> TokenizeResult<'s> {
        if !self.current.map_or(false, |c| c.is_numeric()) {
            return TokenizeResult::None;
        }

        let start = self.current_pos_in_bytes;

        while self.current.map_or(false, |c| c.is_numeric()) {
            self.advance();
        }

        if self.match_char('.') {
            while self.current.map_or(false, |c| c.is_numeric()) {
                self.advance();
            }
        }

        if self.match_char('e') || self.match_char('E') {
            if self.match_char('-') || self.match_char('+') {
                self.advance();
            }
            while self.current.map_or(false, |c| c.is_numeric()) {
                self.advance();
            }
        }
        let end = self.current_pos_in_bytes;

        TokenizeResult::Some(Token::Number(&self.src[start..end]))
    }

    fn string(&mut self) -> TokenizeResult<'s> {
        let ty = if self.match_char('"') {
            StringType::Double
        } else if self.match_char('\'') {
            StringType::Single
        } else if self.match_char('[') {
            if self.match_char('[') {
                StringType::DoubleBracket
            } else {
                return TokenizeResult::Some(Token::OpenBracket);
            }
        } else {
            return TokenizeResult::None;
        };

        let start = self.current_pos_in_bytes;
        let mut finished = false;

        match ty {
            StringType::Single => {
                let mut escaped = false;
                while self.current.is_some() {
                    if self.match_char('\\') {
                        escaped = true;
                    } else if self.match_char('\'') && !escaped {
                        finished = true;
                        break;
                    } else {
                        escaped = false;
                    }
                    self.advance();
                }
            }
            StringType::Double => {
                let mut escaped = false;
                while self.current.is_some() {
                    if self.match_char('\\') {
                        escaped = true;
                    } else if self.match_char('"') && !escaped {
                        finished = true;
                        break;
                    } else {
                        escaped = false;
                    }
                    self.advance();
                }
            }
            StringType::DoubleBracket => {
                let mut almost_close = false;
                while self.current.is_some() {
                    if self.current == Some(']') {
                        if almost_close {
                            finished = true;
                            break;
                        } else {
                            almost_close = true;
                        }
                    } else {
                        almost_close = false;
                    }
                    self.advance();
                }
            }
        }

        if !finished {
            TokenizeResult::Error(LuaXError::UnterminatedStringLiteral)
        } else {
            let end = self.current_pos_in_bytes - 1;
            if ty == StringType::DoubleBracket {
                self.advance();
            }

            TokenizeResult::Some(Token::String(&self.src[start..end], ty))
        }
    }

    fn identifier_or_keyword(&mut self) -> TokenizeResult<'s> {
        if self.current.is_none() || !Self::is_valid_identifier_start(self.current.unwrap()) {
            return TokenizeResult::None;
        }

        let start = self.current_pos_in_bytes;
        self.advance();

        while self.current.is_some() && Self::is_valid_in_identifier(self.current.unwrap()) {
            self.advance();
        }

        let end = self.current_pos_in_bytes;

        TokenizeResult::Some(match &self.src[start..end] {
            "and" => Token::And,
            "break" => Token::Break,
            "do" => Token::Do,
            "else" => Token::Else,
            "elseif" => Token::ElseIf,
            "end" => Token::End,
            "false" => Token::False,
            "for" => Token::For,
            "function" => Token::Function,
            "goto" => Token::Goto,
            "if" => Token::If,
            "in" => Token::In,
            "local" => Token::Local,
            "nil" => Token::Nil,
            "not" => Token::Not,
            "or" => Token::Or,
            "repeat" => Token::Repeat,
            "return" => Token::Return,
            "then" => Token::Then,
            "true" => Token::True,
            "until" => Token::Until,
            "while" => Token::While,
            _ => Token::Identifier(&self.src[start..end]),
        })
    }

    fn advance(&mut self) -> Option<char> {
        self.current_pos_in_bytes += self.current.map_or(0, |c| c.len_utf8());
        self.current = self.chars.next();

        self.current
    }

    fn match_char(&mut self, c: char) -> bool {
        if self.current == Some(c) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn is_valid_in_identifier(c: char) -> bool {
        Self::is_valid_identifier_start(c) || c.is_numeric()
    }

    fn is_valid_identifier_start(c: char) -> bool {
        c.is_alphabetic() || c == '_' // TODO: Support some more unicode characters
                                      //       like emojis (or even emoji modifier sequences?)
    }
}
