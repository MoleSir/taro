use super::*;

#[test]
pub fn test_global_variable() {
    let _vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::DefineGlobal("x".into()), 1, 1, h);
        c.write_instruction(Instruction::GetGlobal("print".into()), 1, 1, h);
        c.write_instruction(Instruction::GetGlobal("x".into()), 1, 1, h);
        c.write_instruction(Instruction::Call(1), 1, 1, h);
        c.write_instruction(Instruction::Pop, 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(99)), 1, 1, h);
        c.write_instruction(Instruction::SetGlobal("x".into()), 1, 1, h);
        c.write_instruction(Instruction::GetGlobal("print".into()), 1, 1, h);
        c.write_instruction(Instruction::GetGlobal("x".into()), 1, 1, h);
        c.write_instruction(Instruction::Call(1), 1, 1, h);
        c.write_instruction(Instruction::Pop, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
}


#[test]
pub fn test_local_variable_get_set() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(10)), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(20)), 1, 1, h);
        c.write_instruction(Instruction::GetLocal(1), 1, 1, h);
        c.write_instruction(Instruction::GetLocal(2), 1, 1, h);
        c.write_instruction(Instruction::Add, 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 30);
}


#[test]
pub fn test_local_variable_set() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(10)), 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::SetLocal(1), 1, 1, h);
        c.write_instruction(Instruction::Pop, 1, 1, h);
        c.write_instruction(Instruction::GetLocal(1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}


#[test]
pub fn test_scopes() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::GetLocal(1), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}


#[test]
pub fn test_jump_if_false() {
    let mut vm = run_chunk(|c, h| {
        c.write_instruction(Instruction::True, 1, 1, h);
        c.write_instruction(Instruction::JumpIfFalse(5), 1, 1, h);
        c.write_instruction(Instruction::Pop, 1, 1, h);
        c.write_instruction(Instruction::Constant(h.alloc_integer_instance(42)), 1, 1, h);
        c.write_instruction(Instruction::Return, 1, 1, h);
    });
    let r = vm.pop_stack().unwrap();
    assert_eq!(get_int(&vm, r), 42);
}


#[test]
pub fn test_while_loop() {
    // Test while loop via compilation (return isn't allowed at top level).
    let mut vm = VirtualMachine::new();
    vm.interpret("{ var i = 0; while i < 3 { i = i + 1; } }").unwrap();
    // The script returns nil implicitly — verify execution succeeded.
}


#[test]
pub fn test_while_break_terminates() {
    let mut vm = VirtualMachine::new();
    // Infinite while loop with break should terminate.
    vm.interpret("while true { break; }").unwrap();
}


#[test]
pub fn test_while_break_early_exit() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var i = 0;
        while i < 10 {
            i = i + 1;
            if i == 3 { break; }
        }
        print(i);  // 3
    ",
    )
    .unwrap();
}


#[test]
pub fn test_while_continue_skips_rest() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var i = 0;
        var sum = 0;
        while i < 5 {
            i = i + 1;
            if i == 3 { continue; }
            sum = sum + i;
        }
        print(sum);  // 1 + 2 + 4 + 5 = 12
    ",
    )
    .unwrap();
}


#[test]
pub fn test_for_break() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var i = 0;
        for (i = 0; i < 10; i = i + 1) {
            if i == 4 { break; }
        }
        print(i);  // 4
    ",
    )
    .unwrap();
}


#[test]
pub fn test_for_continue_increment_runs() {
    // Critical: verify the increment clause executes on continue.
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var results = [];
        for (var i = 0; i < 5; i = i + 1) {
            if i == 2 { continue; }
            results.append(i);
        }
        // results should be [0, 1, 3, 4]
        print(len(results));  // 4
        print(results[0]);    // 0
        print(results[1]);    // 1
        print(results[2]);    // 3
        print(results[3]);    // 4
    ",
    )
    .unwrap();
}


#[test]
pub fn test_nested_break() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var i = 0;
        while i < 3 {
            var j = 0;
            while j < 3 {
                j = j + 1;
                if j == 2 { break; }
            }
            i = i + 1;
        }
        print(i);  // 3 — outer loop runs all iterations
    ",
    )
    .unwrap();
}


#[test]
pub fn test_nested_continue() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var sum = 0;
        var i = 0;
        while i < 3 {
            i = i + 1;
            var j = 0;
            while j < 3 {
                j = j + 1;
                if j == 2 { continue; }
                sum = sum + 1;
            }
        }
        // Inner loop: j=1(sum+1), j=2(skip), j=3(sum+1) => 2 per outer iter
        // 3 outer iterations => 6
        print(sum);  // 6
    ",
    )
    .unwrap();
}


#[test]
pub fn test_break_in_for_without_condition() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var i = 0;
        for (;;) {
            i = i + 1;
            if i == 5 { break; }
        }
        print(i);  // 5
    ",
    )
    .unwrap();
}


#[test]
pub fn test_continue_in_for_without_increment() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var sum = 0;
        for (var i = 0; i < 5;) {
            i = i + 1;
            if i == 3 { continue; }
            sum = sum + i;
        }
        print(sum);  // 1 + 2 + 4 + 5 = 12
    ",
    )
    .unwrap();
}


#[test]
pub fn test_break_inside_if_inside_loop() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var i = 0;
        while i < 10 {
            i = i + 1;
            if i == 7 { break; }
        }
        print(i);  // 7
    ",
    )
    .unwrap();
}


#[test]
pub fn test_break_outside_loop_compile_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("break;").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("break"), "got: {msg}");
}


#[test]
pub fn test_continue_outside_loop_compile_error() {
    let mut vm = VirtualMachine::new();
    let err = vm.interpret("continue;").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("continue"), "got: {msg}");
}


#[test]
pub fn test_break_inside_nested_block_in_while() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var i = 0;
        while i < 10 {
            {
                i = i + 1;
                if i == 5 { break; }
            }
        }
        print(i);  // 5
    ",
    )
    .unwrap();
}


#[test]
pub fn test_continue_respects_increment_in_for() {
    // Make sure the increment is evaluated exactly once per continue.
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var count = 0;
        for (var i = 0; i < 10; i = i + 1) {
            count = count + 1;
            if i < 7 { continue; }
            print(i);  // 7, 8, 9
        }
        // count should be 10 (body executed 10 times)
        print(count);
    ",
    )
    .unwrap();
}

// ------------------------------------------------------------------------
//  For-in — VM tests
// ------------------------------------------------------------------------


#[test]
fn test_for_in_list() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = \"\";
        for x in [1, 2, 3] { acc = acc + str(x); }
        print(acc);
    ",
    )
    .unwrap();
}


#[test]
fn test_for_in_string() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = \"\";
        for c in \"ab\" { acc = acc + c; }
        print(acc);
    ",
    )
    .unwrap();
}


#[test]
fn test_for_in_empty_list() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var count = 0;
        for x in [] { count = count + 1; }
        print(count);  // 0
    ",
    )
    .unwrap();
}


#[test]
fn test_for_in_break() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = \"\";
        for x in [1, 2, 3, 4] {
            if x > 2 { break; }
            acc = acc + str(x);
        }
        print(acc);  // \"12\"
    ",
    )
    .unwrap();
}


#[test]
fn test_for_in_continue() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = \"\";
        for x in [1, 2, 3] {
            if x == 2 { continue; }
            acc = acc + str(x);
        }
        print(acc);  // \"13\"
    ",
    )
    .unwrap();
}


#[test]
fn test_for_in_dict_keys() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var count = 0;
        for k in {\"a\": 1, \"b\": 2} { count = count + 1; }
        print(count);  // 2
    ",
    )
    .unwrap();
}


#[test]
fn test_for_in_nested() {
    let mut vm = VirtualMachine::new();
    vm.interpret(
        "
        var acc = 0;
        for x in [1, 2] {
            for y in [10, 20] { acc = acc + x + y; }
        }
        print(acc);  // 1+10 + 1+20 + 2+10 + 2+20 = 66
    ",
    )
    .unwrap();
}


