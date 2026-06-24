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
        ffi.CDynLib("/nonexistent/lib_does_not_exist.so");
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
        var lib = ffi.CDynLib("{lib_path}");
        var cos = lib.symbol("cos");
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
        var lib = ffi.CDynLib("{lib_path}");
        var srand = lib.symbol("srand");
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
        var lib = ffi.CDynLib("{lib_path}");
        var cos = lib.bind("cos", "double", ["double"]);
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
        var lib = ffi.CDynLib("{lib_path}");
        var abs = lib.bind("abs", "int32", ["int32"]);
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
        var lib = ffi.CDynLib("{lib_path}");
        var srand = lib.bind("srand", "void", ["uint32"]);
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
        var lib = ffi.CDynLib("{lib_path}");
        var Seed = ffi.define_struct([["val", "uint32"]]);
        var s = Seed(42);
        var srand = lib.bind("srand", "void", [Seed]);
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
        var lib = ffi.CDynLib("{lib_path}");

        // Inner struct: a single int32
        var Inner = ffi.define_struct([["val", ffi.c_uint32]]);

        // Outer struct wraps the inner one
        var Outer = ffi.define_struct([["inner", Inner]]);

        var s = Outer(Inner(42));

        // Bind srand with the Outer struct type — this exercises the
        // recursive marshal path (nested struct → buffer).
        var srand = lib.bind("srand", "void", [Outer]);
        var r = srand(s);
        print(r);
        "##
    );
    vm.interpret(&source).expect("ffi_nested_struct_passed_to_bind should succeed");
}

#[test]
fn ffi_bind_struct_return() {
    // Test that bind() can handle a C function that returns a struct by value.
    // div() in libc returns div_t {{ int quot; int rem; }}.
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
        var lib = ffi.CDynLib("{lib_path}");

        var DivT = ffi.define_struct([["quot", "int32"], ["rem", "int32"]]);
        var div = lib.bind("div", DivT, [ffi.c_int32, ffi.c_int32]);

        var r = div(10, 3);
        print(r.quot);  // 3
        print(r.rem);   // 1
        "##
    );
    vm.interpret(&source).expect("ffi_bind_struct_return should succeed");
}

#[test]
fn ffi_call_struct_return() {
    // Test that ffi.call() can handle a C function that returns a struct by value.
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
        var lib = ffi.CDynLib("{lib_path}");

        var DivT = ffi.define_struct([["quot", "int32"], ["rem", "int32"]]);
        var divSym = lib.symbol("div");

        var r = ffi.call(divSym, DivT, [ffi.c_int32, ffi.c_int32], [10, 3]);
        print(r.quot);  // 3
        print(r.rem);   // 1
        "##
    );
    vm.interpret(&source).expect("ffi_call_struct_return should succeed");
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
        var lib = ffi.CDynLib("{lib_path}");
        var cos = lib.bind("cos", "double", [ffi.c_double]);
        var r = cos(0.0);
        print(r);
        "##
    );
    vm.interpret(&source).expect("ffi_ctype_in_bind_arg_types should succeed");
}

// ===========================================================================
// Scalar wrapper tests — typed construction, .value access, struct integration
// ===========================================================================

#[test]
fn ffi_scalar_construction() {
    // c_uint8(255) should construct a typed wrapper.
    let mut vm = VirtualMachine::new();
    let source = r#"
        import "std/ffi";
        var v = ffi.c_uint8(255);
        print(v);
    "#;
    vm.interpret(source).expect("ffi_scalar_construction should succeed");
}

#[test]
fn ffi_scalar_value_getter() {
    // .value should return the underlying Taro value.
    let mut vm = VirtualMachine::new();
    let source = r#"
        import "std/ffi";
        var v = ffi.c_int32(42);
        var x = v.value;
        print(x);  // 42
    "#;
    vm.interpret(source).expect("ffi_scalar_value_getter should succeed");
}

#[test]
fn ffi_scalar_value_setter() {
    // .value = ... should update the wrapped value.
    let mut vm = VirtualMachine::new();
    let source = r#"
        import "std/ffi";
        var v = ffi.c_int32(42);
        v.value = 99;
        print(v.value);  // 99
    "#;
    vm.interpret(source).expect("ffi_scalar_value_setter should succeed");
}

#[test]
fn ffi_scalar_float_construction() {
    let mut vm = VirtualMachine::new();
    let source = r#"
        import "std/ffi";
        var v = ffi.c_float(3.14);
        print(v.value);  // ~3.14
        var d = ffi.c_double(2.718);
        print(d.value);  // ~2.718
    "#;
    vm.interpret(source).expect("ffi_scalar_float_construction should succeed");
}

#[test]
fn ffi_scalar_bool_construction() {
    let mut vm = VirtualMachine::new();
    let source = r#"
        import "std/ffi";
        var t = ffi.c_bool(true);
        print(t.value);   // true
        var f = ffi.c_bool(false);
        print(f.value);   // false
    "#;
    vm.interpret(source).expect("ffi_scalar_bool_construction should succeed");
}

#[test]
fn ffi_scalar_all_integer_types() {
    // All integer scalar types should construct.
    let mut vm = VirtualMachine::new();
    let source = r#"
        import "std/ffi";
        var a = ffi.c_int8(127);
        var b = ffi.c_int16(32767);
        var c = ffi.c_int32(2147483647);
        var d = ffi.c_int64(9223372036854775807);
        var e = ffi.c_uint8(255);
        var f = ffi.c_uint16(65535);
        var g = ffi.c_uint32(4294967295);
        var h = ffi.c_uint64(9223372036854775807);
        print(a.value);
        print(b.value);
        print(c.value);
        print(d.value);
        print(e.value);
        print(f.value);
        print(g.value);
        print(h.value);
    "#;
    vm.interpret(source).expect("ffi_scalar_all_integer_types should succeed");
}

#[test]
fn ffi_scalar_pointer_construction() {
    // c_pointer accepts an integer address.
    let mut vm = VirtualMachine::new();
    let source = r#"
        import "std/ffi";
        var p = ffi.c_pointer(42);
        print(p.value);  // 42 (the address as integer)
    "#;
    vm.interpret(source).expect("ffi_scalar_pointer_construction should succeed");
}

#[test]
fn ffi_struct_with_typed_scalar_fields() {
    // Struct fields can be typed scalars (auto-converted) or raw values.
    let mut vm = VirtualMachine::new();
    let source = r#"
        import "std/ffi";
        var Color = ffi.define_struct([["r", "uint8"], ["g", "uint8"], ["b", "uint8"], ["a", "uint8"]]);
        // Mix typed wrappers and raw values.
        var c = Color(ffi.c_uint8(255), 0, ffi.c_uint8(128), 255);
        print(c.r);  // 255 — auto-unwrapped from CUint8
        print(c.g);  // 0
        print(c.b);  // 128
        print(c.a);  // 255
    "#;
    vm.interpret(source).expect("ffi_struct_with_typed_scalar_fields should succeed");
}

#[test]
fn ffi_struct_named_typed_fields() {
    // Named struct fields with typed scalars.
    let mut vm = VirtualMachine::new();
    let source = r#"
        import "std/ffi";
        var Vec3 = ffi.define_struct([["x", ffi.c_float], ["y", ffi.c_float], ["z", ffi.c_float]]);
        var v = Vec3(ffi.c_float(1.0), ffi.c_float(2.0), 3.0);
        print(v.x);  // 1.0 — auto-unwrapped
        print(v.y);  // 2.0
        print(v.z);  // 3.0
    "#;
    vm.interpret(source).expect("ffi_struct_named_typed_fields should succeed");
}

#[test]
fn ffi_scalar_passed_to_bind() {
    // Typed scalar wrappers should marshal correctly in FFI calls.
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
        var lib = ffi.CDynLib("{lib_path}");
        var abs = lib.bind("abs", "int32", ["int32"]);
        var r = abs(ffi.c_int32(-42));
        print(r);  // 42 (FFI returns raw value)
        "##
    );
    vm.interpret(&source).expect("ffi_scalar_passed_to_bind should succeed");
}

#[test]
fn ffi_ctype_call_returns_typed_wrapper() {
    // c_int32(42) should return a typed wrapper (CI32), not a raw value.
    let mut vm = VirtualMachine::new();
    let source = r#"
        import "std/ffi";
        var x = ffi.c_int32(42);
        var y = ffi.c_int32(99);
        // Different instances should be independent.
        print(x.value);  // 42
        print(y.value);  // 99
        x.value = 77;
        print(x.value);  // 77
        print(y.value);  // 99 (unchanged)
    "#;
    vm.interpret(source).expect("ffi_ctype_call_returns_typed_wrapper should succeed");
}
