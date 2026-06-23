use super::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{ObjectHandle, ShrString};
use std::collections::HashMap;

impl VirtualMachine {
    /// Import a module: read `path`, compile and execute it with isolated
    /// globals, then return a module object containing the top-level
    /// definitions (everything except the builtins).
    pub fn import_module(&mut self, path: &str) -> RuntimeResult<ObjectHandle> {
        // Virtual std/ modules — no file on disk.
        let module = if let Some(module_name) = path.strip_prefix("std/") {
            self.import_std_module(module_name)?
        } else {
            // Read file content.
            let source = std::fs::read_to_string(path).map_err(|e| RuntimeErrorKind::ImportError(format!("cannot read '{path}': {e}")))?;
            self.import_source_module(&source, path)?
        };
        self.loaded_modules.insert(ShrString::new_string(path), module);
        Ok(module)
    }

    /// Compile `source` as a module and execute it in an isolated scope,
    /// returning a module object with the top-level definitions.
    ///
    /// `display_name` is used only in error messages.
    pub(crate) fn import_source_module(&mut self, source: &str, display_name: &str) -> RuntimeResult<ObjectHandle> {
        // 1. Compile the module source (uses local scope so nested functions
        //    capture module-level names as upvalues).
        let function = crate::compile::compile_module(source, &mut self.obj_heap)
            .map_err(|e| RuntimeErrorKind::ImportError(format!("compile error in '{display_name}': {e:?}")))?;

        // 2. Save current VM execution state.
        let saved_frames = std::mem::take(&mut self.frames);
        let saved_stack = std::mem::take(&mut self.stack);
        let saved_globals = std::mem::take(&mut self.globals);
        let saved_upvalues = std::mem::take(&mut self.open_upvalues);
        let saved_gc_threshold = self.gc_threshold;

        // 3. Prevent GC from running while saved state is unreachable from VM roots.
        self.gc_threshold = usize::MAX;

        // 4. Populate extra_gc_roots so the GC (which always runs in test /
        //    gc-stress mode) keeps the importing script's state alive while the
        //    module executes.
        self.extra_gc_roots.clear();
        self.extra_gc_roots.extend_from_slice(&saved_stack);
        for frame in &saved_frames {
            self.extra_gc_roots.push(frame.closure);
        }
        for &handle in saved_globals.values() {
            self.extra_gc_roots.push(handle);
        }
        self.extra_gc_roots.extend_from_slice(&saved_upvalues);

        // 5. Set up fresh globals with builtins only (module top-level
        //    definitions use locals, but builtin functions like `print` still
        //    need to be accessible as globals).
        self.register_builtins();

        // 6. Execute the module function.
        let result = self.interpret_function(function);

        // 7. The module function returns a dict containing its top-level
        //    definitions.  Grab it from the stack before restoring state.
        let exports_dict = self.pop_stack().unwrap_or(ObjectHandle::NIL);

        // 8. Restore original VM state.
        self.frames = saved_frames;
        self.stack = saved_stack;
        self.open_upvalues = saved_upvalues;
        self.globals = saved_globals;
        self.gc_threshold = saved_gc_threshold;
        self.extra_gc_roots.clear();

        // 9. Propagate execution errors.
        result.map_err(|e| RuntimeErrorKind::ImportError(format!("error in module '{display_name}': {e}")))?;

        // 10. Convert the dict into a fields instance.
        let exports: HashMap<ShrString, ObjectHandle> = if let Some(entries) = self.obj_heap.get_dict_instance(exports_dict) {
            entries
                .values()
                .flat_map(|bucket| {
                    bucket.iter().map(|&(k, v)| {
                        let key = self.obj_heap.get_string_instance(k).cloned().unwrap_or_else(|| ShrString::new_str("?"));
                        (key, v)
                    })
                })
                .collect()
        } else {
            HashMap::new()
        };

        // 11. Create module object with exported names as fields.
        let module = self.obj_heap.alloc_fields_instance(self.obj_heap.module_class, exports);
        Ok(module)
    }

    /// Handle virtual std module imports.
    ///
    /// Returns a module instance (Instance with Fields) containing the module's
    /// exports — just like a real file-based module would.  The returned value
    /// is indistinguishable from a compiled `.taro` module.
    pub fn import_std_module(&mut self, module_name: &str) -> RuntimeResult<ObjectHandle> {
        match module_name {
            "argparse" => {
                let source = include_str!("../std/argparse.taro");
                self.import_source_module(source, "std/argparse")
            }
            "logging" => {
                let source = include_str!("../std/logging.taro");
                self.import_source_module(source, "std/logging")
            }
            "ffi" => self.create_ffi_module(),
            "fs" => self.create_fs_module(),
            "itertools" => {
                let source = include_str!("../std/itertools.taro");
                self.import_source_module(source, "std/itertools")
            }
            "json" => self.create_json_module(),
            "math" => self.create_math_module(),
            "net" => self.create_net_module(),
            "os" => self.create_os_module(),
            "random" => self.create_random_module(),
            "time" => self.create_time_module(),
            _ => Err(RuntimeErrorKind::ImportError(format!("unknown std module '{module_name}'")).into()),
        }
    }

    /// Walk from `instance` back through its class to the owning module, then
    /// look up a named export on that module.
    ///
    /// Chain: instance → .class → .module → exports[name]
    /// Returns `None` if any link in the chain is missing.
    pub fn lookup_module_export(&self, instance: ObjectHandle, name: &ShrString) -> Option<ObjectHandle> {
        let inst = self.obj_heap.get_instance(instance)?;
        let class = self.obj_heap.get_class(inst.class)?;
        let module = class.module?;
        let exports = self.obj_heap.get_fields_instance(module)?;
        exports.get(name).copied()
    }

    /// Look up an export from a module that was previously loaded via `import`.
    ///
    /// Used by module-level native functions (which have no `self` receiver)
    /// to find sibling classes within the same module.
    pub fn lookup_loaded_module_export(&self, module_path: &str, name: &ShrString) -> Option<ObjectHandle> {
        let module = self.loaded_modules.get(&ShrString::new_string(module_path))?;
        let exports = self.obj_heap.get_fields_instance(*module)?;
        exports.get(name).copied()
    }
}
