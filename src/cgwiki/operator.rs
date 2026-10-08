pub mod operator {
    pub const GE: &str = "GreaterThanOrEqualTo";
    pub const GT: &str = "GreaterThan";
    pub const LE: &str = "LessThanOrEqualTo";
    pub const LT: &str = "LessThan";
    pub const EQ: &str = "Equal";
    pub const NE: &str = "NotEqual";

    pub const VARS: &[&str] = &[GE, GT, LT, LE, EQ, NE];
}
