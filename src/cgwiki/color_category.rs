/// Types of color formatting
#[repr(u32)]
#[non_exhaustive]
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub enum ColorCategory {
    #[default]
    None,
    Green,
    Yellow,
    Orange,
    White,
    Ascian,
    Accompany,
    Adjusted,
    QuestSync,
    Hint,
    Unknown(u32),
}

impl From<u32> for ColorCategory {
    fn from(value: u32) -> Self {
        match value {
            0 => ColorCategory::None,
            504 | 505 => ColorCategory::Green,
            506 | 507 => ColorCategory::Yellow,
            500 | 501 => ColorCategory::Orange,
            571 | 572 => ColorCategory::White,
            547 => ColorCategory::Ascian,
            548 => ColorCategory::Accompany,
            533 | 534 => ColorCategory::Adjusted,
            508 | 509 => ColorCategory::QuestSync,
            582 | 581 => ColorCategory::Hint,
            _ => ColorCategory::Unknown(value),
        }
    }
}
