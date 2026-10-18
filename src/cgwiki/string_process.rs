use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use regex::Regex;
use std::borrow::Cow;
use std::sync::LazyLock;

const REPLACES: &[(&str, &str)] = &[
    ("─", "—"),
    ("–", "-"),
    ("“", "\""),
    ("”", "\""),
    // (" ", ""), // Repeatable
    // (" ", ""), // Level sync
    ("<Global(1)>", "[Forename] [Surname]"),
    (
        "[Limsa GC Title][Gridania GC Title][Ul'dah GC Title]",
        "[GC Title]",
    ),
    (
        "<IfLimsaGC>[Limsa GC Rank]</IfLimsaGC><IfGridaniaGC>[Gridania GC Rank]</IfGridaniaGC><IfUldahGC>[Ul'dah GC Rank]</IfUldahGC>",
        "[GC Rank]",
    ),
    (
        "<IfLimsaGC>[Limsa GC Title]<Else><IfGridaniaGC>[Gridania GC Title]<Else>[Ul'dah GC Title]</IfGridaniaGC></IfLimsaGC>",
        "[GC Title]",
    ),
    (
        "<IfLimsaGC>[Limsa GC Title] </IfLimsaGC><IfGridaniaGC>[Gridania GC Title] </IfGridaniaGC><IfUldahGC>[Ul'dah GC Title] </IfUldahGC>",
        "[GC Title] ",
    ),
    (
        "<IfLimsaGC>[Limsa GC Title] [Surname]</IfLimsaGC><IfGridaniaGC>[Gridania GC Title] [Surname]</IfGridaniaGC><IfUldahGC>[Ul'dah GC Title] [Surname]</IfUldahGC>",
        "[GC Title] [Surname]",
    ),
    (
        "<IfLimsaGC>[Forename][Limsa GC Title]</IfLimsaGC><IfGridaniaGC>[Forename][Gridania GC Title]</IfGridaniaGC><IfUldahGC>[Forename][Ul'dah GC Title]</IfUldahGC>",
        "[Forename][GC Title]",
    ),
    (
        "<IfLimsaGC>[Surname][Limsa GC Title]</IfLimsaGC><IfGridaniaGC>[Surname][Gridania GC Title]</IfGridaniaGC><IfUldahGC>[Surname][Ul'dah GC Title]</IfUldahGC>",
        "[Surname][GC Title]",
    ),
    (
        "<IfLimsaGC><Head([Limsa GC Title])></IfLimsaGC><IfGridaniaGC><Head([Gridania GC Title])></IfGridaniaGC><IfUldahGC><Head([Ul'dah GC Title])></IfUldahGC>",
        "<Head([GC Title])>",
    ),
    (
        "<IfLimsaGC><Head([Limsa GC Title])> </IfLimsaGC><IfGridaniaGC><Head([Gridania GC Title])> </IfGridaniaGC><IfUldahGC><Head([Ul'dah GC Title])> </IfUldahGC>",
        "<Head([GC Title])> ",
    ),
    (
        "<IfLimsaGC><Head([Limsa GC Title])> [Surname]</IfLimsaGC><IfGridaniaGC><Head([Gridania GC Title])> [Surname]</IfGridaniaGC><IfUldahGC><Head([Ul'dah GC Title])> [Surname]</IfUldahGC>",
        "<Head([GC Title])> [Surname]",
    ),
];

static LOOKUP: LazyLock<AhoCorasick> = LazyLock::new(|| {
    let replace_list: Vec<&str> = REPLACES.iter().map(|(from, _)| *from).collect();
    AhoCorasickBuilder::new()
        .build(replace_list)
        .expect("Failed to create character replacement dictionary")
});

static REGEX_TIME_OF_DAY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<If\(LessThan\(<Global\(11\)>,12\)\)><If\(LessThan\(<Global\(11\)>,4\)\)>(?<pre_dawn>.*)<Else>(?<morning>.*)<\/If><Else><If\(LessThan\(<Global\(11\)>,17\)\)>(?<day>.*)<Else>(?<pre_midnight>.*)<\/If><\/If>").unwrap()
});

pub fn clean_str(input: &str) -> Cow<'_, str> {
    let mut output = String::new();
    let mut replaced = false;
    if REGEX_TIME_OF_DAY.is_match(&input) {
        let replace_res: std::borrow::Cow<'_, str> =
            REGEX_TIME_OF_DAY.replace_all(&input, |cap: &regex::Captures| {
                let pre_dawn = &cap["pre_dawn"];
                let pre_mid = &cap["pre_midnight"];
                let morning = &cap["morning"];
                let day = &cap["day"];
                if pre_dawn == pre_mid {
                    format!(
                        "{{{{alternatives|morning={}|day={}|night={}}}}}",
                        morning, day, pre_mid
                    )
                } else {
                    cap[0].to_string()
                }
            });
        output = replace_res.to_string();
        replaced = true;
    }

    let mut matches = LOOKUP.find_iter(input).peekable();

    if matches.peek().is_none() {
        if replaced {
            return Cow::Owned(output);
        } else {
            return Cow::Borrowed(input);
        }
    }

    // Most strings expected to be the same size - line breaks aren't too common
    output = String::with_capacity(input.len());
    let mut last_end = 0;

    for found in matches {
        let start = found.start();
        let end = found.end();

        if start < last_end {
            continue;
        }

        output.push_str(&input[last_end..start]);
        let to = REPLACES[found.pattern()].1;
        output.push_str(to);
        last_end = end;
    }

    output.push_str(&input[last_end..]);

    Cow::Owned(output)
}
