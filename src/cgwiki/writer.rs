use ironworks::sestring::{
    Error as SeStringError,
    format::{ColorUsage, Style},
};

use crate::cgwiki::{color_category::ColorCategory, write::Write};

#[derive(Debug, Default)]
pub struct CGWikiWriter {
    pub buffer: String,
    pub is_italic: bool,
    pub is_bold: bool,
    pub is_edge: bool,
    pub is_shadow: bool,
    pub color_category: ColorCategory,
}

impl CGWikiWriter {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            is_italic: false,
            is_bold: false,
            is_edge: true,
            is_shadow: true,
            color_category: ColorCategory::None,
        }
    }

    pub fn reset_style(&mut self) {
        _ = self.set_style(Style::Bold, false);
        _ = self.set_style(Style::Italic, false);
        _ = self.set_style(Style::Outline, true);
        _ = self.set_style(Style::Shadow, true);
        _ = self.pop_color_type(ColorUsage::Edge);
        _ = self.pop_color_type(ColorUsage::Foreground);
        _ = self.pop_color_type(ColorUsage::Shadow);
    }
}

impl Write for CGWikiWriter {
    fn write_str(&mut self, str: &str) -> Result<(), SeStringError> {
        self.buffer.push_str(&str);
        Ok(())
    }

    fn set_style(&mut self, style: Style, _enabled: bool) -> Result<(), SeStringError> {
        match style {
            Style::Bold => {
                if _enabled != self.is_bold {
                    self.is_bold = _enabled;
                    self.buffer.push_str("'''");
                }
            }
            Style::Italic => {
                if _enabled != self.is_italic {
                    self.is_italic = _enabled;
                    self.buffer.push_str("''");
                }
            }
            Style::Outline => {
                if _enabled != self.is_edge {
                    self.is_edge = _enabled;
                }
            }
            Style::Shadow => {
                if _enabled != self.is_edge {
                    self.is_shadow = _enabled;
                }
            }
        }
        Ok(())
    }

    fn write(&mut self, str: String) -> Result<(), SeStringError> {
        self.write_str(&str)
    }

    fn push_color_type(
        &mut self,
        usage: ColorUsage,
        color: super::color_category::ColorCategory,
    ) -> Result<(), SeStringError> {
        if usage == ColorUsage::Foreground {
            match color {
                ColorCategory::Hint => {
                    self.write_str("<span class=\"color-hint\">")?;
                }
                ColorCategory::QuestSync => {
                    self.write_str("<span class=\"color-questsync\">")?;
                }
                ColorCategory::Adjusted => {
                    self.write_str("<span class=\"color-adjusted\">")?;
                }
                ColorCategory::Ascian => {
                    self.write_str("<span class=\"color-ascian\">")?;
                }
                ColorCategory::White => {
                    self.write_str("<span class=\"color-cosmicquest\">")?;
                }
                ColorCategory::Orange => {
                    self.write_str("{{colorize|```")?;
                }
                ColorCategory::Yellow | ColorCategory::Accompany => {
                    self.write_str("{{colorize|``")?;
                }
                ColorCategory::Green => {
                    self.write_str("{{colorize|`")?;
                }
                _ => {}
            }
            self.color_category = color;
        }
        Ok(())
    }

    fn pop_color_type(&mut self, usage: ColorUsage) -> Result<(), SeStringError> {
        if usage == ColorUsage::Foreground {
            match self.color_category {
                ColorCategory::Hint => {
                    self.write_str("</span>")?;
                }
                ColorCategory::QuestSync => {
                    self.write_str("</span>")?;
                }
                ColorCategory::Adjusted => {
                    self.write_str("</span>")?;
                }
                ColorCategory::Ascian => {
                    self.write_str("</span>")?;
                }
                ColorCategory::White => {
                    self.write_str("</span>")?;
                }
                ColorCategory::Orange => {
                    self.write_str("```}}")?;
                }
                ColorCategory::Yellow | ColorCategory::Accompany => {
                    self.write_str("``}}")?;
                }
                ColorCategory::Green => {
                    self.write_str("`}}")?;
                }
                _ => {}
            }
            self.color_category = ColorCategory::None;
        }
        Ok(())
    }
}
