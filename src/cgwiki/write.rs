use ironworks::sestring::{
    Error as SeStringError,
    format::{ColorUsage, Style},
};

use crate::cgwiki::color_category::ColorCategory;

pub trait Write {
    fn write(&mut self, str: String) -> Result<(), SeStringError> {
        self.write_str(&str)
    }

    fn write_str(&mut self, str: &str) -> Result<(), SeStringError>;

    fn set_style(&mut self, style: Style, enabled: bool) -> Result<(), SeStringError> {
        let _ = (style, enabled);
        Ok(())
    }

    fn push_color_type(
        &mut self,
        usage: ColorUsage,
        color: ColorCategory,
    ) -> Result<(), SeStringError> {
        let _ = (usage, color);
        Ok(())
    }

    fn pop_color_type(&mut self, usage: ColorUsage) -> Result<(), SeStringError> {
        let _ = usage;
        Ok(())
    }
}
