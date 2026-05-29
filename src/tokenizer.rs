use crate::error::MemorsError;

pub fn tokenize(input: &str) -> Result<Vec<String>, MemorsError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for c in input.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
            }

            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }

            _ => {
                current.push(c);
            }
        }
    }

    if in_quotes {
        return Err(MemorsError::InvalidSyntax);
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}
