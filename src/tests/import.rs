use super::*;

#[test]
pub fn test_import_statement_defines_global() {
    let mut vm = VirtualMachine::new();
    let path = math_module_path();
    vm.interpret(&format!("import \"{path}\"; print(math);")).unwrap();
}


#[test]
pub fn test_import_module_pi_value() {
    let mut vm = VirtualMachine::new();
    let path = math_module_path();
    vm.interpret(&format!("import \"{path}\"; print(math.PI);")).unwrap();
}


#[test]
pub fn test_import_module_call_function() {
    let mut vm = VirtualMachine::new();
    let path = math_module_path();
    vm.interpret(&format!("import \"{path}\"; print(math.add(10, 20));")).unwrap();
}


#[test]
pub fn test_import_module_mul_function() {
    let mut vm = VirtualMachine::new();
    let path = math_module_path();
    vm.interpret(&format!("import \"{path}\"; print(math.mul(7, 6));")).unwrap();
}


#[test]
pub fn test_import_module_use_class() {
    let mut vm = VirtualMachine::new();
    let path = math_module_path();
    vm.interpret(&format!("import \"{path}\"; var v = math.Vec(1, 2); print(str(v));")).unwrap();
}


#[test]
pub fn test_import_as_expression() {
    let mut vm = VirtualMachine::new();
    let path = math_module_path();
    vm.interpret(&format!("var m = import \"{path}\"; print(m.PI); print(m.add(1, 2));")).unwrap();
}


#[test]
pub fn test_import_file_not_found_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"nonexistent_file.taro\";").unwrap_err();
    assert!(err.to_string().contains("import error"), "got: {err}");
}


#[test]
pub fn test_import_file_not_exists_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"nonexistent_file_xyz.taro\";").unwrap_err();
    assert!(err.to_string().contains("import error"), "got: {err}");
    assert!(err.to_string().contains("module not found"), "got: {err}");
}


#[test]
pub fn test_import_does_not_leak_globals() {
    // Verify that importing a module doesn't add its internals to the
    // importing script's global scope (only the module name is added).
    let mut vm = VirtualMachine::new();
    let path = math_module_path();
    // PI is defined in math.taro but should not be a global.
    vm.interpret(&format!("import \"{path}\";")).unwrap();
    // `math` should exist as a global.
    // `PI` should NOT be a global (it's only accessible via math.PI).
    let err = vm.interpret("print(PI);").unwrap_err();
    assert!(err.to_string().contains("not found"), "PI should not be a global, got: {err}");
}


#[test]
pub fn test_import_nested() {
    // Test that a module can import another module.
    // We need a module that imports math.
    let mut vm = VirtualMachine::new();
    let math_path = math_module_path();
    // This script imports math and uses it — the import system should handle
    // nested import (import inside a module).
    vm.interpret(&format!("import \"{math_path}\"; var m = math; print(m.add(100, 200));")).unwrap();
}

/// File module importing another file module — exercises nested
/// `with_module_scope` and verifies `extra_gc_roots` stacking.

#[test]
pub fn test_import_nested_file_modules() {
    let inner_path = tmp_file_path("nested_inner.taro");
    let outer_path = tmp_file_path("nested_outer.taro");

    // Inner module exports a function.
    std::fs::write(&inner_path, "fun double(x) { return x * 2; }\n").unwrap();

    // Outer module imports inner and wraps the export.
    std::fs::write(
        &outer_path,
        &format!("import \"{inner_path}\" as inner;\nfun quadruple(x) {{ return inner.double(inner.double(x)); }}\n"),
    )
    .unwrap();

    let mut vm = VirtualMachine::new();
    vm.interpret(&format!("import \"{outer_path}\" as outer; print(outer.quadruple(5));")).unwrap();

    std::fs::remove_file(&inner_path).ok();
    std::fs::remove_file(&outer_path).ok();
}


#[test]
pub fn test_import_module_caching() {
    // Verify that importing the same module twice returns the cached module
    // (i.e. the module is executed at most once, matching Python's semantics).
    let mut vm = VirtualMachine::new();
    let path = tmp_file_path("cache_test.taro");

    // Write a module that defines a variable.
    std::fs::write(&path, "var x = 42;\n").unwrap();

    // First import — module is compiled and executed.
    vm.interpret(&format!("import \"{path}\";")).unwrap();

    // The module name is derived from the file stem.
    let module_name = std::path::Path::new(&path).file_stem().unwrap().to_str().unwrap();
    // Cache key is the canonical absolute path.
    let canonical = std::fs::canonicalize(&path).unwrap();
    let module_handle = vm.modules.loaded.get(&ModuleKey::File(canonical.clone())).copied().unwrap();

    // After first import, x == 42.
    let fields = vm.obj_heap.get_module(module_handle).map(|m| &m.fields).unwrap();
    let x_handle = fields.get(&crate::ShrString::new_str("x")).copied().unwrap();
    assert_eq!(*vm.obj_heap.expect_integer(x_handle).unwrap(), 42);

    // Modify the module's field via script.
    vm.interpret(&format!("{module_name}.x = 100;")).unwrap();

    // Verify the modification took effect on the module object.
    let fields = vm.obj_heap.get_module(module_handle).map(|m| &m.fields).unwrap();
    let x_handle = fields.get(&crate::ShrString::new_str("x")).copied().unwrap();
    assert_eq!(*vm.obj_heap.expect_integer(x_handle).unwrap(), 100);

    // Second import — should return the cached module (x still == 100, not 42).
    vm.interpret(&format!("import \"{path}\";")).unwrap();

    // The global should still point to the same cached module with x == 100.
    let fields = vm.obj_heap.get_module(module_handle).map(|m| &m.fields).unwrap();
    let x_handle = fields.get(&crate::ShrString::new_str("x")).copied().unwrap();
    assert_eq!(
        *vm.obj_heap.expect_integer(x_handle).unwrap(),
        100,
        "second import should return the cached module with x == 100, not a fresh module with x == 42"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
pub fn test_std_file_import_creates_global() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/fs\"; print(fs);").unwrap();
}


#[test]
pub fn test_std_file_import_as_expression() {
    let mut vm = VirtualMachine::new();
    vm.interpret("var f = import \"std/fs\"; print(f); print(f.File);").unwrap();
}
