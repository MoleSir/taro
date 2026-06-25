use super::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{ObjectHandle, ShrString};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ============================================================================
// Types
// ============================================================================

/// Unified cache key for loaded modules.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub(crate) enum ModuleKey {
    /// A native Rust module, identified by its full import path (e.g. `std/math`).
    Native(ShrString),
    /// A file-based module, identified by its canonical absolute path.
    File(PathBuf),
}

/// Function signature for a native module factory.
pub(crate) type NativeLoader = fn(&mut VirtualMachine) -> RuntimeResult<ObjectHandle>;

/// Owns the module cache, file search paths, and native-module registry.
pub(crate) struct Modules {
    /// Loaded module cache, keyed by [`ModuleKey`].
    pub loaded: HashMap<ModuleKey, ObjectHandle>,
    /// Directories searched (in order) when resolving `import "name"`.
    pub search_paths: Vec<PathBuf>,
    /// Registry of native std module factories, keyed by the module name
    /// *without* the `"std/"` prefix (e.g. `"math"`, `"fs"`).
    pub native: HashMap<&'static str, NativeLoader>,
}

impl Modules {
    /// Build the default module system with all native std modules registered.
    pub fn default() -> Self {
        let mut m = Self {
            loaded: HashMap::new(),
            search_paths: VirtualMachine::default_search_paths(),
            native: HashMap::new(),
        };

        // Register every native std module.
        // Adding a new one is just one line here + its create_*_module impl.
        m.native.insert("ffi", VirtualMachine::create_ffi_module as NativeLoader);
        m.native.insert("fs", VirtualMachine::create_fs_module as NativeLoader);
        m.native.insert("json", VirtualMachine::create_json_module as NativeLoader);
        m.native.insert("math", VirtualMachine::create_math_module as NativeLoader);
        m.native.insert("net", VirtualMachine::create_net_module as NativeLoader);
        m.native.insert("os", VirtualMachine::create_os_module as NativeLoader);
        m.native.insert("random", VirtualMachine::create_random_module as NativeLoader);
        m.native.insert("time", VirtualMachine::create_time_module as NativeLoader);

        m
    }

    /// Try to locate a `.taro` file for the given import path.
    ///
    /// If `path` looks like a filesystem path (has a parent component or
    /// starts with `.`) it is tried relative to CWD.  Otherwise each directory
    /// in `search_paths` is searched.
    ///
    /// Returns the canonical absolute [`PathBuf`] of the first matching file,
    /// or `None` if no file is found.
    fn resolve_file_path(&self, path: &str) -> Option<PathBuf> {
        /// Check `candidate` is a regular file and return its canonical path.
        fn try_canonical(candidate: &Path) -> Option<PathBuf> {
            let meta = std::fs::metadata(candidate).ok()?;
            if !meta.is_file() {
                return None;
            }
            std::fs::canonicalize(candidate).ok()
        }

        let import_path = Path::new(path);
        // Only treat as a filesystem path when it explicitly starts with
        // `.` or `/`.  Paths like `std/itertools` are logical module paths
        // that should be searched in every search directory.
        let is_filesystem_path = path.starts_with('.') || path.starts_with('/');
        let has_taro_ext = import_path.extension().map_or(false, |e| e == "taro");

        let search_dirs: Vec<&Path> = if is_filesystem_path {
            vec![Path::new(".")]
        } else {
            self.search_paths.iter().map(|d| d.as_path()).collect()
        };

        for dir in search_dirs {
            let mut candidate = PathBuf::from(dir);
            candidate.push(import_path);

            if let Some(found) = try_canonical(&candidate) {
                return Some(found);
            }
            if !has_taro_ext {
                candidate.set_extension("taro");
                if let Some(found) = try_canonical(&candidate) {
                    return Some(found);
                }
            }
        }

        None
    }
}

// ============================================================================
// Import orchestration
// ============================================================================

impl VirtualMachine {
    /// Import a module identified by `path`.
    ///
    /// The lookup order is:
    /// 1. Native std modules (paths starting with `"std/"`)
    /// 2. File-based modules (searched in [`Modules::search_paths`])
    ///
    /// Already-loaded modules are returned from the cache.
    pub fn import_module(&mut self, path: &str) -> RuntimeResult<ObjectHandle> {
        // ── native std modules ───────────────────────────────────────────
        if let Some(module_name) = path.strip_prefix("std/") {
            if let Some(&loader) = self.modules.native.get(module_name) {
                let key = ModuleKey::Native(ShrString::new_string(path));
                if let Some(&cached) = self.modules.loaded.get(&key) {
                    return Ok(cached);
                }
                let module = loader(self)?;
                self.modules.loaded.insert(key, module);
                return Ok(module);
            }
        }

        // ── file-based modules ───────────────────────────────────────────
        let resolved = self
            .modules
            .resolve_file_path(path)
            .ok_or_else(|| RuntimeErrorKind::ImportError(format!("module not found: '{path}'")))?;

        let key = ModuleKey::File(resolved.clone());
        if let Some(&cached) = self.modules.loaded.get(&key) {
            return Ok(cached);
        }

        let source = std::fs::read_to_string(&resolved).map_err(|e| {
            RuntimeErrorKind::ImportError(format!("cannot read '{}': {e}", resolved.display()))
        })?;
        let display = resolved.display().to_string();
        let module = self.import_source_module(&source, &display)?;
        self.modules.loaded.insert(key, module);
        Ok(module)
    }

    /// Compile `source` as a module and execute it in an isolated scope,
    /// returning a module object containing the top-level definitions.
    ///
    /// Module semantics are handled at runtime: the module's closure gets
    /// `.module` set to the module object, and `DefineGlobal` /
    /// `GetGlobal` operate on that module's fields (just like the root
    /// `__main__` module does for directly executed scripts).
    ///
    /// `display_name` is used only in error messages.
    pub(crate) fn import_source_module(&mut self, source: &str, display_name: &str) -> RuntimeResult<ObjectHandle> {
        // 1. Compile normally — definitions become DefineGlobal instructions.
        let function = crate::compile::compile(source, &mut self.obj_heap)
            .map_err(|e| RuntimeErrorKind::ImportError(format!("compile error in '{display_name}': {e:?}")))?;

        // 2. Create an empty module object with a meaningful name.
        //    DefineGlobal will populate its fields during execution.
        let module = self.obj_heap.alloc_module(ShrString::new_string(display_name));

        // Keep the module alive across GC cycles during execution.
        self.extra_gc_roots.push(module);

        // 3. Execute in an isolated scope with the module context set.
        let result = self.with_module_scope(|vm| {
            let closure = vm.obj_heap.alloc_closure(function, module);
            vm.reset();
            vm.push_stack(closure);
            vm.call_closure(closure, 0, true).expect("can't fail in script call");
            vm.run()
        });

        self.extra_gc_roots.pop();

        result.map_err(|e| RuntimeErrorKind::ImportError(format!("error in module '{display_name}': {e}")))?;

        Ok(module)
    }
}

// ============================================================================
// Module-scope isolation
// ============================================================================

/// Snapshot of VM execution state saved during module import.
struct ModuleScope {
    frames: Vec<super::CallFrame>,
    stack: Vec<ObjectHandle>,
    open_upvalues: Vec<ObjectHandle>,
    // globals are always stored in closure.module.fields — no separate
    // globals map exists to save/restore.
}

impl VirtualMachine {
    /// Execute `f` in an isolated module scope.
    ///
    /// The VM's current execution state (frames, stack, upvalues) is saved,
    /// `f` is called, and then the original state is restored unconditionally.
    ///
    /// Global operations (`GetGlobal` / `DefineGlobal`) always operate on
    /// `closure.module.fields`, so there is no separate globals map to
    /// save/restore.  Builtins are always available via `self.builtins`
    /// fallback.
    ///
    /// GC is allowed to run during module execution; the saved importing state
    /// is kept alive via `extra_gc_roots`.  Nested imports are supported — each
    /// level pushes its saved state onto `extra_gc_roots` and pops it on exit.
    fn with_module_scope<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        // ── save ─────────────────────────────────────────────────────────
        let saved = ModuleScope {
            frames: std::mem::take(&mut self.frames),
            stack: std::mem::take(&mut self.stack),
            open_upvalues: std::mem::take(&mut self.open_upvalues),
        };

        // Keep the importing state alive during GC.
        let prev_root_count = self.extra_gc_roots.len();
        self.extra_gc_roots.extend_from_slice(&saved.stack);
        for frame in &saved.frames {
            self.extra_gc_roots.push(frame.closure);
        }
        self.extra_gc_roots.extend_from_slice(&saved.open_upvalues);

        // ── execute ──────────────────────────────────────────────────────
        let result = f(self);

        // ── restore ──────────────────────────────────────────────────────
        self.frames = saved.frames;
        self.stack = saved.stack;
        self.open_upvalues = saved.open_upvalues;

        // Pop our roots, leaving any outer scope's roots in place.
        self.extra_gc_roots.truncate(prev_root_count);

        result
    }
}

// ============================================================================
// Intra-module lookups (used by native methods to find sibling classes)
// ============================================================================

impl VirtualMachine {
    /// Walk from `instance` back through its class to the owning module, then
    /// look up a named export on that module.
    ///
    /// Chain: instance → .class → .module → exports[name]
    /// Returns `None` if any link in the chain is missing.
    pub fn lookup_module_export(&self, instance: ObjectHandle, name: &str) -> Option<ObjectHandle> {
        let inst = self.obj_heap.get_instance(instance)?;
        let class = self.obj_heap.get_class(inst.class)?;
        let mod_obj = self.obj_heap.get_module(class.module)?;
        mod_obj.fields.get(name).copied()
    }

    /// Look up an export from a module that was previously loaded via `import`.
    ///
    /// `module_path` is the original import path (e.g. `"std/ffi"`).
    /// Returns `None` if the module hasn't been loaded yet.
    pub fn lookup_loaded_module_export(&self, module_path: &str, name: &str) -> Option<ObjectHandle> {
        let key = ModuleKey::Native(ShrString::new_string(module_path));
        let module = self.modules.loaded.get(&key)?;
        let mod_obj = self.obj_heap.get_module(*module)?;
        mod_obj.fields.get(name).copied()
    }
}
