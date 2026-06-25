use super::*;

#[test]
pub fn test_string_concat() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("hello".into())), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance(" world".into())), 1, 1, h);
        c.write_instruction(Instruction::Add, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "hello world");
}


#[test]
pub fn test_string_equality() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("abc".into())), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("abc".into())), 1, 1, h);
        c.write_instruction(Instruction::Equal, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert!({
        let r = vm.pop_stack().unwrap();
        get_bool(&vm, r)
    });
}


#[test]
pub fn test_string_not_equality() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("abc".into())), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("xyz".into())), 1, 1, h);
        c.write_instruction(Instruction::NotEqual, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert!({
        let r = vm.pop_stack().unwrap();
        get_bool(&vm, r)
    });
}


#[test]
pub fn test_len_string() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::GetGlobal("len".into()), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("hello".into())), 1, 1, h);
        c.write_instruction(Instruction::Call(1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert_eq!(
        {
            let r = vm.pop_stack().unwrap();
            get_int(&vm, r)
        },
        5
    );
}


#[test]
pub fn test_build_list() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(1)), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(2)), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(3)), 1, 1, h);
        c.write_instruction(Instruction::BuildList(3), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(vm.obj_heap.get_list_instance(r).unwrap().len(), 3);
}


#[test]
pub fn test_list_index_get() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(10)), 1, 1, h);
        c.write_instruction(Instruction::BuildList(1), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(0)), 1, 1, h);
        c.write_instruction(Instruction::IndexGet, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert_eq!(
        {
            let r = vm.pop_stack().unwrap();
            get_int(&vm, r)
        },
        10
    );
}


#[test]
pub fn test_list_index_set() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(10)), 1, 1, h);
        c.write_instruction(Instruction::BuildList(1), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(0)), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::IndexSet, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}


#[test]
pub fn test_list_append_method() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(1)), 1, 1, h);
        c.write_instruction(Instruction::BuildList(1), 1, 1, h);
        c.write_instruction(Instruction::GetProperty("append".into()), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(2)), 1, 1, h);
        c.write_instruction(Instruction::Call(1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert_eq!(
        {
            let r = vm.pop_stack().unwrap();
            get_int(&vm, r)
        },
        2
    );
}


#[test]
pub fn test_build_dict() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("a".into())), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(1)), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("b".into())), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(2)), 1, 1, h);
        c.write_instruction(Instruction::BuildDict(2), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    let entry_count: usize = vm.obj_heap.get_dict_instance(r).unwrap().values().map(|b| b.len()).sum();
    assert_eq!(entry_count, 2);
}


#[test]
pub fn test_dict_index_get() {
    let mut vm = run_chunk(|c, h| {
        let k = h.alloc_string_instance("x".into());
        c.write_instruction(Instruction::Constant(k), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::BuildDict(1), 1, 1, h);
        c.write_instruction(Instruction::Constant(k), 1, 1, h);
        c.write_instruction(Instruction::IndexGet, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}


#[test]
pub fn test_dict_get_method() {
    let mut vm = run_chunk(|c, h| {
        let k = h.alloc_string_instance("x".into());
        c.write_instruction(Instruction::Constant(k), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::BuildDict(1), 1, 1, h);
        c.write_instruction(Instruction::GetProperty("get".into()), 1, 1, h);
        c.write_instruction(Instruction::Constant(k), 1, 1, h);
        c.write_instruction(Instruction::Call(1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}


#[test]
pub fn test_dict_get_missing() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("x".into())), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::BuildDict(1), 1, 1, h);
        c.write_instruction(Instruction::GetProperty("get".into()), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("y".into())), 1, 1, h);
        c.write_instruction(Instruction::Call(1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    assert!({
        let r = vm.pop_stack().unwrap();
        is_nil(r)
    });
}


#[test]
pub fn test_dict_keys_method() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_string_instance("a".into())), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(1)), 1, 1, h);
        c.write_instruction(Instruction::BuildDict(1), 1, 1, h);
        c.write_instruction(Instruction::GetProperty("keys".into()), 1, 1, h);
        c.write_instruction(Instruction::Call(0), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(vm.obj_heap.get_list_instance(r).unwrap().len(), 1);
}


#[test]
pub fn test_dict_pop_method() {
    let mut vm = run_chunk(|c, h| {
        let k = h.alloc_string_instance("x".into());
        c.write_instruction(Instruction::Constant(k), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::BuildDict(1), 1, 1, h);
        c.write_instruction(Instruction::GetProperty("pop".into()), 1, 1, h);
        c.write_instruction(Instruction::Constant(k), 1, 1, h);
        c.write_instruction(Instruction::Call(1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}


#[test]
pub fn test_list_nested() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var matrix = [[1, 2], [3, 4]];
        print(matrix[0][0]);  // 1
        print(matrix[1][1]);  // 4
        matrix[0][1] = 99;
        print(matrix[0][1]);  // 99
    ",
    )
    .unwrap();
}

/// Dict with mixed key types (string only — cross-type eq not yet supported).

#[test]
pub fn test_dict_mixed_values() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var d = {\"int\": 1, \"str\": \"two\", \"bool\": true};
        print(d[\"int\"]);   // 1
        print(d[\"str\"]);   // two
        print(d[\"bool\"]);  // true
        print(len(d));       // 3
    ",
    )
    .unwrap();
}

/// Dict builtin methods: keys(), get(), values().

#[test]
pub fn test_dict_keys_values() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var d = {\"a\": 1, \"b\": 2, \"c\": 3};
        var keys = d.keys();
        print(len(keys));  // 3
        var val = d.get(\"b\");
        print(val);        // 2
        var missing = d.get(\"z\");
        print(missing);    // nil
        d[\"d\"] = 4;
        print(d.get(\"d\"));  // 4
    ",
    )
    .unwrap();
}

/// Basic string operations: concatenation, length, equality.

#[test]
pub fn test_string_operations() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var s = \"Hello\";
        print(s + \" World\");    // Hello World
        print(len(s));           // 5
        print(s == \"Hello\");    // true
        print(s != \"Bye\");      // true
        print(str(s));           // Hello
    ",
    )
    .unwrap();
}
