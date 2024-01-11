use crate::luax::lexer::*;
use crate::luax::tokens::*;
use color_eyre::Result;

fn compare_tokens<'a>(lua: &'a str, expected: Vec<Token<'a>>) -> Result<()> {
    let mut lexer = Lexer::new(lua);
    let mut tokens = Vec::new();
    loop {
        let token = lexer.next_token()?;
        if token.is_none() {
            break;
        }
        tokens.push(token.unwrap());
    }

    assert_eq!(tokens, expected);

    Ok(())
}

#[test]
fn empty() -> Result<()> {
    compare_tokens("", vec![Token::Eof])
}

#[test]
fn number() -> Result<()> {
    compare_tokens("123", vec![Token::Number("123"), Token::Eof])
}

#[test]
fn string() -> Result<()> {
    compare_tokens(
        "\"hello world\"",
        vec![Token::String("hello world", StringType::Double), Token::Eof],
    )
}

#[test]
fn empty_string() -> Result<()> {
    compare_tokens(
        "\"\"",
        vec![Token::String("", StringType::Double), Token::Eof],
    )
}

#[test]
fn all_string_types() -> Result<()> {
    compare_tokens(
        "\"hello world\"'hello world'[[hello world]]",
        vec![
            Token::String("hello world", StringType::Double),
            Token::String("hello world", StringType::Single),
            Token::String("hello world", StringType::DoubleBracket),
            Token::Eof,
        ],
    )
}

#[test]
fn table() -> Result<()> {
    compare_tokens(
        "{1, 2, 3}",
        vec![
            Token::OpenBrace,
            Token::Number("1"),
            Token::Comma,
            Token::Number("2"),
            Token::Comma,
            Token::Number("3"),
            Token::CloseBrace,
            Token::Eof,
        ],
    )
}

#[test]
fn table_with_string_keys() -> Result<()> {
    compare_tokens(
        "{[\"a\"]=1, [\"b\"]=2, [\"c\"]=3}",
        vec![
            Token::OpenBrace,
            Token::OpenBracket,
            Token::String("a", StringType::Double),
            Token::CloseBracket,
            Token::Eq,
            Token::Number("1"),
            Token::Comma,
            Token::OpenBracket,
            Token::String("b", StringType::Double),
            Token::CloseBracket,
            Token::Eq,
            Token::Number("2"),
            Token::Comma,
            Token::OpenBracket,
            Token::String("c", StringType::Double),
            Token::CloseBracket,
            Token::Eq,
            Token::Number("3"),
            Token::CloseBrace,
            Token::Eof,
        ],
    )
}

#[test]
fn operators() -> Result<()> {
    compare_tokens(
        "+ - * / // ^ % & ~ | << >> .. < <= > >= == ~= and or # not",
        vec![
            Token::Plus,
            Token::Minus,
            Token::Star,
            Token::Slash,
            Token::SlashSlash,
            Token::Hat,
            Token::Percent,
            Token::Amp,
            Token::Tilde,
            Token::Pipe,
            Token::LtLt,
            Token::GtGt,
            Token::DotDot,
            Token::Lt,
            Token::Le,
            Token::Gt,
            Token::Ge,
            Token::EqEq,
            Token::TildeEq,
            Token::And,
            Token::Or,
            Token::Hash,
            Token::Not,
            Token::Eof,
        ],
    )
}

#[test]
fn keywords() -> Result<()> {
    compare_tokens(
        "and break do else elseif end false for function goto if in local nil not or repeat return then true until while",
        vec![
            Token::And,
            Token::Break,
            Token::Do,
            Token::Else,
            Token::ElseIf,
            Token::End,
            Token::False,
            Token::For,
            Token::Function,
            Token::Goto,
            Token::If,
            Token::In,
            Token::Local,
            Token::Nil,
            Token::Not,
            Token::Or,
            Token::Repeat,
            Token::Return,
            Token::Then,
            Token::True,
            Token::Until,
            Token::While,
            Token::Eof,
        ],
    )
}

#[test]
fn html() -> Result<()> {
    compare_tokens(
        "<div class=\"hello\">hello world</div><img src=\"hello.png\"/>",
        vec![
            Token::Lt,
            Token::Identifier("div"),
            Token::Identifier("class"),
            Token::Eq,
            Token::String("hello", StringType::Double),
            Token::Gt,
            Token::Identifier("hello"),
            Token::Identifier("world"),
            Token::OpenClosingTag,
            Token::Identifier("div"),
            Token::Gt,
            Token::Lt,
            Token::Identifier("img"),
            Token::Identifier("src"),
            Token::Eq,
            Token::String("hello.png", StringType::Double),
            Token::Slash,
            Token::Gt,
            Token::Eof,
        ],
    )
}
