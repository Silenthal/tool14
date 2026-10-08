use ironworks::file::tmb::command::C004;
use std::fmt;
use std::iter::repeat;

#[derive(Default, Debug, Clone)]
pub struct CameraBindingTarget {
    pub target: u32,
    pub target_name: Option<String>,
    pub args: Vec<u32>,
}

impl CameraBindingTarget {
    pub fn new(target: u32, args: Vec<u32>) -> Self {
        CameraBindingTarget {
            target,
            target_name: None,
            args,
        }
    }
}

impl fmt::Display for CameraBindingTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.target == 0xFFFFFFFF {
            write!(f, "nil")
        } else {
            let arg_combo: String = self
                .args
                .iter()
                .map(|a| a.to_string())
                .collect::<Vec<String>>()
                .join(", ");
            match &self.target_name {
                Some(name) => write!(f, "Target(\"{}\", {})", name, arg_combo),
                None => write!(f, "Target(P_{:08X}, {})", self.target, arg_combo),
            }
        }
    }
}

pub fn create_target_list(cmd: &C004) -> Vec<CameraBindingTarget> {
    let mut it = cmd.bindings().iter().copied().chain(repeat(0xFFFFFFFF));
    let mut ret_list = Vec::new();
    let arg_counts = [1, 3, 1, 2, 1, 3];
    for count in arg_counts {
        if let Some(target) = it.next() {
            let mut args = Vec::with_capacity(count as usize);
            for _ in 0..count {
                if let Some(val) = it.next() {
                    args.push(val);
                }
            }
            ret_list.push(CameraBindingTarget::new(target, args));
        }
    }
    return ret_list;
}
