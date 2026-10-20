#[derive(Default, Debug)]
pub struct ReplaceResult {
    pub handover: Vec<String>,
    pub script: Vec<String>,
}

impl ReplaceResult {
    pub fn new() -> Self {
        Self {
            handover: Vec::new(),
            script: Vec::new(),
        }
    }
}
