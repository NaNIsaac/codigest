//! Token estimate: about four characters per token, counted in characters like Python's
//! `len(str)`, so that the budget needs no tokenizer dependency.

pub fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}
