use crate::{execute::VirtualMachine, Value};
use super::ExecuteResult;

macro_rules! get_args {
    ($vm:ident, $arg_count:ident) => {
        &$vm.stack[$vm.stack.len() - $arg_count..]
    };
}

impl VirtualMachine {
    pub fn print(&mut self, arg_count: usize) -> ExecuteResult<Value> {
        // Copy arguments to avoid borrow conflict with self.str() which
        // now takes &mut self (it may call __str__ on instances).
        let args: Vec<Value> = get_args!(self, arg_count).to_vec();
        for (i, arg) in args.iter().enumerate() {
            if i == 0 {
                print!("{}", self.str(arg)?);
            } else {
                print!(" {}", self.str(arg)?);
            }
        }
        println!("");
        Ok(Value::Nil)
    }
}
