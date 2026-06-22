use crate::object::ObjectDict;
use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{NativeFunction, ObjectHandle, ShrString};
use std::collections::HashMap;
use std::process::Command;

impl VirtualMachine {
    /// Create the `os` std module.
    ///
    /// # Exports
    ///
    /// | name            | description                                  |
    /// |-----------------|----------------------------------------------|
    /// | `args`          | list of command-line arguments               |
    /// | `getenv(name)`  | get environment variable (nil if not set)    |
    /// | `setenv(k, v)`  | set environment variable                     |
    /// | `env()`         | dict of all environment variables            |
    /// | `cwd()`         | current working directory                    |
    /// | `chdir(path)`   | change working directory                     |
    /// | `pid`           | current process ID                           |
    /// | `tmpdir()`      | path to the system temp directory            |
    /// | `system(cmd)`   | run a shell command, return exit code        |
    pub(crate) fn create_os_module(&mut self) -> RuntimeResult<ObjectHandle> {
        // ---- function handles ----
        let args_fn = self.obj_heap.alloc_native_fn("args", NativeFunction::a0(args));
        let getenv_fn = self.obj_heap.alloc_native_fn("getenv", NativeFunction::a1(getenv));
        let setenv_fn = self.obj_heap.alloc_native_fn("setenv", NativeFunction::a2(setenv));
        let env_fn = self.obj_heap.alloc_native_fn("env", NativeFunction::a0(env));
        let cwd_fn = self.obj_heap.alloc_native_fn("cwd", NativeFunction::a0(cwd));
        let chdir_fn = self.obj_heap.alloc_native_fn("chdir", NativeFunction::a1(chdir));
        let pid_fn = self.obj_heap.alloc_native_fn("pid", NativeFunction::a0(pid));
        let tmpdir_fn = self.obj_heap.alloc_native_fn("tmpdir", NativeFunction::a0(tmpdir));
        let system_fn = self.obj_heap.alloc_native_fn("system", NativeFunction::a1(system));

        // ---- assemble module ----
        let mut exports: HashMap<ShrString, ObjectHandle> = HashMap::new();
        exports.insert(ShrString::new_str("args"), args_fn);
        exports.insert(ShrString::new_str("getenv"), getenv_fn);
        exports.insert(ShrString::new_str("setenv"), setenv_fn);
        exports.insert(ShrString::new_str("env"), env_fn);
        exports.insert(ShrString::new_str("cwd"), cwd_fn);
        exports.insert(ShrString::new_str("chdir"), chdir_fn);
        exports.insert(ShrString::new_str("pid"), pid_fn);
        exports.insert(ShrString::new_str("tmpdir"), tmpdir_fn);
        exports.insert(ShrString::new_str("system"), system_fn);

        let module = self.obj_heap.alloc_fields_instance(self.obj_heap.module_class, exports);
        Ok(module)
    }
}

// =====================================================================
//  Function implementations
// =====================================================================

/// `os.args()` — return the script's command-line arguments as a list.
///
/// Returns all arguments from `std::env::args()`, skipping the first one
/// (the executable name).
fn args(vm: &mut VirtualMachine) -> RuntimeResult<ObjectHandle> {
    let items: Vec<ObjectHandle> = std::env::args().skip(1).map(|a| vm.obj_heap.alloc_string_instance(ShrString::new_string(&a))).collect();
    Ok(vm.obj_heap.alloc_list_instance(items))
}

/// `os.getenv(name)` — return the value of environment variable `name`,
/// or nil if it is not set.
fn getenv(vm: &mut VirtualMachine, name: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let key = vm.get_string_instance(name)?;
    match std::env::var(key.as_str()) {
        Ok(val) => Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(&val))),
        Err(_) => Ok(ObjectHandle::NIL),
    }
}

/// `os.setenv(key, value)` — set an environment variable.
fn setenv(vm: &mut VirtualMachine, key: ObjectHandle, value: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let k = vm.get_string_instance(key)?;
    let v = vm.get_string_instance(value)?;
    // SAFETY: single-threaded VM — no concurrent env access.
    unsafe { std::env::set_var(k.as_str(), v.as_str()) };
    Ok(ObjectHandle::NIL)
}

/// `os.env()` — return a dict of all environment variables.
fn env(vm: &mut VirtualMachine) -> RuntimeResult<ObjectHandle> {
    let dict_handle = vm.obj_heap.alloc_dict_instance(HashMap::new());
    for (key, val) in std::env::vars() {
        let k = vm.obj_heap.alloc_string_instance(ShrString::new_string(&key));
        let v = vm.obj_heap.alloc_string_instance(ShrString::new_string(&val));
        ObjectDict::__setitem__(vm, dict_handle, k, v)?;
    }
    Ok(dict_handle)
}

/// `os.cwd()` — return the current working directory.
fn cwd(vm: &mut VirtualMachine) -> RuntimeResult<ObjectHandle> {
    let path = std::env::current_dir().map_err(|e| RuntimeErrorKind::OsError(format!("cannot get cwd: {}", e)))?;
    Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(path.to_string_lossy().into_owned())))
}

/// `os.chdir(path)` — change the current working directory.
fn chdir(vm: &mut VirtualMachine, path: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let s = vm.get_string_instance(path)?;
    std::env::set_current_dir(s.as_str()).map_err(|e| RuntimeErrorKind::OsError(format!("cannot chdir to '{}': {}", s, e)))?;
    Ok(ObjectHandle::NIL)
}

/// `os.pid` — return the current process ID as an integer.
fn pid(vm: &mut VirtualMachine) -> RuntimeResult<ObjectHandle> {
    Ok(vm.obj_heap.alloc_integer_instance(std::process::id() as i64))
}

/// `os.tmpdir()` — return the path to the system temporary directory.
fn tmpdir(vm: &mut VirtualMachine) -> RuntimeResult<ObjectHandle> {
    Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(std::env::temp_dir().to_string_lossy().into_owned())))
}

/// `os.system(command)` — run a shell command and return its exit code.
fn system(vm: &mut VirtualMachine, cmd: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let s = vm.get_string_instance(cmd)?;

    let status = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", s.as_str()]).status()
    } else {
        Command::new("sh").args(["-c", s.as_str()]).status()
    }
    .map_err(|e| RuntimeErrorKind::OsError(format!("system: failed to run '{}': {}", s, e)))?;

    Ok(vm.obj_heap.alloc_integer_instance(status.code().unwrap_or(-1) as i64))
}
