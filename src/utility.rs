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

pub fn clean_quest_name(input: &str) -> String {
    // BF = repeatable
    // BE = quest sync
    input.replace("\u{E0BF} ", "").replace("\u{E0BE} ", "")
}

pub fn clean_output_name(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            other => other,
        })
        .collect()
}

pub fn clean_todo(input: &str) -> String {
    // 3C = HQ
    input.replace("  \u{E03C}", " {{HQ}}")
}

pub fn clean_journal(input: &str) -> String {
    input
        .strip_prefix("\u{E0BE} Quest Sync<br>")
        .unwrap_or(input)
        .to_string()
}
