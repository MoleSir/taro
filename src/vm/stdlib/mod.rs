mod fs;
mod math;
mod random;

use super::{ExecuteError, ExecuteResult, VirtualMachine};
use crate::ObjectHandle;

impl VirtualMachine {
    /// Handle virtual std module imports.
    ///
    /// Returns a module instance (Instance with Fields) containing the module's
    /// exports — just like a real file-based module would.  The returned value
    /// is indistinguishable from a compiled `.taro` module.
    pub fn import_std_module(&mut self, module_name: &str) -> ExecuteResult<ObjectHandle> {
        match module_name {
            "fs" => self.create_fs_module(),
            "math" => self.create_math_module(),
            "random" => self.create_random_module(),
            _ => Err(ExecuteError::ImportError(format!(
                "unknown std module '{module_name}'"
            ))),
        }
    }
}
