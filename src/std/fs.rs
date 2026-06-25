use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{NativeFunction, ObjectHandle, ShrString, impl_object_instance_data};
use std::io::{BufRead, BufReader, Read, Seek, Write};
use super::ModuleBuilder;

impl VirtualMachine {
    /// Create the `fs` std module.
    ///
    /// # Exports
    ///
    /// ## File class
    /// `File(path, mode)` — open / read / write / readline / close / seek / tell
    ///
    /// ## Standalone functions
    /// | function        | description                               |
    /// |-----------------|-------------------------------------------|
    /// | `exists(path)`  | true if the path exists                   |
    /// | `is_file(path)` | true if the path is a regular file        |
    /// | `is_dir(path)`  | true if the path is a directory           |
    /// | `remove(path)`  | delete a file or empty directory          |
    /// | `rename(from,to)`| rename / move a file or directory        |
    /// | `read(path)`    | convenience: open, read all, close        |
    /// | `write(path,s)` | convenience: create (or truncate), write, close |
    /// | `list_dir(path)`| list of entry names in a directory        |
    /// | `mkdir(path)`   | create a directory (including parents)    |
    pub(crate) fn create_fs_module(&mut self) -> RuntimeResult<ObjectHandle> {
        let mut m = ModuleBuilder::new(&mut self.obj_heap, "fs");

        m.define_class("File", |class| {
            class.method("__new__", NativeFunction::var(FileInstance::__new__));
            class.method("__init__", NativeFunction::var(FileInstance::__init__));
            class.method("read", NativeFunction::a1(FileInstance::read));
            class.method("write", NativeFunction::a2(FileInstance::write));
            class.method("readline", NativeFunction::a1(FileInstance::readline));
            class.method("close", NativeFunction::a1(FileInstance::close));
            class.method("seek", NativeFunction::a2(FileInstance::seek));
            class.method("tell", NativeFunction::a1(FileInstance::tell));
            class.method("read_bytes", NativeFunction::a1(FileInstance::read_bytes));
            class.method("__str__", NativeFunction::a1(FileInstance::__str__));
        });

        m.define_fn("exists", NativeFunction::a1(exists));
        m.define_fn("is_file", NativeFunction::a1(is_file));
        m.define_fn("is_dir", NativeFunction::a1(is_dir));
        m.define_fn("remove", NativeFunction::a1(remove));
        m.define_fn("rename", NativeFunction::a2(rename));
        m.define_fn("read", NativeFunction::a1(read));
        m.define_fn("read_bytes", NativeFunction::a1(read_bytes));
        m.define_fn("write", NativeFunction::a2(write));
        m.define_fn("list_dir", NativeFunction::a1(list_dir));
        m.define_fn("mkdir", NativeFunction::a1(mkdir));

        Ok(m.build())
    }
}

// =============================================================================
//  FileInstance — stored in Instance
// =============================================================================

struct FileInstance {
    reader: Option<BufReader<std::fs::File>>,
    path: String,
    mode: String,
}

impl_object_instance_data!(FileInstance, "File");

impl FileInstance {
    fn __new__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        let class = args[0];
        Ok(vm.obj_heap.alloc_instance_dyn(class, Box::new(FileInstance { reader: None, path: String::new(), mode: String::new() })))
    }

    fn __init__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        // args[0] = receiver, args[1] = path, args[2] = optional mode
        let explicit = args.len().saturating_sub(1);
        if explicit < 1 || explicit > 2 {
            return Err(RuntimeErrorKind::ArgumentCountMismatch { expected: 1, got: explicit });
        }
        let self_handle = args[0];
        let path_handle = args[1];
        let mode_handle = args.get(2).copied();

        let path = vm.obj_heap.expect_string(path_handle)?.as_str().to_string();
        let mode = match mode_handle {
            Some(h) => vm.obj_heap.expect_string(h)?.as_str().to_string(),
            None => "r".to_string(),
        };

        let file = match mode.as_str() {
            "r" => std::fs::File::open(&path).map_err(|e| RuntimeErrorKind::IoError(format!("cannot open '{}': {}", path, e)))?,
            "w" => std::fs::File::create(&path).map_err(|e| RuntimeErrorKind::IoError(format!("cannot create '{}': {}", path, e)))?,
            "a" => std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&path)
                .map_err(|e| RuntimeErrorKind::IoError(format!("cannot open '{}': {}", path, e)))?,
            _ => Err(RuntimeErrorKind::IoError(format!("unknown file mode '{}'", mode)))?,
        };

        let inst = vm.obj_heap.get_instance_mut(self_handle).ok_or_else(|| RuntimeErrorKind::IoError("not a File instance".into()))?;
        inst.data = Box::new(FileInstance { reader: Some(BufReader::new(file)), path, mode });

        Ok(self_handle)
    }

    fn read(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let found = vm.obj_heap.type_of(receiver);
        let reader = vm
            .obj_heap
            .get_instance_data_mut::<FileInstance>(receiver)
            .ok_or_else(|| RuntimeErrorKind::TypeMismatch { expected: "native", found })?
            .reader
            .as_mut()
            .ok_or_else(|| RuntimeErrorKind::IoError("file is closed".into()))?;
        let mut buf = String::new();
        reader.read_to_string(&mut buf).map_err(|e| RuntimeErrorKind::IoError(format!("read error: {}", e)))?;
        Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(&buf)))
    }

    fn read_bytes(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let found = vm.obj_heap.type_of(receiver);
        let reader = vm
            .obj_heap
            .get_instance_data_mut::<FileInstance>(receiver)
            .ok_or_else(|| RuntimeErrorKind::TypeMismatch { expected: "native", found })?
            .reader
            .as_mut()
            .ok_or_else(|| RuntimeErrorKind::IoError("file is closed".into()))?;
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).map_err(|e| RuntimeErrorKind::IoError(format!("read error: {}", e)))?;
        Ok(vm.obj_heap.alloc_bytes_instance(buf))
    }

    fn write(vm: &mut VirtualMachine, receiver: ObjectHandle, text: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let text = vm.obj_heap.expect_string(text)?.clone();
        let found = vm.obj_heap.type_of(receiver);
        let reader = vm
            .obj_heap
            .get_instance_data_mut::<FileInstance>(receiver)
            .ok_or_else(|| RuntimeErrorKind::TypeMismatch { expected: "native", found })?
            .reader
            .as_mut()
            .ok_or_else(|| RuntimeErrorKind::IoError("file is closed".into()))?;
        reader.get_mut().write_all(text.as_bytes()).map_err(|e| RuntimeErrorKind::IoError(format!("write error: {}", e)))?;
        Ok(ObjectHandle::NIL)
    }

    fn readline(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let found = vm.obj_heap.type_of(receiver);
        let reader = vm
            .obj_heap
            .get_instance_data_mut::<FileInstance>(receiver)
            .ok_or_else(|| RuntimeErrorKind::TypeMismatch { expected: "native", found })?
            .reader
            .as_mut()
            .ok_or_else(|| RuntimeErrorKind::IoError("file is closed".into()))?;
        let mut line = String::new();
        let n = reader.read_line(&mut line).map_err(|e| RuntimeErrorKind::IoError(format!("read error: {}", e)))?;
        if n == 0 {
            return Ok(ObjectHandle::NIL);
        }
        if line.ends_with('\n') {
            line.pop();
        }
        if line.ends_with('\r') {
            line.pop();
        }
        Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(&line)))
    }

    fn close(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let found = vm.obj_heap.type_of(receiver);
        vm.obj_heap
            .get_instance_data_mut::<FileInstance>(receiver)
            .ok_or_else(|| RuntimeErrorKind::TypeMismatch { expected: "native", found })?
            .reader = None;
        Ok(ObjectHandle::NIL)
    }

    fn seek(vm: &mut VirtualMachine, receiver: ObjectHandle, pos: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let pos = *vm.obj_heap.expect_integer(pos)?;
        let found = vm.obj_heap.type_of(receiver);
        let reader = vm
            .obj_heap
            .get_instance_data_mut::<FileInstance>(receiver)
            .ok_or_else(|| RuntimeErrorKind::TypeMismatch { expected: "native", found })?
            .reader
            .as_mut()
            .ok_or_else(|| RuntimeErrorKind::IoError("file is closed".into()))?;
        reader.seek(std::io::SeekFrom::Start(pos as u64)).map_err(|e| RuntimeErrorKind::IoError(format!("seek error: {}", e)))?;
        Ok(ObjectHandle::NIL)
    }

    fn tell(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let found = vm.obj_heap.type_of(receiver);
        let reader = vm
            .obj_heap
            .get_instance_data_mut::<FileInstance>(receiver)
            .ok_or_else(|| RuntimeErrorKind::TypeMismatch { expected: "native", found })?
            .reader
            .as_mut()
            .ok_or_else(|| RuntimeErrorKind::IoError("file is closed".into()))?;
        let pos = reader.stream_position().map_err(|e| RuntimeErrorKind::IoError(format!("tell error: {}", e)))?;
        Ok(vm.obj_heap.alloc_integer_instance(pos as i64))
    }

    fn __str__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let (is_open, path, mode) = if let Some(d) = vm.obj_heap.get_instance_data::<FileInstance>(receiver) {
            (d.reader.is_some(), d.path.clone(), d.mode.clone())
        } else {
            (false, "?".into(), "?".into())
        };
        let status = if is_open { "open" } else { "closed" };
        Ok(vm
            .obj_heap
            .alloc_string_instance(ShrString::new_string(&format!("<File path='{}' mode='{}' status={}>", path, mode, status))))
    }
}

// =============================================================================
//  Functions
// =============================================================================

fn exists(vm: &mut VirtualMachine, path: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let s = vm.obj_heap.expect_string(path)?;
    let ok = std::path::Path::new(s.as_str()).exists();
    Ok(vm.obj_heap.alloc_bool_instance(ok))
}

fn is_file(vm: &mut VirtualMachine, path: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let s = vm.obj_heap.expect_string(path)?;
    Ok(vm.obj_heap.alloc_bool_instance(std::path::Path::new(s.as_str()).is_file()))
}

fn is_dir(vm: &mut VirtualMachine, path: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let s = vm.obj_heap.expect_string(path)?;
    Ok(vm.obj_heap.alloc_bool_instance(std::path::Path::new(s.as_str()).is_dir()))
}

fn remove(vm: &mut VirtualMachine, path: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let s = vm.obj_heap.expect_string(path)?;
    let p = std::path::Path::new(s.as_str());
    if p.is_dir() { std::fs::remove_dir(p) } else { std::fs::remove_file(p) }
        .map_err(|e| RuntimeErrorKind::IoError(format!("cannot remove '{}': {}", s, e)))?;
    Ok(ObjectHandle::NIL)
}

fn rename(vm: &mut VirtualMachine, from: ObjectHandle, to: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let from_s = vm.obj_heap.expect_string(from)?;
    let to_s = vm.obj_heap.expect_string(to)?;
    std::fs::rename(from_s.as_str(), to_s.as_str()).map_err(|e| RuntimeErrorKind::IoError(format!("cannot rename: {}", e)))?;
    Ok(ObjectHandle::NIL)
}

fn read(vm: &mut VirtualMachine, path: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let s = vm.obj_heap.expect_string(path)?;
    let content = std::fs::read_to_string(s.as_str()).map_err(|e| RuntimeErrorKind::IoError(format!("cannot read '{}': {}", s, e)))?;
    Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(&content)))
}

fn read_bytes(vm: &mut VirtualMachine, path: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let s = vm.obj_heap.expect_string(path)?;
    let content = std::fs::read(s.as_str()).map_err(|e| RuntimeErrorKind::IoError(format!("cannot read '{}': {}", s, e)))?;
    Ok(vm.obj_heap.alloc_bytes_instance(content))
}

fn write(vm: &mut VirtualMachine, path: ObjectHandle, text: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let path_s = vm.obj_heap.expect_string(path)?;
    let text_s = vm.obj_heap.expect_string(text)?;
    std::fs::write(path_s.as_str(), text_s.as_bytes())
        .map_err(|e| RuntimeErrorKind::IoError(format!("cannot write '{}': {}", path_s, e)))?;
    Ok(ObjectHandle::NIL)
}

fn list_dir(vm: &mut VirtualMachine, path: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let s = vm.obj_heap.expect_string(path)?;
    let dir = std::fs::read_dir(s.as_str()).map_err(|e| RuntimeErrorKind::IoError(format!("cannot list '{}': {}", s, e)))?;
    let mut entries = Vec::new();
    for entry in dir {
        let entry = entry.map_err(|e| RuntimeErrorKind::IoError(format!("readdir: {}", e)))?;
        let name = entry.file_name().to_string_lossy().to_string();
        entries.push(vm.obj_heap.alloc_string_instance(ShrString::new_string(&name)));
    }
    Ok(vm.obj_heap.alloc_list_instance(entries))
}

fn mkdir(vm: &mut VirtualMachine, path: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let s = vm.obj_heap.expect_string(path)?;
    std::fs::create_dir_all(s.as_str()).map_err(|e| RuntimeErrorKind::IoError(format!("cannot mkdir '{}': {}", s, e)))?;
    Ok(ObjectHandle::NIL)
}
