pub fn head(input: &str) -> String {
    let mut chars = input.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

pub fn head_all(input: &str) -> String {
    let ignore = ["of", "the", "a"];

    input
        .split_whitespace()
        .enumerate()
        .map(|(i, word)| {
            if i == 0 {
                head(word)
            } else if ignore.contains(&word) {
                word.to_string()
            } else {
                head(word)
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}
