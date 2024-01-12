use crate::luax::{lexer::Lexer, tokens::Token, *};
use color_eyre::Result;

fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut lexer = Lexer::new(input);
    let mut tokens = Vec::new();

    while let Some(token) = lexer.next_token()? {
        tokens.push(token);
    }

    Ok(tokens)
}

fn compare_output(input: &str, expected: &str) -> Result<()> {
    let output = preprocess(input)?;

    // collect the tokens from each string
    let expected_tokens = tokenize(expected)?;
    let output_tokens = tokenize(&output)?;

    if !expected_tokens
        .iter()
        .zip(output_tokens.iter())
        .all(|(a, b)| a == b)
    {
        println!("expected: {:?}", expected_tokens);
        println!("output: {:?}", output_tokens);
        let first_mismatch = expected_tokens
            .iter()
            .zip(output_tokens.iter())
            .enumerate()
            .find(|(_, (a, b))| a != b)
            .unwrap();
        println!(
            "first mismatch: index {}: {:?} != {:?}",
            first_mismatch.0, first_mismatch.1 .0, first_mismatch.1 .1
        );
        println!("expected: {}", expected);
        println!("output: {}", output);
        panic!("output did not match expected");
    }

    Ok(())
}

#[test]
fn empty() -> Result<()> {
    compare_output("", "")
}

#[test]
fn no_preprocessor() -> Result<()> {
    compare_output("hello", "hello")
}

#[test]
fn lua_function() -> Result<()> {
    compare_output("function hello() end", "function hello ( ) end")
}

#[test]
fn lua_function_with_args() -> Result<()> {
    compare_output(
        "function hello(a, b, c) end",
        "function hello ( a , b , c ) end",
    )
}

#[test]
fn variable() -> Result<()> {
    compare_output(
        "local hello = 123 hello = 456",
        "local hello = 123 hello = 456",
    )
}

#[test]
fn call() -> Result<()> {
    compare_output("hello()", "hello ( )")
}

#[test]
fn call_with_args() -> Result<()> {
    compare_output("hello(1, 2, 3)", "hello ( 1 , 2 , 3 )")
}

#[test]
fn call_with_args_and_assign() -> Result<()> {
    compare_output("local a = hello(1, 2, 3)", "local a = hello ( 1 , 2 , 3 )")
}

#[test]
fn call_with_string_arg() -> Result<()> {
    compare_output("hello \"world\"", "hello \"world\"")
}

#[test]
fn call_with_table_arg() -> Result<()> {
    compare_output("hello {1, 2, 3}", "hello { 1 , 2 , 3 }")
}

#[test]
fn long_call_and_access() -> Result<()> {
    compare_output("a.b.c()[1].d()().e", "a . b . c ( ) [ 1 ] . d ( ) ( ) . e")
}

#[test]
fn simple_html() -> Result<()> {
    compare_output(
        "return <div></div>",
        "return { tag=\"div\", attrs={}, children={} }",
    )
}

#[test]
fn html_with_text() -> Result<()> {
    compare_output(
        "return <div>hello</div>",
        "return { tag=\"div\", attrs={}, children={ \"hello\",} }",
    )
}

#[test]
fn html_with_weird_text() -> Result<()> {
    compare_output(
        "return <div>hello #world!</div>",
        "return { tag=\"div\", attrs={}, children={ \"hello #world!\",} }",
    )
}

#[test]
fn html_with_attrs() -> Result<()> {
    compare_output(
        "return <div class=\"hello\" id=\"world\"></div>",
        "return { tag=\"div\", attrs={class=\"hello\", id=\"world\", }, children={} }",
    )
}

#[test]
fn nested_html() -> Result<()> {
    compare_output(
        "return <div><span></span></div>",
        "return { tag=\"div\", attrs={}, children={ { tag=\"span\", attrs={}, children={} },} }",
    )
}

#[test]
fn nested_html_with_text() -> Result<()> {
    compare_output(
        "return <div><span>hello</span></div>",
        "return { tag=\"div\", attrs={}, children={ { tag=\"span\", attrs={}, children={ \"hello\",} },} }",
    )
}

#[test]
fn nested_html_with_attrs() -> Result<()> {
    compare_output(
        "return <div><span class=\"hello\" id=\"world\"></span></div>",
        "return { tag=\"div\", attrs={}, children={ { tag=\"span\", attrs={class=\"hello\", id=\"world\", }, children={} },} }",
    )
}

#[test]
fn self_closing_tag() -> Result<()> {
    compare_output(
        "return <div><span /><span /></div>",
        "return { tag=\"div\", attrs={}, children={ { tag=\"span\", attrs={}, children={} }, { tag=\"span\", attrs={}, children={} },} }",
    )
}

#[test]
fn site() -> Result<()> {
    compare_output(
        r#"
        return <html>
            <head>
                <title>My Site</title>
            </head>
            <body>
                <h1>My Site</h1>
                <p>My site is the best site.</p>
            </body>
        </html>
        "#,
        r#"
        return { tag="html", attrs={}, children={
            { tag="head", attrs={}, children={
                { tag="title", attrs={}, children={ "My Site",} },
            } },
            { tag="body", attrs={}, children={
                { tag="h1", attrs={}, children={ "My Site",} },
                { tag="p", attrs={}, children={ "My site is the best site.",} },
            } },
        } }
        "#,
    )
}

#[test]
fn html_with_code() -> Result<()> {
    compare_output(
        "return <div>{$ hello $}</div>",
        "return { tag=\"div\", attrs={}, children={ hello,} }",
    )
}

#[test]
fn html_with_code_and_text() -> Result<()> {
    compare_output(
        "return <div>hello {$ world $}</div>",
        "return { tag=\"div\", attrs={}, children={ \"hello \", world,} }",
    )
}

#[test]
fn function_returning_html() -> Result<()> {
    compare_output(
        "function hello() return <div></div> end",
        "function hello ( ) return { tag=\"div\", attrs={}, children={} } end",
    )
}

#[test]
fn function_returning_nested_html() -> Result<()> {
    compare_output(
        "function hello() return <div><span></span></div> end",
        "function hello ( ) return { tag=\"div\", attrs={}, children={ { tag=\"span\", attrs={}, children={} },} } end",
    )
}

#[test]
fn function_returning_nested_html_with_code() -> Result<()> {
    compare_output(
        "function hello() return <div>{$ world $}</div> end",
        "function hello ( ) return { tag=\"div\", attrs={}, children={ world,} } end",
    )
}

#[test]
fn component() -> Result<()> {
    compare_output("return <Hello />", "return Hello ({ attrs={}, children={} })")
}

#[test]
fn component_with_attrs() -> Result<()> {
    compare_output(
        "return <Hello name=\"world\" />",
        "return Hello ({ attrs={name=\"world\", }, children={} })",
    )
}

#[test]
fn component_with_children() -> Result<()> {
    compare_output(
        "return <Hello><span /></Hello>",
        "return Hello ({ attrs={}, children={ { tag=\"span\", attrs={}, children={} },} })",
    )
}

#[test]
fn html_with_attrs_with_dash() -> Result<()> {
    compare_output(
        "return <div data-hello=\"world\"></div>",
        "return { tag=\"div\", attrs={[\"data-hello\"]=\"world\", }, children={} }",
    )
}

#[test]
fn weird_symbols_in_html() -> Result<()> {
    compare_output(
        "return <div>@everyone</div>",
        "return { tag=\"div\", attrs={}, children={ \"@everyone\",} }",
    )
}
