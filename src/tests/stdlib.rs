use super::*;

#[test]
pub fn test_file_write_and_read() {
    rm_tmp("write_read");
    let mut vm = VirtualMachine::new();
    let path = tmp_file_path("write_read");
    vm.interpret(&format!(
        "import \"std/fs\"; \
         var f = fs.File(\"{path}\", \"w\"); \
         f.write(\"hello world\"); \
         f.close(); \
         var g = fs.File(\"{path}\", \"r\"); \
         print(g.read()); \
         g.close();"
    ))
    .unwrap();
    rm_tmp("write_read");
}


#[test]
pub fn test_file_str() {
    rm_tmp("str_test");
    let mut vm = VirtualMachine::new();
    let path = tmp_file_path("str_test");
    vm.interpret(&format!(
        "import \"std/fs\"; \
         var f = fs.File(\"{path}\", \"w\"); \
         print(str(f)); \
         f.close(); \
         print(str(f));"
    ))
    .unwrap();
    rm_tmp("str_test");
}


#[test]
pub fn test_file_readline() {
    rm_tmp("readline");
    let mut vm = VirtualMachine::new();
    let path = tmp_file_path("readline");
    vm.interpret(&format!(
        "import \"std/fs\"; \
         var f = fs.File(\"{path}\", \"w\"); \
         f.write(\"line1\\nline2\\nline3\"); \
         f.close(); \
         var g = fs.File(\"{path}\", \"r\"); \
         print(g.readline()); \
         print(g.readline()); \
         print(g.readline()); \
         var eof = g.readline(); \
         print(eof == nil); \
         g.close();"
    ))
    .unwrap();
    rm_tmp("readline");
}


#[test]
pub fn test_file_seek_and_tell() {
    rm_tmp("seek_tell");
    let mut vm = VirtualMachine::new();
    let path = tmp_file_path("seek_tell");
    vm.interpret(&format!(
        "import \"std/fs\"; \
         var f = fs.File(\"{path}\", \"w\"); \
         f.write(\"abcdefghij\"); \
         f.close(); \
         var g = fs.File(\"{path}\", \"r\"); \
         print(g.tell()); \
         g.seek(5); \
         print(g.tell()); \
         print(g.read()); \
         g.close();"
    ))
    .unwrap();
    rm_tmp("seek_tell");
}


#[test]
pub fn test_file_not_found_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/fs\"; var f = fs.File(\"/tmp/nonexistent_xyz_12345\", \"r\");").unwrap_err();
    assert!(err.to_string().contains("cannot open"), "got: {err}");
}


#[test]
pub fn test_file_closed_error() {
    rm_tmp("closed_err");
    let mut vm = VirtualMachine::new();
    let path = tmp_file_path("closed_err");
    // Create the file first so we can close it and then try reading.
    vm.interpret(&format!(
        "import \"std/fs\"; \
         var f = fs.File(\"{path}\", \"w\"); \
         f.write(\"data\"); \
         f.close();"
    ))
    .unwrap();
    // Now try reading from a closed file.
    let err = vm
        .interpret(&format!(
            "import \"std/fs\"; \
         var f = fs.File(\"{path}\", \"r\"); \
         f.close(); \
         f.read();"
        ))
        .unwrap_err();
    assert!(err.to_string().contains("file is closed"), "got: {err}");
    rm_tmp("closed_err");
}


#[test]
pub fn test_file_wrong_arg_count() {
    let mut vm = VirtualMachine::new();
    // read() takes no arguments
    let err = vm.interpret("import \"std/fs\"; var f = fs.File(\"/tmp/t\", \"w\"); f.read(\"extra\");").unwrap_err();
    assert!(err.to_string().contains("argument"), "got: {err}");
}

// ===========================================================================
// Std module tests — Math
// ===========================================================================


#[test]
pub fn test_std_math_import_creates_global() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/math\"; print(math);").unwrap();
}


#[test]
pub fn test_std_math_import_as_expression() {
    let mut vm = VirtualMachine::new();
    vm.interpret("var m = import \"std/math\"; print(m); print(m.PI);").unwrap();
}


#[test]
pub fn test_math_constants() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/math\"; print(math.PI); print(math.E); print(math.TAU);").unwrap();
}


#[test]
pub fn test_math_trig() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/math\"; \
         print(math.sin(0)); \
         print(math.cos(0)); \
         print(math.tan(0)); \
         print(math.asin(0)); \
         print(math.acos(1)); \
         print(math.atan(0)); \
         print(math.sin(1.5)); \
         print(math.atan2(1, 1));",
    )
    .unwrap();
}


#[test]
pub fn test_math_power_log() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/math\"; \
         print(math.sqrt(16)); \
         print(math.pow(2, 10)); \
         print(math.exp(1)); \
         print(math.ln(math.E)); \
         print(math.log2(8)); \
         print(math.log10(100)); \
         print(math.hypot(3, 4));",
    )
    .unwrap();
}


#[test]
pub fn test_math_rounding() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/math\"; \
         print(math.floor(3.7)); \
         print(math.ceil(3.1)); \
         print(math.round(3.5));",
    )
    .unwrap();
}


#[test]
pub fn test_math_conversion() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/math\"; \
         print(math.degrees(math.PI)); \
         print(math.radians(180));",
    )
    .unwrap();
}


#[test]
pub fn test_math_accepts_int_args() {
    // All math functions should accept int and treat it as float.
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/math\"; \
         print(math.sin(0)); \
         print(math.sqrt(16)); \
         print(math.pow(2, 10)); \
         print(math.floor(3)); \
         print(math.ceil(3));",
    )
    .unwrap();
}


#[test]
pub fn test_math_type_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/math\"; math.sin(\"hello\");").unwrap_err();
    assert!(err.to_string().contains("unsupported operand"), "got: {err}");
}


#[test]
pub fn test_math_wrong_arg_count() {
    let mut vm = VirtualMachine::new();
    // sin() takes 1 argument
    let err = vm.interpret("import \"std/math\"; math.sin();").unwrap_err();
    assert!(err.to_string().contains("argument"), "got: {err}");

    let mut vm2 = VirtualMachine::new();
    // sqrt() takes 1 argument
    let err2 = vm2.interpret("import \"std/math\"; math.sqrt(4, 5);").unwrap_err();
    assert!(err2.to_string().contains("argument"), "got: {err2}");
}

// ==========================================================================
//  std/random
// ==========================================================================


#[test]
pub fn test_std_random_import_creates_global() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/random\"; print(random);").unwrap();
}


#[test]
pub fn test_std_random_import_as_expression() {
    let mut vm = VirtualMachine::new();
    vm.interpret("var r = import \"std/random\"; print(r); print(r.random());").unwrap();
}


#[test]
pub fn test_random_random_in_range() {
    let mut vm = VirtualMachine::new();
    // random() should return values in [0, 1).
    vm.interpret(
        "import \"std/random\"; \
         var sum = 0.0; \
         for (var i = 0; i < 100; i = i + 1) { \
             var v = random.random(); \
             if v < 0 or v >= 1 { print(\"out of range: \" + str(v)); } \
         }",
    )
    .unwrap();
}


#[test]
pub fn test_random_randint() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/random\"; \
         var v = random.randint(1, 10); \
         if v < 1 or v > 10 { print(\"out of range: \" + str(v)); } \
         var w = random.randint(-5, 5); \
         if w < -5 or w > 5 { print(\"out of range: \" + str(w)); }",
    )
    .unwrap();
}


#[test]
pub fn test_random_randint_min_equals_max() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/random\"; \
         var v = random.randint(7, 7); \
         if v != 7 { print(\"expected 7, got \" + str(v)); }",
    )
    .unwrap();
}


#[test]
pub fn test_random_randint_error_on_float() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/random\"; random.randint(1.5, 10);").unwrap_err();
    assert!(err.to_string().contains("unsupported"), "got: {err}");
}


#[test]
pub fn test_random_randint_error_min_gt_max() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/random\"; random.randint(10, 1);").unwrap_err();
    assert!(err.to_string().contains("must be <="), "got: {err}");
}


#[test]
pub fn test_random_uniform() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/random\"; \
         var v = random.uniform(1.0, 5.0); \
         if v < 1.0 or v >= 5.0 { print(\"out of range: \" + str(v)); } \
         var w = random.uniform(-2.5, 3.5); \
         if w < -2.5 or w >= 3.5 { print(\"out of range: \" + str(w)); }",
    )
    .unwrap();
}


#[test]
pub fn test_random_uniform_accepts_int_args() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/random\"; \
         var v = random.uniform(0, 10); \
         if v < 0 or v >= 10 { print(\"out of range: \" + str(v)); }",
    )
    .unwrap();
}


#[test]
pub fn test_random_choice() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/random\"; \
         var items = [\"a\", \"b\", \"c\"]; \
         var v = random.choice(items); \
         if v != \"a\" and v != \"b\" and v != \"c\" { print(\"unexpected element: \" + str(v)); }",
    )
    .unwrap();
}


#[test]
pub fn test_random_choice_empty_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/random\"; random.choice([]);").unwrap_err();
    assert!(err.to_string().contains("empty"), "got: {err}");
}


#[test]
pub fn test_random_choice_non_list_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/random\"; random.choice(42);").unwrap_err();
    assert!(err.to_string().contains("unsupported"), "got: {err}");
}


#[test]
pub fn test_random_shuffle() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/random\"; \
         var items = [1, 2, 3, 4, 5]; \
         var result = random.shuffle(items); \
         if len(result) != 5 { print(\"wrong length: \" + str(len(result))); }",
    )
    .unwrap();
}

// ===========================================================================
// Break / Continue — runtime tests
// ===========================================================================


#[test]
pub fn test_std_os_import_creates_global() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/os\"; print(os);").unwrap();
}


#[test]
pub fn test_std_os_import_as_expression() {
    let mut vm = VirtualMachine::new();
    vm.interpret("var m = import \"std/os\"; print(m);").unwrap();
}


#[test]
pub fn test_os_pid() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/os\"; var p = os.pid(); print(p > 0);").unwrap();
}


#[test]
pub fn test_os_cwd() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/os\"; print(os.cwd()); var d = os.cwd(); print(len(d) > 0);").unwrap();
}


#[test]
pub fn test_os_getenv() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/os\"; var v = os.getenv(\"PATH\"); print(v != nil);").unwrap();
}


#[test]
pub fn test_os_getenv_missing() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/os\"; var v = os.getenv(\"TARO_NO_SUCH_VAR_XYZ\"); print(v == nil);").unwrap();
}


#[test]
pub fn test_os_setenv_and_getenv() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/os\"; \
         os.setenv(\"TARO_TEST_VAR\", \"hello\"); \
         print(os.getenv(\"TARO_TEST_VAR\") == \"hello\");",
    )
    .unwrap();
}


#[test]
pub fn test_os_env_returns_dict() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/os\"; \
         var e = os.env(); \
         print(type(e) == Dict); \
         print(len(e) > 0);",
    )
    .unwrap();
}


#[test]
pub fn test_os_tmpdir() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/os\"; var t = os.tmpdir(); print(len(t) > 0);").unwrap();
}


#[test]
pub fn test_os_args_returns_list() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/os\"; var a = os.args(); print(type(a) == List);").unwrap();
}

// ===========================================================================
// Std module tests — Time
// ===========================================================================


#[test]
pub fn test_std_time_import_creates_global() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/time\"; print(time);").unwrap();
}


#[test]
pub fn test_std_time_import_as_expression() {
    let mut vm = VirtualMachine::new();
    vm.interpret("var m = import \"std/time\"; print(m);").unwrap();
}


#[test]
pub fn test_time_time_returns_number() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/time\"; \
         var t = time.time(); \
         print(t > 1700000000);", // well after 2023
    )
    .unwrap();
}


#[test]
pub fn test_time_sleep() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/time\"; \
         var t0 = time.time(); \
         time.sleep(0.01); \
         var t1 = time.time(); \
         print(t1 >= t0);",
    )
    .unwrap();
}


#[test]
pub fn test_time_now_returns_object() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/time\"; \
         var n = time.now(); \
         print(n.year > 2023); \
         print(n.month >= 1); \
         print(n.month <= 12); \
         print(n.day >= 1); \
         print(n.day <= 31); \
         print(n.hour >= 0); \
         print(n.hour <= 23); \
         print(n.min >= 0); \
         print(n.min <= 59); \
         print(n.sec >= 0); \
         print(n.sec < 60); \
         print(n.wday >= 0); \
         print(n.wday <= 6); \
         print(n.yday >= 1); \
         print(n.yday <= 366); \
         print(n.timestamp > 1700000000);",
    )
    .unwrap();
}


#[test]
pub fn test_time_sleep_negative_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/time\"; time.sleep(-1);").unwrap_err();
    assert!(err.to_string().contains("negative duration"), "got: {err}");
}

// ===========================================================================
// Std module tests — JSON
// ===========================================================================


#[test]
pub fn test_std_json_import_creates_global() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/json\"; print(json);").unwrap();
}


#[test]
pub fn test_json_encode_nil() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/json\"; print(json.encode(nil) == \"null\");").unwrap();
}


#[test]
pub fn test_json_encode_bool() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         print(json.encode(true) == \"true\"); \
         print(json.encode(false) == \"false\");",
    )
    .unwrap();
}


#[test]
pub fn test_json_encode_int() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/json\"; print(json.encode(42) == \"42\");").unwrap();
}


#[test]
pub fn test_json_encode_float() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/json\"; print(json.encode(3.14) == \"3.14\");").unwrap();
}


#[test]
pub fn test_json_encode_string() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         print(json.encode(\"hello\") == \"\\\"hello\\\"\"); \
         print(json.encode(\"a\\nb\") == \"\\\"a\\\\nb\\\"\");",
    )
    .unwrap();
}


#[test]
pub fn test_json_encode_list() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         print(json.encode([1, 2, 3]) == \"[1,2,3]\"); \
         print(json.encode([\"a\", 1, true]) == \"[\\\"a\\\",1,true]\");",
    )
    .unwrap();
}


#[test]
pub fn test_json_encode_dict() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         var d = dict(); \
         d[\"name\"] = \"taro\"; \
         d[\"ver\"] = 1; \
         var s = json.encode(d); \
         print(s == \"{\\\"name\\\":\\\"taro\\\",\\\"ver\\\":1}\" \
               or s == \"{\\\"ver\\\":1,\\\"name\\\":\\\"taro\\\"}\");",
    )
    .unwrap();
}


#[test]
pub fn test_json_encode_deeply_nested() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         var v = [1, [2, [3, 4]], {\"k\": [5, 6]}]; \
         print(json.encode(v) == \"[1,[2,[3,4]],{\\\"k\\\":[5,6]}]\");",
    )
    .unwrap();
}


#[test]
pub fn test_json_decode_null() {
    let mut vm = VirtualMachine::new();
    vm.interpret("import \"std/json\"; print(json.decode(\"null\") == nil);").unwrap();
}


#[test]
pub fn test_json_decode_bool() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         print(json.decode(\"true\") == true); \
         print(json.decode(\"false\") == false);",
    )
    .unwrap();
}


#[test]
pub fn test_json_decode_number() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         print(json.decode(\"42\") == 42); \
         print(json.decode(\"3.14\") == 3.14); \
         print(json.decode(\"-100\") == -100);",
    )
    .unwrap();
}


#[test]
pub fn test_json_decode_string() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         print(json.decode(\"\\\"hello\\\"\") == \"hello\"); \
         print(json.decode(\"\\\"a\\\\nb\\\"\") == \"a\\nb\");",
    )
    .unwrap();
}


#[test]
pub fn test_json_decode_array() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         var a = json.decode(\"[1, 2, 3]\"); \
         print(len(a) == 3); \
         print(a[0] == 1); \
         print(a[2] == 3);",
    )
    .unwrap();
}


#[test]
pub fn test_json_decode_object() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         var d = json.decode(\"{\\\"x\\\": 10, \\\"y\\\": 20}\"); \
         print(d[\"x\"] == 10); \
         print(d[\"y\"] == 20);",
    )
    .unwrap();
}


#[test]
pub fn test_json_decode_nested() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         var v = json.decode(\"{\\\"items\\\": [1, {\\\"nested\\\": true}]}\"); \
         print(v[\"items\"][0] == 1); \
         print(v[\"items\"][1][\"nested\"] == true);",
    )
    .unwrap();
}


#[test]
pub fn test_json_roundtrip() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "import \"std/json\"; \
         var original = {\"a\": 1, \"b\": [2, 3], \"c\": {\"d\": \"hello\"}}; \
         var encoded = json.encode(original); \
         var decoded = json.decode(encoded); \
         print(decoded[\"a\"] == 1); \
         print(decoded[\"b\"][1] == 3); \
         print(decoded[\"c\"][\"d\"] == \"hello\");",
    )
    .unwrap();
}


#[test]
pub fn test_json_encode_unsupported_type() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/json\"; json.encode(print);").unwrap_err();
    assert!(err.to_string().contains("cannot serialize"), "got: {err}");
}


#[test]
pub fn test_json_decode_invalid() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("import \"std/json\"; json.decode(\"not json\");").unwrap_err();
    assert!(err.to_string().contains("json.decode"), "got: {err}");
}

// ===========================================================================
// Default parameters & keyword arguments
