use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Seek, Write};
use crate::{ToNativeData, NativeFunction, NativeData, ObjectHandle, ObjectInstanceData, ShrString};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

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
    pub(crate) fn create_fs_module(&mut self) -> ExecuteResult<ObjectHandle> {
        let file_class = self.obj_heap.alloc_class("File");

        self.register_native_method(file_class, "__init__",  NativeFunction::var(StdFileData::__init__));
        self.register_native_method(file_class, "read",      NativeFunction::a1(StdFileData::read));
        self.register_native_method(file_class, "write",     NativeFunction::a2(StdFileData::write));
        self.register_native_method(file_class, "readline",  NativeFunction::a1(StdFileData::readline));
        self.register_native_method(file_class, "close",     NativeFunction::a1(StdFileData::close));
        self.register_native_method(file_class, "seek",      NativeFunction::a2(StdFileData::seek));
        self.register_native_method(file_class, "tell",      NativeFunction::a1(StdFileData::tell));
        self.register_native_method(file_class, "__str__",   NativeFunction::a1(StdFileData::__str__));

        // Standalone function handles.
        let exists = self.obj_heap.alloc_native_fn("exists", NativeFunction::a1(exists));
        let is_file = self.obj_heap.alloc_native_fn("is_file", NativeFunction::a1(is_file));
        let is_dir = self.obj_heap.alloc_native_fn("is_dir", NativeFunction::a1(is_dir));
        let remove = self.obj_heap.alloc_native_fn("remove", NativeFunction::a1(remove));
        let rename = self.obj_heap.alloc_native_fn("rename", NativeFunction::a2(rename));
        let read = self.obj_heap.alloc_native_fn("read", NativeFunction::a1(read));
        let write = self.obj_heap.alloc_native_fn("write", NativeFunction::a2(write));
        let list_dir = self.obj_heap.alloc_native_fn("list_dir", NativeFunction::a1(list_dir));
        let mkdir = self.obj_heap.alloc_native_fn("mkdir", NativeFunction::a1(mkdir));

        let mut exports: HashMap<ShrString, ObjectHandle> = HashMap::new();
        exports.insert(ShrString::new_str("File"), file_class);
        exports.insert(ShrString::new_str("exists"), exists);
        exports.insert(ShrString::new_str("is_file"), is_file);
        exports.insert(ShrString::new_str("is_dir"), is_dir);
        exports.insert(ShrString::new_str("remove"), remove);
        exports.insert(ShrString::new_str("rename"), rename);
        exports.insert(ShrString::new_str("read"), read);
        exports.insert(ShrString::new_str("write"), write);
        exports.insert(ShrString::new_str("list_dir"), list_dir);
        exports.insert(ShrString::new_str("mkdir"), mkdir);

        let module = self.obj_heap.alloc_fields_instance(self.obj_heap.module_class);
        if let Some(inst) = self.obj_heap.get_instance_mut(module) {
            if let ObjectInstanceData::Fields(fields) = &mut inst.data {
                *fields = exports;
            }
        }

        Ok(module)
    }
}

// =============================================================================
//  StdFileData — stored in Instance via ObjectInstanceData::Native
// =============================================================================

struct StdFileData {
    reader: Option<BufReader<std::fs::File>>,
    path: String,
    mode: String,
}

impl ToNativeData for StdFileData {}

impl StdFileData {
    fn __init__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> ExecuteResult<ObjectHandle> {
        // args[0] = receiver, args[1] = path, args[2] = optional mode
        let explicit = args.len().saturating_sub(1);
        if explicit < 1 || explicit > 2 {
            return Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: explicit });
        }
        let self_handle = args[0];
        let path_handle = args[1];
        let mode_handle = args.get(2).copied();

        let path = vm.get_string_instance(path_handle)?.as_str().to_string();
        let mode = match mode_handle {
            Some(h) => vm.get_string_instance(h)?.as_str().to_string(),
            None => "r".to_string(),
        };

        let file = match mode.as_str() {
            "r" => std::fs::File::open(&path)
                .map_err(|e| ExecuteError::IoError(format!("cannot open '{}': {}", path, e)))?,
            "w" => std::fs::File::create(&path)
                .map_err(|e| ExecuteError::IoError(format!("cannot create '{}': {}", path, e)))?,
            "a" => std::fs::OpenOptions::new()
                .append(true).create(true).open(&path)
                .map_err(|e| ExecuteError::IoError(format!("cannot open '{}': {}", path, e)))?,
            _ => Err(ExecuteError::IoError(format!("unknown file mode '{}'", mode)))?,
        };

        let inst = vm.obj_heap.get_instance_mut(self_handle)
            .ok_or_else(|| ExecuteError::IoError("not a File instance".into()))?;
        inst.data = ObjectInstanceData::Native(
            NativeData::new(StdFileData { reader: Some(BufReader::new(file)), path, mode }),
        );

        Ok(self_handle)
    }

    fn read(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let reader = vm.get_native_mut::<StdFileData>(receiver)?.reader.as_mut()
            .ok_or_else(|| ExecuteError::IoError("file is closed".into()))?;
        let mut buf = String::new();
        reader.read_to_string(&mut buf)
            .map_err(|e| ExecuteError::IoError(format!("read error: {}", e)))?;
        Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(&buf)))
    }

    fn write(vm: &mut VirtualMachine, receiver: ObjectHandle, text: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let text = vm.get_string_instance(text)?.clone();
        let reader = vm.get_native_mut::<StdFileData>(receiver)?.reader.as_mut()
            .ok_or_else(|| ExecuteError::IoError("file is closed".into()))?;
        reader.get_mut().write_all(text.as_bytes())
            .map_err(|e| ExecuteError::IoError(format!("write error: {}", e)))?;
        Ok(ObjectHandle::NIL)
    }

    fn readline(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let reader = vm.get_native_mut::<StdFileData>(receiver)?.reader.as_mut()
            .ok_or_else(|| ExecuteError::IoError("file is closed".into()))?;
        let mut line = String::new();
        let n = reader.read_line(&mut line)
            .map_err(|e| ExecuteError::IoError(format!("read error: {}", e)))?;
        if n == 0 {
            return Ok(ObjectHandle::NIL);
        }
        if line.ends_with('\n') { line.pop(); }
        if line.ends_with('\r') { line.pop(); }
        Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(&line)))
    }

    fn close(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        vm.get_native_mut::<StdFileData>(receiver)?.reader = None;
        Ok(ObjectHandle::NIL)
    }

    fn seek(vm: &mut VirtualMachine, receiver: ObjectHandle, pos: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let pos = *vm.get_integer_instance(pos)?;
        let reader = vm.get_native_mut::<StdFileData>(receiver)?.reader.as_mut()
            .ok_or_else(|| ExecuteError::IoError("file is closed".into()))?;
        reader.seek(std::io::SeekFrom::Start(pos as u64))
            .map_err(|e| ExecuteError::IoError(format!("seek error: {}", e)))?;
        Ok(ObjectHandle::NIL)
    }

    fn tell(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let reader = vm.get_native_mut::<StdFileData>(receiver)?.reader.as_mut()
            .ok_or_else(|| ExecuteError::IoError("file is closed".into()))?;
        let pos = reader.stream_position()
            .map_err(|e| ExecuteError::IoError(format!("tell error: {}", e)))?;
        Ok(vm.obj_heap.alloc_integer_instance(pos as i64))
    }

    fn __str__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let (is_open, path, mode) =
            if let Some(d) = vm.obj_heap.get_native::<StdFileData>(receiver) {
                (d.reader.is_some(), d.path.clone(), d.mode.clone())
            } else {
                (false, "?".into(), "?".into())
            };
        let status = if is_open { "open" } else { "closed" };
        Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(&format!(
            "<File path='{}' mode='{}' status={}>", path, mode, status
        ))))
    }
}

// =============================================================================
//  Functions
// =============================================================================
  
fn exists(vm: &mut VirtualMachine, path: ObjectHandle) -> ExecuteResult<ObjectHandle> {
    let s = vm.get_string_instance(path)?;
    let ok = std::path::Path::new(s.as_str()).exists();
    Ok(vm.obj_heap.alloc_bool_instance(ok))
}

fn is_file(vm: &mut VirtualMachine, path: ObjectHandle) -> ExecuteResult<ObjectHandle> {
    let s = vm.get_string_instance(path)?;
    Ok(vm.obj_heap.alloc_bool_instance(std::path::Path::new(s.as_str()).is_file()))
}

fn is_dir(vm: &mut VirtualMachine, path: ObjectHandle) -> ExecuteResult<ObjectHandle> {
    let s = vm.get_string_instance(path)?;
    Ok(vm.obj_heap.alloc_bool_instance(std::path::Path::new(s.as_str()).is_dir()))
}

fn remove(vm: &mut VirtualMachine, path: ObjectHandle) -> ExecuteResult<ObjectHandle> {
    let s = vm.get_string_instance(path)?;
    let p = std::path::Path::new(s.as_str());
    if p.is_dir() { std::fs::remove_dir(p) } else { std::fs::remove_file(p) }
        .map_err(|e| ExecuteError::IoError(format!("cannot remove '{}': {}", s, e)))?;
    Ok(ObjectHandle::NIL)
}

fn rename(vm: &mut VirtualMachine, from: ObjectHandle, to: ObjectHandle) -> ExecuteResult<ObjectHandle> {
    let from_s = vm.get_string_instance(from)?;
    let to_s = vm.get_string_instance(to)?;
    std::fs::rename(from_s.as_str(), to_s.as_str())
        .map_err(|e| ExecuteError::IoError(format!("cannot rename: {}", e)))?;
    Ok(ObjectHandle::NIL)
}

fn read(vm: &mut VirtualMachine, path: ObjectHandle) -> ExecuteResult<ObjectHandle> {
    let s = vm.get_string_instance(path)?;
    let content = std::fs::read_to_string(s.as_str())
        .map_err(|e| ExecuteError::IoError(format!("cannot read '{}': {}", s, e)))?;
    Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(&content)))
}

fn write(vm: &mut VirtualMachine, path: ObjectHandle, text: ObjectHandle) -> ExecuteResult<ObjectHandle> {
    let path_s = vm.get_string_instance(path)?;
    let text_s = vm.get_string_instance(text)?;
    std::fs::write(path_s.as_str(), text_s.as_bytes())
        .map_err(|e| ExecuteError::IoError(format!("cannot write '{}': {}", path_s, e)))?;
    Ok(ObjectHandle::NIL)
}

fn list_dir(vm: &mut VirtualMachine, path: ObjectHandle) -> ExecuteResult<ObjectHandle> {
    let s = vm.get_string_instance(path)?;
    let dir = std::fs::read_dir(s.as_str())
        .map_err(|e| ExecuteError::IoError(format!("cannot list '{}': {}", s, e)))?;
    let mut entries = Vec::new();
    for entry in dir {
        let entry = entry.map_err(|e| ExecuteError::IoError(format!("readdir: {}", e)))?;
        let name = entry.file_name().to_string_lossy().to_string();
        entries.push(vm.obj_heap.alloc_string_instance(ShrString::new_string(&name)));
    }
    Ok(vm.obj_heap.alloc_list_instance(entries))
}

fn mkdir(vm: &mut VirtualMachine, path: ObjectHandle) -> ExecuteResult<ObjectHandle> {
    let s = vm.get_string_instance(path)?;
    std::fs::create_dir_all(s.as_str())
        .map_err(|e| ExecuteError::IoError(format!("cannot mkdir '{}': {}", s, e)))?;
    Ok(ObjectHandle::NIL)
}
