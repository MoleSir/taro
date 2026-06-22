//! FFI module tests.

use crate::vm::VirtualMachine;

#[test]
fn ffi_import_module() {
    let mut vm = VirtualMachine::new();
    vm.interpret(r#"import "std/ffi";"#).unwrap();
}

#[test]
fn ffi_dlopen_nonexistent_library() {
    let mut vm = VirtualMachine::new();
    let result = vm.interpret(
        r#"
        import "std/ffi";
        ffi.dlopen("/nonexistent/lib_does_not_exist.so");
        "#,
    );
    assert!(result.is_err(), "dlopen of nonexistent library should fail");
}

#[test]
fn ffi_libm_cos() {
    let lib_path = if cfg!(target_os = "linux") {
        "libm.so.6"
    } else if cfg!(target_os = "macos") {
        "libSystem.dylib"
    } else {
        return;
    };

    let mut vm = VirtualMachine::new();
    let source = format!(
        r##"
        import "std/ffi";
        var lib = ffi.dlopen("{lib_path}");
        var cos = ffi.dlsym(lib, "cos");
        var r = ffi.call(cos, "double", ["double"], [0.0]);
        print(r);
        "##
    );
    vm.interpret(&source).expect("ffi_libm_cos should succeed");
}

#[test]
fn ffi_call_void_return() {
    let lib_path = if cfg!(target_os = "linux") {
        "libc.so.6"
    } else if cfg!(target_os = "macos") {
        "libSystem.dylib"
    } else {
        return;
    };

    let mut vm = VirtualMachine::new();
    let source = format!(
        r##"
        import "std/ffi";
        var lib = ffi.dlopen("{lib_path}");
        var srand = ffi.dlsym(lib, "srand");
        var result = ffi.call(srand, "void", ["uint32"], [42]);
        print(result);
        "##
    );
    vm.interpret(&source).expect("ffi_call_void_return should succeed");
}

#[test]
fn ffi_struct_def_and_new() {
    let mut vm = VirtualMachine::new();
    let source = r#"
        import "std/ffi";
        var Color = ffi.struct_def(["uint8", "uint8", "uint8", "uint8"]);
        var c = ffi.struct_new(Color, [255, 0, 0, 255]);
        print(c);
    "#;
    vm.interpret(source).expect("ffi_struct_def_and_new should succeed");
}

#[test]
fn ffi_struct_named_fields() {
    let mut vm = VirtualMachine::new();
    // Named-pair format: list of [name, type] pairs.
    let source = r#"
        import "std/ffi";
        var Color = ffi.struct_def([["r", "uint8"], ["g", "uint8"], ["b", "uint8"], ["a", "uint8"]]);
        var c = Color(255, 0, 128, 255);
        print(c.r);
        print(c.g);
        print(c.b);
        print(c.a);
    "#;
    vm.interpret(source).expect("ffi_struct_named_fields should succeed");
}

#[test]
fn ffi_struct_field_mutation() {
    let mut vm = VirtualMachine::new();
    let source = r#"
        import "std/ffi";
        var Vec2 = ffi.struct_def([["x", "float"], ["y", "float"]]);
        var v = Vec2(1.0, 2.0);
        v.x = 10.0;
        v.y = 20.0;
        print(v.x);
        print(v.y);
    "#;
    vm.interpret(source).expect("ffi_struct_field_mutation should succeed");
}

#[test]
fn ffi_struct_call_syntax() {
    let mut vm = VirtualMachine::new();
    // StructDef.__call__ creates the instance.
    let source = r#"
        import "std/ffi";
        var Point = ffi.struct_def(["int32", "int32"]);
        var p = Point(100, 200);
        print(p);
    "#;
    vm.interpret(source).expect("ffi_struct_call_syntax should succeed");
}

#[test]
fn ffi_bind_cos() {
    let lib_path = if cfg!(target_os = "linux") {
        "libm.so.6"
    } else if cfg!(target_os = "macos") {
        "libSystem.dylib"
    } else {
        return;
    };

    let mut vm = VirtualMachine::new();
    let source = format!(
        r##"
        import "std/ffi";
        var lib = ffi.dlopen("{lib_path}");
        var cos = ffi.bind(lib, "cos", "double", ["double"]);
        var r = cos(0.0);
        print(r);
        "##
    );
    vm.interpret(&source).expect("ffi_bind_cos should succeed");
}

#[test]
fn ffi_bind_abs() {
    let lib_path = if cfg!(target_os = "linux") {
        "libc.so.6"
    } else if cfg!(target_os = "macos") {
        "libSystem.dylib"
    } else {
        return;
    };

    let mut vm = VirtualMachine::new();
    let source = format!(
        r##"
        import "std/ffi";
        var lib = ffi.dlopen("{lib_path}");
        var abs = ffi.bind(lib, "abs", "int32", ["int32"]);
        var r = abs(-42);
        print(r);
        "##
    );
    vm.interpret(&source).expect("ffi_bind_abs should succeed");
}

#[test]
fn ffi_bind_void_return() {
    let lib_path = if cfg!(target_os = "linux") {
        "libc.so.6"
    } else if cfg!(target_os = "macos") {
        "libSystem.dylib"
    } else {
        return;
    };

    let mut vm = VirtualMachine::new();
    let source = format!(
        r##"
        import "std/ffi";
        var lib = ffi.dlopen("{lib_path}");
        var srand = ffi.bind(lib, "srand", "void", ["uint32"]);
        var r = srand(42);
        print(r);
        "##
    );
    vm.interpret(&source).expect("ffi_bind_void_return should succeed");
}

#[test]
fn ffi_bind_with_struct() {
    let lib_path = if cfg!(target_os = "linux") {
        "libc.so.6"
    } else if cfg!(target_os = "macos") {
        "libSystem.dylib"
    } else {
        return;
    };

    let mut vm = VirtualMachine::new();
    // srand takes a single uint32 — use a positional struct with one field.
    let source = format!(
        r##"
        import "std/ffi";
        var lib = ffi.dlopen("{lib_path}");
        var Seed = ffi.struct_def([["val", "uint32"]]);
        var s = Seed(42);
        var srand = ffi.bind(lib, "srand", "void", [Seed]);
        var r = srand(s);
        print(r);
        "##
    );
    vm.interpret(&source).expect("ffi_bind_with_struct should succeed");
}
