use ironworks::sestring::format::Value;
use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct Input {
    local: HashMap<u32, Value>,
}

impl Input {
    pub fn new() -> Self {
        Self {
            local: HashMap::new(),
        }
    }

    pub fn add_local_parameter(&mut self, id: u32, value: impl Into<Value>) {
        self.local.insert(id, value.into());
    }

    pub fn with_local_parameter(mut self, id: u32, value: impl Into<Value>) -> Self {
        self.add_local_parameter(id, value);
        self
    }

    pub fn get_local_parameter(&self, key: u32) -> Option<Value> {
        self.local.get(&key).cloned()
    }

    pub fn get_param_count(&self) -> usize {
        self.local.len()
    }
}
