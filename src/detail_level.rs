#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DetailLevel {
    #[default]
    NoScript,
    SomeScript,
    FullScript,
}

impl From<i32> for DetailLevel {
    fn from(value: i32) -> Self {
        match value {
            ..=0 => DetailLevel::NoScript, // intentional
            1 => DetailLevel::NoScript,
            2 => DetailLevel::SomeScript,
            3 => DetailLevel::FullScript,
            4.. => DetailLevel::SomeScript,
        }
    }
}
