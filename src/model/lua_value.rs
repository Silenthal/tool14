#[derive(Default, Clone, Debug)]
pub struct LuaFunction {
    pub args: Vec<String>,
    // var_map: HashMap<String, LuaValue>,
}

#[derive(Default, Clone, Debug)]
pub enum LuaValue {
    #[default]
    Nil,
    // Bool(bool), // Not handling these currently
    Integer(i32),
    // Float(f64), // Not handling these currently
    String(String),
    Function(LuaFunction),
    Table(Vec<String>),
    NoSet,
}
