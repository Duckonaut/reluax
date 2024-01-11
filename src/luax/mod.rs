use crate::Result;

mod lexer;
mod preprocessor;
mod tokens;

#[derive(Debug)]
pub enum Node<'s> {
    Code(&'s str),
    Text(&'s str),
    Element(Element<'s>),
}

impl<'s> Node<'s> {
    pub fn to_table<W: std::io::Write>(&self, f: &mut W) -> Result<()> {
        match &self {
            Node::Code(s) => write!(f, "{}", s)?,
            Node::Text(s) => write!(f, "\"{}\"", s)?,
            Node::Element(element) => {
                write!(f, "{{")?;
                write!(f, "tag=\"{}\", ", element.tag)?;
                for attribute in &element.attributes {
                    match &attribute.value {
                        Value::Literal(s) => write!(f, "{}=\"{}\", ", attribute.key, s)?,
                        Value::Expression(s) => write!(f, "{}={}, ", attribute.key, s)?,
                    }
                }
                write!(f, "children={{")?;
                for child in &element.children {
                    child.to_table(f)?;
                }
                write!(f, "}}")?;
                write!(f, "}}")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Element<'s> {
    pub tag: &'s str,
    pub attributes: Vec<Attribute<'s>>,
    pub children: Vec<Node<'s>>,
}

#[derive(Debug)]
pub struct Attribute<'s> {
    pub key: &'s str,
    pub value: Value<'s>,
}

#[derive(Debug)]
pub enum Value<'s> {
    Literal(&'s str),
    Expression(&'s str),
}

#[derive(Debug)]
pub struct Template<'s> {
    pub root: Node<'s>,
}

impl<'s> Template<'s> {
    pub fn to_table<W: std::io::Write>(&self, f: &mut W) -> Result<()> {
        self.root.to_table(f)
    }
}

pub fn preprocess(s: &str) -> Result<String> {
    let mut buf = Vec::new();
    let preprocessor = preprocessor::Preprocessor::new(s, &mut buf)?;

    preprocessor.preprocess()?;

    let s = String::from_utf8(buf).unwrap();

    Ok(s)
}

pub fn preprocess_dir(path: &std::path::Path) -> Result<()> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            preprocess_dir(&path)?;
        } else {
            if path.extension().unwrap_or_default() != "luax" {
                continue;
            }
            let s = std::fs::read_to_string(&path)?;
            let s = preprocess(&s)?;

            let out_path = path.with_extension("lua");

            std::fs::write(out_path, s)?;
        }
    }

    Ok(())
}
