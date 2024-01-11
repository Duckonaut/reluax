use std::io::Write;

use crate::error::LuaXError;

use crate::luax::lexer::Lexer;
use crate::luax::tokens::Token;

use color_eyre::Result;

/// Macro for trying to get a match from a list of functions
/// If a function returns a value, the token is returned
/// If a function returns an error, the error is returned
/// If a function returns the `InvalidStart` error, the next function is tried
macro_rules! alternatives {
    () => {
        Err(LuaXError::InvalidStart)
    };
    ($e:expr) => {
        $e
    };
    ($head:expr $(,$tail:expr)* $(,)?) => {
        match $head {
            Ok(t) => Ok(t),
            Err(e) => {
                match e.downcast_ref::<LuaXError>() {
                    Some(LuaXError::InvalidStart) => alternatives!($($tail),*),
                    _ => Err(e),
                }
            }
        }
    };
}

macro_rules! repeat_until_not {
    ($e:expr) => {
        loop {
            match $e {
                Ok(()) => {}
                Err(e) => match e.downcast_ref::<LuaXError>() {
                    Some(LuaXError::InvalidStart) => break,
                    _ => return Err(e),
                },
            }
        }
    };
}

macro_rules! optionally {
    ($e:expr) => {
        match $e {
            Ok(t) => Some(t),
            Err(e) => match e.downcast_ref::<LuaXError>() {
                Some(LuaXError::InvalidStart) => None,
                _ => return Err(e),
            },
        }
    };
}

macro_rules! require {
    ($e:expr, $err:expr $(,)?) => {
        match $e {
            Ok(t) => t,
            Err(e) => match e.downcast_ref::<LuaXError>() {
                Some(LuaXError::InvalidStart) => return Err($err.into()),
                _ => return Err(e),
            },
        }
    };
}

pub struct Preprocessor<'s, W: Write> {
    lexer: Lexer<'s>,
    current: Token<'s>,
    out_stream: W,
    first_token: bool,
}

impl<'s, W: Write> Preprocessor<'s, W> {
    pub fn new(template: &'s str, out_stream: W) -> Result<Self> {
        let mut lexer = Lexer::new(template);
        let current = lexer.next_token()?.unwrap();
        Ok(Preprocessor {
            lexer,
            current,
            out_stream,
            first_token: true,
        })
    }

    fn next_token(&mut self) -> Result<()> {
        if self.current != Token::Eof {
            if !self.first_token {
                write!(self.out_stream, " ")?;
            }
            write!(self.out_stream, "{}", self.current)?;
            self.first_token = false;
        }
        match self.lexer.next_token()? {
            Some(token) => {
                self.current = token;
            }
            None => return Err(LuaXError::InvalidStart.into()),
        }
        Ok(())
    }

    fn match_token(&mut self, token: Token<'s>) -> Result<bool> {
        if self.current == token {
            self.next_token()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn consume_token(&mut self, token: Token<'s>, if_not: LuaXError) -> Result<()> {
        if self.current == token {
            self.next_token()?;
            Ok(())
        } else {
            Err(if_not.into())
        }
    }

    fn next_token_silent(&mut self) -> Result<()> {
        match self.lexer.next_token()? {
            Some(token) => {
                self.current = token;
            }
            None => return Err(LuaXError::InvalidStart.into()),
        }
        Ok(())
    }

    fn match_token_silent(&mut self, token: Token<'s>) -> Result<bool> {
        if self.current == token {
            self.next_token_silent()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn consume_token_silent(&mut self, token: Token<'s>, if_not: LuaXError) -> Result<()> {
        if self.current == token {
            self.next_token_silent()?;
            Ok(())
        } else {
            Err(if_not.into())
        }
    }

    pub fn preprocess(mut self) -> Result<()> {
        self.chunk()
    }

    fn chunk(&mut self) -> Result<()> {
        self.block()
    }

    fn block(&mut self) -> Result<()> {
        repeat_until_not!(self.statement());

        optionally!(self.return_statement());

        Ok(())
    }

    fn statement(&mut self) -> Result<()> {
        if self.match_token(Token::Semicolon)? {
            return Ok(());
        }
        alternatives!(
            self.label(),
            self.break_statement(),
            self.goto_statement(),
            self.do_statement(),
            self.while_statement(),
            self.repeat_statement(),
            self.if_statement(),
            self.for_statement(),
            self.function_statement(),
            self.local_starting_statement(),
            self.call_or_assignment(),
        )
    }

    fn break_statement(&mut self) -> Result<()> {
        if !self.match_token(Token::Break)? {
            return Err(LuaXError::InvalidStart.into());
        }

        Ok(())
    }

    fn goto_statement(&mut self) -> Result<()> {
        if !self.match_token(Token::Goto)? {
            return Err(LuaXError::InvalidStart.into());
        }

        require!(
            self.identifier(),
            LuaXError::NeededToken("identifier".to_string())
        );

        Ok(())
    }

    fn do_statement(&mut self) -> Result<()> {
        if !self.match_token(Token::Do)? {
            return Err(LuaXError::InvalidStart.into());
        }

        require!(self.block(), LuaXError::ExpectedExpression);

        self.consume_token(Token::End, LuaXError::NeededToken(Token::End.to_string()))
    }

    fn while_statement(&mut self) -> Result<()> {
        if !self.match_token(Token::While)? {
            return Err(LuaXError::InvalidStart.into());
        }

        require!(self.expression(), LuaXError::ExpectedExpression);

        self.consume_token(Token::Do, LuaXError::NeededToken(Token::Do.to_string()))?;

        require!(self.block(), LuaXError::ExpectedExpression);

        self.consume_token(Token::End, LuaXError::NeededToken(Token::End.to_string()))
    }

    fn repeat_statement(&mut self) -> Result<()> {
        if !self.match_token(Token::Repeat)? {
            return Err(LuaXError::InvalidStart.into());
        }

        require!(self.block(), LuaXError::ExpectedExpression);

        self.consume_token(
            Token::Until,
            LuaXError::NeededToken(Token::Until.to_string()),
        )?;

        require!(self.expression(), LuaXError::ExpectedExpression);

        Ok(())
    }

    fn if_statement(&mut self) -> Result<()> {
        if !self.match_token(Token::If)? {
            return Err(LuaXError::InvalidStart.into());
        }

        require!(self.expression(), LuaXError::ExpectedExpression);

        self.consume_token(Token::Then, LuaXError::NeededToken(Token::Then.to_string()))?;

        require!(self.block(), LuaXError::ExpectedExpression);

        loop {
            if !self.match_token(Token::ElseIf)? {
                break;
            }

            require!(self.expression(), LuaXError::ExpectedExpression);

            self.consume_token(Token::Then, LuaXError::NeededToken(Token::Then.to_string()))?;

            require!(self.block(), LuaXError::ExpectedExpression);
        }

        if self.match_token(Token::Else)? {
            require!(self.block(), LuaXError::ExpectedExpression);
        }

        self.consume_token(Token::End, LuaXError::NeededToken(Token::End.to_string()))
    }

    fn for_statement(&mut self) -> Result<()> {
        if !self.match_token(Token::For)? {
            return Err(LuaXError::InvalidStart.into());
        }

        require!(
            self.identifier(),
            LuaXError::NeededToken("identifier".to_string())
        );

        if self.match_token(Token::Eq)? {
            require!(self.expression(), LuaXError::ExpectedExpression);

            self.consume_token(
                Token::Comma,
                LuaXError::NeededToken(Token::Comma.to_string()),
            )?;

            require!(self.expression(), LuaXError::ExpectedExpression);

            if self.match_token(Token::Comma)? {
                require!(self.expression(), LuaXError::ExpectedExpression);
            }
        } else {
            loop {
                if !self.match_token(Token::Comma)? {
                    break;
                }

                require!(
                    self.identifier(),
                    LuaXError::NeededToken("identifier".to_string())
                );
            }

            self.consume_token(Token::In, LuaXError::NeededToken(Token::In.to_string()))?;

            require!(self.explist(), LuaXError::ExpectedExpression);
        }

        self.consume_token(Token::Do, LuaXError::NeededToken(Token::Do.to_string()))?;

        require!(self.block(), LuaXError::ExpectedExpression);

        self.consume_token(Token::End, LuaXError::NeededToken(Token::End.to_string()))
    }

    fn function_statement(&mut self) -> Result<()> {
        if !self.match_token(Token::Function)? {
            return Err(LuaXError::InvalidStart.into());
        }

        require!(
            self.funcname(),
            LuaXError::NeededToken("identifier".to_string())
        );

        require!(self.funcbody(), LuaXError::ExpectedExpression);

        Ok(())
    }

    fn local_starting_statement(&mut self) -> Result<()> {
        if !self.match_token(Token::Local)? {
            return Err(LuaXError::InvalidStart.into());
        }

        if self.match_token(Token::Function)? {
            require!(
                self.identifier(),
                LuaXError::NeededToken("identifier".to_string())
            );

            require!(self.funcbody(), LuaXError::ExpectedExpression);

            return Ok(());
        }

        require!(
            self.attribute_name_list(),
            LuaXError::NeededToken("identifier".to_string())
        );

        if self.match_token(Token::Eq)? {
            require!(self.explist(), LuaXError::ExpectedExpression);
        }

        Ok(())
    }

    fn return_statement(&mut self) -> Result<()> {
        if !self.match_token(Token::Return)? {
            return Err(LuaXError::InvalidStart.into());
        }

        optionally!(self.expression());

        loop {
            if !self.match_token(Token::Comma)? {
                break;
            }

            require!(self.expression(), LuaXError::ExpectedExpression);
        }

        self.match_token(Token::Semicolon)?;

        Ok(())
    }

    fn attribute(&mut self) -> Result<()> {
        if !self.match_token(Token::Lt)? {
            return Ok(());
        }

        require!(
            self.identifier(),
            LuaXError::NeededToken("identifier".to_string())
        );

        self.consume_token(Token::Gt, LuaXError::NeededToken(Token::Gt.to_string()))
    }

    fn attribute_name_list(&mut self) -> Result<()> {
        require!(
            self.identifier(),
            LuaXError::NeededToken("identifier".to_string())
        );

        self.attribute()?;

        loop {
            if !self.match_token(Token::Comma)? {
                break;
            }

            require!(
                self.identifier(),
                LuaXError::NeededToken("identifier".to_string())
            );

            self.attribute()?;
        }

        Ok(())
    }

    fn label(&mut self) -> Result<()> {
        if !self.match_token(Token::ColonColon)? {
            return Err(LuaXError::InvalidStart.into());
        }

        require!(
            self.identifier(),
            LuaXError::NeededToken("identifier".to_string())
        );

        self.consume_token(
            Token::ColonColon,
            LuaXError::NeededToken(Token::ColonColon.to_string()),
        )
    }

    fn funcname(&mut self) -> Result<()> {
        require!(
            self.identifier(),
            LuaXError::NeededToken("identifier".to_string())
        );

        loop {
            if !self.match_token(Token::Dot)? {
                break;
            }

            require!(
                self.identifier(),
                LuaXError::NeededToken("identifier".to_string())
            );
        }

        if self.match_token(Token::Colon)? {
            require!(
                self.identifier(),
                LuaXError::NeededToken("identifier".to_string())
            );
        }

        Ok(())
    }

    fn explist(&mut self) -> Result<()> {
        require!(self.expression(), LuaXError::ExpectedVar,);

        loop {
            if !self.match_token(Token::Comma)? {
                break;
            }

            require!(self.expression(), LuaXError::ExpectedVar,);
        }

        Ok(())
    }

    fn expression(&mut self) -> Result<()> {
        alternatives!(
            self.html_template(),
            self.literal(),
            self.function_def(),
            self.access_or_call(),
            self.table_constructor(),
            self.unary_expression(),
        )?;

        optionally!(self.binary_expression_followup());

        Ok(())
    }

    fn literal(&mut self) -> Result<()> {
        alternatives!(
            self.number(),
            self.string(),
            self.boolean(),
            self.nil_literal(),
        )
    }

    fn call_or_assignment(&mut self) -> Result<()> {
        if optionally!(self.access_or_call()).is_none() {
            return Err(LuaXError::InvalidStart.into());
        }

        if self.match_token(Token::Eq)? {
            require!(self.explist(), LuaXError::ExpectedExpression,);
        } else if self.match_token(Token::Comma)? {
            require!(self.access_or_call(), LuaXError::ExpectedExpression,);
            while self.match_token(Token::Comma)? {
                require!(self.access_or_call(), LuaXError::ExpectedExpression,);
            }
            if self.match_token(Token::Eq)? {
                require!(self.explist(), LuaXError::ExpectedExpression,);
            }
        }

        Ok(())
    }

    fn access_or_call(&mut self) -> Result<()> {
        if self.match_token(Token::OpenParen)? {
            require!(self.expression(), LuaXError::ExpectedVar,);
            self.consume_token(
                Token::CloseParen,
                LuaXError::NeededToken(Token::CloseParen.to_string()),
            )?;
        } else if optionally!(self.identifier()).is_some() {
        } else {
            return Err(LuaXError::InvalidStart.into());
        }

        // } else if let Some(()) = optionally!(self.html_template()) {
        //     return Ok(());

        self.continue_access_or_call()
    }

    fn continue_access_or_call(&mut self) -> Result<()> {
        loop {
            if self.match_token(Token::OpenBracket)? {
                require!(self.expression(), LuaXError::ExpectedVar,);
                self.consume_token(
                    Token::CloseBracket,
                    LuaXError::NeededToken(Token::CloseBracket.to_string()),
                )?;
            } else if self.match_token(Token::Dot)? {
                require!(
                    self.identifier(),
                    LuaXError::NeededToken("identifier".to_string())
                );
            } else if optionally!(self.args()).is_some() {
            } else if self.match_token(Token::Colon)? {
                require!(
                    self.identifier(),
                    LuaXError::NeededToken("identifier".to_string())
                );
                require!(self.args(), LuaXError::ExpectedExpression,);
            } else {
                break;
            }
        }

        Ok(())
    }

    fn args(&mut self) -> Result<()> {
        if self.match_token(Token::OpenParen)? {
            if !self.match_token(Token::CloseParen)? {
                require!(self.explist(), LuaXError::ExpectedExpression,);
                self.consume_token(
                    Token::CloseParen,
                    LuaXError::NeededToken(Token::CloseParen.to_string()),
                )
            } else {
                Ok(())
            }
        } else if optionally!(self.table_constructor()).is_some()
            || optionally!(self.string()).is_some()
        {
            Ok(())
        } else {
            Err(LuaXError::InvalidStart.into())
        }
    }

    fn function_def(&mut self) -> Result<()> {
        if !self.match_token(Token::Function)? {
            return Err(LuaXError::InvalidStart.into());
        }

        require!(self.funcbody(), LuaXError::ExpectedExpression);

        Ok(())
    }

    fn funcbody(&mut self) -> Result<()> {
        self.consume_token(
            Token::OpenParen,
            LuaXError::NeededToken(Token::OpenParen.to_string()),
        )?;

        optionally!(self.parlist());

        self.consume_token(
            Token::CloseParen,
            LuaXError::NeededToken(Token::CloseParen.to_string()),
        )?;

        require!(self.block(), LuaXError::ExpectedExpression);

        self.consume_token(Token::End, LuaXError::NeededToken(Token::End.to_string()))
    }

    fn parlist(&mut self) -> Result<()> {
        if self.match_token(Token::DotDotDot)? {
            return Ok(());
        }

        if optionally!(self.identifier()).is_some() {
            while self.match_token(Token::Comma)? {
                if self.match_token(Token::DotDotDot)? {
                    return Ok(());
                }

                if optionally!(self.identifier()).is_some() {
                    continue;
                } else {
                    return Err(LuaXError::ExpectedExpression.into());
                }
            }

            Ok(())
        } else {
            Err(LuaXError::InvalidStart.into())
        }
    }

    fn table_constructor(&mut self) -> Result<()> {
        if !self.match_token(Token::OpenBrace)? {
            return Err(LuaXError::InvalidStart.into());
        }

        optionally!(self.fieldlist());

        self.consume_token(
            Token::CloseBrace,
            LuaXError::NeededToken(Token::CloseBrace.to_string()),
        )
    }

    fn fieldlist(&mut self) -> Result<()> {
        if optionally!(self.field()).is_none() {
            return Ok(());
        }

        loop {
            if !self.match_token(Token::Comma)? && !self.match_token(Token::Semicolon)? {
                break;
            }

            if self.match_token(Token::CloseBrace)? {
                break;
            }

            require!(self.field(), LuaXError::ExpectedExpression);
        }

        Ok(())
    }

    fn field(&mut self) -> Result<()> {
        if optionally!(self.identifier()).is_some() {
            if self.match_token(Token::Eq)? {
                require!(self.expression(), LuaXError::ExpectedExpression);
            } else {
                self.continue_access_or_call()?;
            }
        } else if self.match_token(Token::OpenBracket)? {
            require!(self.expression(), LuaXError::ExpectedExpression);
            self.consume_token(
                Token::CloseBracket,
                LuaXError::NeededToken(Token::CloseBracket.to_string()),
            )?;
            self.consume_token(Token::Eq, LuaXError::NeededToken(Token::Eq.to_string()))?;
            require!(self.expression(), LuaXError::ExpectedExpression);
        } else if optionally!(self.expression()).is_none() {
            return Err(LuaXError::InvalidStart.into());
        }

        Ok(())
    }

    fn binary_expression_followup(&mut self) -> Result<()> {
        loop {
            if !self.match_token(Token::Plus)?
                && !self.match_token(Token::Minus)?
                && !self.match_token(Token::Star)?
                && !self.match_token(Token::Slash)?
                && !self.match_token(Token::SlashSlash)?
                && !self.match_token(Token::Hat)?
                && !self.match_token(Token::Percent)?
                && !self.match_token(Token::Amp)?
                && !self.match_token(Token::Tilde)?
                && !self.match_token(Token::Pipe)?
                && !self.match_token(Token::LtLt)?
                && !self.match_token(Token::GtGt)?
                && !self.match_token(Token::DotDot)?
                && !self.match_token(Token::Lt)?
                && !self.match_token(Token::Le)?
                && !self.match_token(Token::Gt)?
                && !self.match_token(Token::Ge)?
                && !self.match_token(Token::EqEq)?
                && !self.match_token(Token::TildeEq)?
                && !self.match_token(Token::And)?
                && !self.match_token(Token::Or)?
            {
                break;
            }

            require!(self.expression(), LuaXError::ExpectedExpression);
        }

        Ok(())
    }

    fn unary_expression(&mut self) -> Result<()> {
        if !self.match_token(Token::Not)?
            && !self.match_token(Token::Hash)?
            && !self.match_token(Token::Minus)?
            && !self.match_token(Token::Tilde)?
        {
            return Err(LuaXError::InvalidStart.into());
        }

        require!(self.expression(), LuaXError::ExpectedExpression);

        Ok(())
    }

    fn boolean(&mut self) -> Result<()> {
        if self.match_token(Token::True)? || self.match_token(Token::False)? {
            Ok(())
        } else {
            Err(LuaXError::InvalidStart.into())
        }
    }

    fn nil_literal(&mut self) -> Result<()> {
        if self.match_token(Token::Nil)? {
            Ok(())
        } else {
            Err(LuaXError::InvalidStart.into())
        }
    }

    fn number(&mut self) -> Result<()> {
        if let Token::Number(_) = self.current {
            self.next_token()?;
            Ok(())
        } else {
            Err(LuaXError::InvalidStart.into())
        }
    }

    fn string(&mut self) -> Result<()> {
        if let Token::String(..) = self.current {
            self.next_token()?;
            Ok(())
        } else {
            Err(LuaXError::InvalidStart.into())
        }
    }

    fn identifier(&mut self) -> Result<String> {
        if let Token::Identifier(s) = self.current {
            self.next_token()?;
            Ok(s.to_string())
        } else {
            Err(LuaXError::InvalidStart.into())
        }
    }

    fn html_string(&mut self) -> Result<String> {
        if let Token::String(s, _) = self.current {
            self.next_token_silent()?;
            Ok(s.to_string())
        } else {
            Err(LuaXError::InvalidStart.into())
        }
    }

    fn html_identifier(&mut self) -> Result<String> {
        if let Token::Identifier(s) = self.current {
            self.next_token_silent()?;
            Ok(s.to_string())
        } else {
            Err(LuaXError::InvalidStart.into())
        }
    }

    fn html_template(&mut self) -> Result<()> {
        if !self.match_token_silent(Token::Lt)? {
            return Err(LuaXError::InvalidStart.into());
        }

        self.lexer.allow_unknowns();

        let tag = require!(
            self.html_identifier(),
            LuaXError::NeededToken("identifier".to_string())
        );

        write!(self.out_stream, " {{ tag=\"{}\", ", tag)?;

        self.html_attributes()?;

        if self.match_token_silent(Token::Slash)? {
            self.consume_token_silent(Token::Gt, LuaXError::NeededToken(Token::Gt.to_string()))?;
            write!(self.out_stream, "children={{}} }}")?;
            return Ok(());
        }

        self.consume_token_silent(Token::Gt, LuaXError::NeededToken(Token::Gt.to_string()))?;

        self.html_children()?;

        self.consume_token_silent(
            Token::OpenClosingTag,
            LuaXError::NeededToken(Token::OpenClosingTag.to_string()),
        )?;

        let closing_tag = require!(
            self.html_identifier(),
            LuaXError::NeededToken("identifier".to_string())
        );

        if closing_tag != tag {
            return Err(LuaXError::InvalidStart.into());
        }

        self.consume_token_silent(Token::Gt, LuaXError::NeededToken(Token::Gt.to_string()))?;

        write!(self.out_stream, " }}")?;

        self.lexer.disallow_unknowns();

        Ok(())
    }

    fn html_attributes(&mut self) -> Result<()> {
        write!(self.out_stream, "attrs={{")?;
        loop {
            let key = optionally!(self.html_identifier());

            if key.is_none() {
                break;
            }

            let key = key.unwrap();

            self.consume_token_silent(Token::Eq, LuaXError::NeededToken(Token::Eq.to_string()))?;

            write!(self.out_stream, "{}=", key)?;

            if self.match_token_silent(Token::OpenBrace)? {
                require!(self.expression(), LuaXError::ExpectedExpression);
                self.consume_token_silent(
                    Token::CloseBrace,
                    LuaXError::NeededToken(Token::CloseBrace.to_string()),
                )?;
            } else {
                let value = require!(
                    self.html_string(),
                    LuaXError::NeededToken("string".to_string())
                );

                write!(self.out_stream, "\"{}\"", value)?;
            }

            write!(self.out_stream, ", ")?;
        }
        write!(self.out_stream, "}}, ")?;

        Ok(())
    }

    fn html_children(&mut self) -> Result<()> {
        write!(self.out_stream, "children={{")?;
        loop {
            if self.current == Token::OpenClosingTag {
                break;
            }
            if self.match_token_silent(Token::LuaStart)? {
                self.lexer.disallow_unknowns();
                require!(self.expression(), LuaXError::ExpectedExpression);
                self.lexer.allow_unknowns();
                self.consume_token_silent(
                    Token::LuaEnd,
                    LuaXError::NeededToken(Token::LuaEnd.to_string()),
                )?;
                write!(self.out_stream, ",")?;
                continue;
            }

            if optionally!(self.html_template()).is_some() {
                write!(self.out_stream, ",")?;
                continue;
            }

            if self.current == Token::Lt
                || self.current == Token::OpenClosingTag
                || self.current == Token::LuaStart
            {
                break;
            }

            // handle plain HTML text, which can really be anything. Needs to become
            // a string literal
            write!(self.out_stream, " \"")?;
            self.lexer.emit_whitespace();
            loop {
                if self.current == Token::Lt
                    || self.current == Token::LuaStart
                    || self.current == Token::OpenClosingTag
                {
                    break;
                }
                // all other tokens *should* be fine to just emit
                write!(self.out_stream, "{}", self.current)?;
                self.next_token_silent()?;
            }
            self.lexer.hide_whitespace();
            write!(self.out_stream, "\",")?;
        }
        write!(self.out_stream, "}}")?;

        Ok(())
    }
}
