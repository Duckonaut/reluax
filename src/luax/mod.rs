use crate::Result;

mod lexer;
mod preprocessor;
#[cfg(test)]
mod tests;
mod tokens;

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
