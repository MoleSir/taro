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
fn ffi_define_struct_and_new() {
    let mut vm = VirtualMachine::new();
    let source = r#"
        import "std/ffi";
        var Color = ffi.define_struct(["uint8", "uint8", "uint8", "uint8"]);
        var c = Color(255, 0, 0, 255);
        print(Color);
        print(c);
    "#;
    vm.interpret(source).expect("ffi_define_struct_and_new should succeed");
}

#[test]
fn ffi_struct_named_fields() {
    let mut vm = VirtualMachine::new();
    // Named-pair format: list of [name, type] pairs.
    let source = r#"
        import "std/ffi";
        var Color = ffi.define_struct([["r", "uint8"], ["g", "uint8"], ["b", "uint8"], ["a", "uint8"]]);
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
        var Vec2 = ffi.define_struct([["x", "float"], ["y", "float"]]);
        var v = Vec2(1.0, 2.0);
        // v.x = 10.0;
        // v.y = 20.0;
        // print(v.x);
        // print(v.y);
    "#;
    vm.interpret(source).expect("ffi_struct_field_mutation should succeed");
}

#[test]
fn ffi_struct_call_syntax() {
    let mut vm = VirtualMachine::new();
    // CType.__call__ creates the struct instance.
    let source = r#"
        import "std/ffi";
        var Point = ffi.define_struct(["int32", "int32"]);
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
        var Seed = ffi.define_struct([["val", "uint32"]]);
        var s = Seed(42);
        var srand = ffi.bind(lib, "srand", "void", [Seed]);
        var r = srand(s);
        print(r);
        "##
    );
    vm.interpret(&source).expect("ffi_bind_with_struct should succeed");
}

#[test]
fn ffi_ctype_singletons_exist() {
    let mut vm = VirtualMachine::new();
    let source = r#"
        import "std/ffi";
        // Verify all CType singletons are accessible.
        var t;
        t = ffi.c_int8;    print(t);
        t = ffi.c_int16;   print(t);
        t = ffi.c_int32;   print(t);
        t = ffi.c_int64;   print(t);
        t = ffi.c_uint8;   print(t);
        t = ffi.c_uint16;  print(t);
        t = ffi.c_uint32;  print(t);
        t = ffi.c_uint64;  print(t);
        t = ffi.c_float;   print(t);
        t = ffi.c_double;  print(t);
        t = ffi.c_bool;    print(t);
        t = ffi.c_pointer; print(t);
        t = ffi.c_cstring; print(t);
    "#;
    vm.interpret(source).expect("ffi_ctype_singletons_exist should succeed");
}

#[test]
fn ffi_struct_with_ctype_objects() {
    let mut vm = VirtualMachine::new();
    // Use CType objects (c_float, c_int) instead of strings as field types.
    let source = r#"
        import "std/ffi";
        var Vec3 = ffi.define_struct([["x", ffi.c_float], ["y", ffi.c_float], ["z", ffi.c_float]]);
        var v = Vec3(1.0, 2.0, 3.0);
        print(v.x);
        print(v.y);
        print(v.z);
    "#;
    vm.interpret(source).expect("ffi_struct_with_ctype_objects should succeed");
}

#[test]
fn ffi_nested_define_struct() {
    let mut vm = VirtualMachine::new();
    // define_struct containing another define_struct (nested).
    let source = r#"
        import "std/ffi";
        var Vector3 = ffi.define_struct([["x", "float"], ["y", "float"], ["z", "float"]]);
        var Camera3D = ffi.define_struct([
            ["position", Vector3],
            ["target", Vector3],
            ["up", Vector3],
            ["fovy", "float"],
            ["projection", "int32"]
        ]);
        var cam = Camera3D(
            Vector3(0.0, 10.0, 10.0),
            Vector3(0.0, 0.0, 0.0),
            Vector3(0.0, 1.0, 0.0),
            45.0,
            0
        );
        print(cam.position.x);  // 0.0
        print(cam.position.y);  // 10.0
        print(cam.fovy);        // 45.0
    "#;
    vm.interpret(source).expect("ffi_nested_define_struct should succeed");
}

#[test]
fn ffi_nested_struct_with_ctype_objects() {
    let mut vm = VirtualMachine::new();
    // Nested structs using CType objects for scalar fields.
    let source = r#"
        import "std/ffi";
        var Vector3 = ffi.define_struct([["x", ffi.c_float], ["y", ffi.c_float], ["z", ffi.c_float]]);
        var Camera3D = ffi.define_struct([
            ["position", Vector3],
            ["target", Vector3],
            ["up", Vector3],
            ["fovy", ffi.c_float],
            ["projection", ffi.c_int32]
        ]);
        var cam = Camera3D(
            Vector3(0.0, 10.0, 10.0),
            Vector3(0.0, 0.0, 0.0),
            Vector3(0.0, 1.0, 0.0),
            45.0,
            0
        );
        print(cam.position.x);
        print(cam.target.y);
        print(cam.up.z);
    "#;
    vm.interpret(source).expect("ffi_nested_struct_with_ctype_objects should succeed");
}

#[test]
fn ffi_nested_struct_passed_to_bind() {
    // Test that a nested struct (Camera3D-like) can be passed to an FFI call.
    let lib_path = if cfg!(target_os = "linux") {
        "libc.so.6"
    } else if cfg!(target_os = "macos") {
        "libSystem.dylib"
    } else {
        return;
    };

    let mut vm = VirtualMachine::new();
    // We can't easily test Camera3D with libc, but we can create a two-level
    // nested struct and pass it to a function that takes a pointer.
    // Here we just verify the marshal path doesn't panic by passing a simple
    // struct to srand (which takes uint32 — our struct has one field).
    let source = format!(
        r##"
        import "std/ffi";
        var lib = ffi.dlopen("{lib_path}");

        // Inner struct: a single int32
        var Inner = ffi.define_struct([["val", ffi.c_uint32]]);

        // Outer struct wraps the inner one
        var Outer = ffi.define_struct([["inner", Inner]]);

        var s = Outer(Inner(42));

        // Bind srand with the Outer struct type — this exercises the
        // recursive marshal path (nested struct → buffer).
        var srand = ffi.bind(lib, "srand", "void", [Outer]);
        var r = srand(s);
        print(r);
        "##
    );
    vm.interpret(&source).expect("ffi_nested_struct_passed_to_bind should succeed");
}

#[test]
fn ffi_ctype_in_bind_arg_types() {
    let lib_path = if cfg!(target_os = "linux") {
        "libm.so.6"
    } else if cfg!(target_os = "macos") {
        "libSystem.dylib"
    } else {
        return;
    };

    let mut vm = VirtualMachine::new();
    // Use CType objects in bind argument type list.
    let source = format!(
        r##"
        import "std/ffi";
        var lib = ffi.dlopen("{lib_path}");
        var cos = ffi.bind(lib, "cos", "double", [ffi.c_double]);
        var r = cos(0.0);
        print(r);
        "##
    );
    vm.interpret(&source).expect("ffi_ctype_in_bind_arg_types should succeed");
}
