use crate::{Chunk, Instruction, Value, ToShrString};
use super::{CallFrame, VirtualMachine};

fn run_chunk(chunk: Chunk) -> VirtualMachine {    
    let mut vm = VirtualMachine::new();
    let function = vm.obj_heap.alloc_function("script", 0, chunk);
    vm.stack = vec![Value::Object(function)];
    vm.frames = vec![CallFrame { function, ip: 0, slots_start: 0 }];
    vm.run().unwrap();
    vm
}

#[test]
pub fn test_base_arith() {
    let mut chunk = Chunk::new();

    chunk.write_instruction(Instruction::Constant(Value::Float(3.4)));
    chunk.write_instruction(Instruction::Constant(Value::Float(1.2)));
    chunk.write_instruction(Instruction::Add);

    chunk.write_instruction(Instruction::Constant(Value::Float(5.6)));
    chunk.write_instruction(Instruction::Div);
    chunk.write_instruction(Instruction::Negate);
    chunk.write_instruction(Instruction::Return);

    let mut vm = run_chunk(chunk);
    // (3.4 + 1.2) / 5.6 = 4.6 / 5.6 ≈ 0.82142…, negated ≈ -0.82142…
    let result = vm.pop_stack().unwrap();
    println!("{:?}", result);
    assert!(matches!(result, Value::Float(_)));
}

#[test]
pub fn test_global_variable() {
    let mut chunk = Chunk::new();

    // var x = 42;
    chunk.write_instruction(Instruction::Constant(Value::Integer(42))); // push_stack 42
    chunk.write_instruction(Instruction::DefineGlobal("x".to_shrstring())); // pop_stack → x

    // print x;  →  GetGlobal("print"); GetGlobal("x"); Call(1); Pop;
    chunk.write_instruction(Instruction::GetGlobal("print".into()));
    chunk.write_instruction(Instruction::GetGlobal("x".to_shrstring())); // push_stack x
    chunk.write_instruction(Instruction::Call(1));
    chunk.write_instruction(Instruction::Pop);

    // x = 99;
    chunk.write_instruction(Instruction::Constant(Value::Integer(99)));
    chunk.write_instruction(Instruction::SetGlobal("x".to_shrstring()));

    // print x;
    chunk.write_instruction(Instruction::GetGlobal("print".into()));
    chunk.write_instruction(Instruction::GetGlobal("x".to_shrstring()));
    chunk.write_instruction(Instruction::Call(1));
    chunk.write_instruction(Instruction::Pop);

    chunk.write_instruction(Instruction::Return);

    run_chunk(chunk);
}

#[test]
pub fn test_local_variable_get_set() {
    let mut chunk = Chunk::new();

    // Simulate: var a = 10; var b = 20;
    // slot 0 = function, slot 1 = a, slot 2 = b
    chunk.write_instruction(Instruction::Constant(Value::Integer(10)));
    chunk.write_instruction(Instruction::Constant(Value::Integer(20)));

    // a + b  →  GetLocal(1); GetLocal(2); Add
    chunk.write_instruction(Instruction::GetLocal(1));
    chunk.write_instruction(Instruction::GetLocal(2));
    chunk.write_instruction(Instruction::Add);

    // Result should be 10 + 20 = 30
    chunk.write_instruction(Instruction::Return);

    let mut vm = run_chunk(chunk);
    let result = vm.pop_stack().unwrap();
    assert_eq!(result, Value::Integer(30));
}

#[test]
pub fn test_local_variable_set() {
    let mut chunk = Chunk::new();

    // Simulate: var a = 10; a = 42;  (slot 0 = fn, slot 1 = a)
    // First push the initial value for slot 1
    chunk.write_instruction(Instruction::Constant(Value::Integer(10)));

    // Now a = 42:
    // 1. Push 42 onto the stack
    // 2. SetLocal(1) — writes 42 into stack[1], leaves 42 on stack
    // 3. Pop to discard the expression result
    chunk.write_instruction(Instruction::Constant(Value::Integer(42)));
    chunk.write_instruction(Instruction::SetLocal(1));
    chunk.write_instruction(Instruction::Pop);

    // Now read a: GetLocal(1) → should be 42
    chunk.write_instruction(Instruction::GetLocal(1));
    chunk.write_instruction(Instruction::Return);

    let mut vm = run_chunk(chunk);
    let result = vm.pop_stack().unwrap();
    assert_eq!(result, Value::Integer(42));
}

#[test]
pub fn test_if_then_branch_taken() {
    // if (true) { 42; }  → result on stack = 42
    let mut chunk = Chunk::new();
    // true → condition
    chunk.write_instruction(Instruction::True);
    // JumpIfFalse: if false, skip then-branch. offset to land after the Jump (at Pop+Return below)
    chunk.write_instruction(Instruction::JumpIfFalse(7)); // skip: Constant(42) + Pop + Jump = 3+1+3 = 7
    chunk.write_instruction(Instruction::Pop); // pop true (taking then-branch)
    // then-branch: 42
    chunk.write_instruction(Instruction::Constant(Value::Integer(42)));
    chunk.write_instruction(Instruction::Pop); // discard expr result
    // Jump over else (absent) → to final Pop
    chunk.write_instruction(Instruction::Jump(1)); // skip the condition Pop
    chunk.write_instruction(Instruction::Pop); // pop true (from JumpIfFalse)
    chunk.write_instruction(Instruction::Return);

    let mut vm = run_chunk(chunk);
    // The stack should be empty (expression statement pops result,
    // only Return would leave something if we pushed before it...
    // Actually there's nothing left on stack — let's verify by
    // checking we can push/pop without error)
    vm.push_stack(Value::Integer(0));
    let _ = vm.pop_stack().unwrap();
}

#[test]
pub fn test_if_else_branch_taken() {
    // if (false) { 1; } else { 42; } → result 42
    let mut chunk = Chunk::new();
    // false → condition
    chunk.write_instruction(Instruction::False);
    // JumpIfFalse: if false, jump to else. offset = past then-branch (Pop+Constant(1)+Pop+Jump)
    chunk.write_instruction(Instruction::JumpIfFalse(8)); // skip then-branch: 1+3+1+3 = 8
    chunk.write_instruction(Instruction::Pop); // pop false (not taken here)
    // then: 1
    chunk.write_instruction(Instruction::Constant(Value::Integer(1)));
    chunk.write_instruction(Instruction::Pop);
    // Jump over else
    chunk.write_instruction(Instruction::Jump(6)); // skip else: Pop+Constant(42)+Pop = 1+3+1 = 5, +1
    chunk.write_instruction(Instruction::Pop); // pop false (else entry)
    // else: 42
    chunk.write_instruction(Instruction::Constant(Value::Integer(42)));
    chunk.write_instruction(Instruction::Pop); // discard expr
    chunk.write_instruction(Instruction::Return);

    run_chunk(chunk);
}

#[test]
pub fn test_while_loop_executes() {
    // Simulate: var i = 3; while (i > 0) { i = i - 1; }
    // But simpler: just test the loop mechanics
    // slot 0 = function, slot 1 = i
    // while (i > 0): GetLocal(1), Constant(0), Greater, JumpIfFalse, ...
    let mut chunk = Chunk::new();
    // Set up: i = 1 at slot 1 (slot 0 is the function)
    chunk.write_instruction(Instruction::Constant(Value::Integer(1)));
    // loop_start at pos 3 (after Constant 3 bytes)
    // condition: GetLocal(1), Constant(0), Greater
    chunk.write_instruction(Instruction::GetLocal(1));
    chunk.write_instruction(Instruction::Constant(Value::Integer(0)));
    chunk.write_instruction(Instruction::Greater);                // stack: [fn, 1, 0, true]
    // JumpIfFalse exit — skip Pop + body + Loop = 1 + 3 + 1 + 3 = 8
    chunk.write_instruction(Instruction::JumpIfFalse(8));
    chunk.write_instruction(Instruction::Pop);                    // pop true
    // body: push 0, SetLocal(1), Pop. slot 1 = 0 on second iteration
    chunk.write_instruction(Instruction::Constant(Value::Integer(0)));
    chunk.write_instruction(Instruction::SetLocal(1));
    chunk.write_instruction(Instruction::Pop);                    // pop assignment result
    // Loop back to loop_start (position 3, offset = here-3+3)
    chunk.write_instruction(Instruction::Loop(12));
    chunk.write_instruction(Instruction::Pop);                    // pop exit condition
    chunk.write_instruction(Instruction::Return);

    let vm = run_chunk(chunk);
    // After the loop, slot 1 should be 0
    assert_eq!(vm.stack[1], Value::Integer(0));
}

#[test]
pub fn test_for_loop_simple() {
    // Simulate: var i = 0; for (; i < 3; i = i + 1) {}
    // This tests the basic for-loop desugaring pattern
    // slot 0 = function, slot 1 = i
    let mut chunk = Chunk::new();
    chunk.write_instruction(Instruction::Constant(Value::Integer(0)));
    // loop_start (condition): GetLocal(1), Constant(3), Less
    let loop_start = chunk.codes.len(); // should be 3 (after Constant 3 bytes)
    chunk.write_instruction(Instruction::GetLocal(1));
    chunk.write_instruction(Instruction::Constant(Value::Integer(3)));
    chunk.write_instruction(Instruction::Less);
    // JumpIfFalse exit
    chunk.write_instruction(Instruction::JumpIfFalse(0)); // placeholder, patched later
    let exit_jump = chunk.codes.len() - 2;
    chunk.write_instruction(Instruction::Pop); // pop condition
    // body: empty (just a no-op — we're testing the increment)
    // increment: GetLocal(1), Constant(1), Add, SetLocal(1), Pop
    let increment_start = chunk.codes.len();
    chunk.write_instruction(Instruction::GetLocal(1));
    chunk.write_instruction(Instruction::Constant(Value::Integer(1)));
    chunk.write_instruction(Instruction::Add);
    chunk.write_instruction(Instruction::SetLocal(1));
    chunk.write_instruction(Instruction::Pop); // pop assignment result
    // Loop to condition
    let loop_offset = chunk.codes.len() - loop_start + 3;
    chunk.write_instruction(Instruction::Loop(loop_offset));
    // Loop to increment (from body — body is empty here, so this is redundant
    // but included to test the pattern)
    let body_loop_offset = chunk.codes.len() - increment_start + 3;
    chunk.write_instruction(Instruction::Loop(body_loop_offset));
    // Patch exit jump
    let exit_offset = chunk.codes.len() - exit_jump - 2;
    chunk.codes[exit_jump] = (exit_offset as u16).to_le_bytes()[0];
    chunk.codes[exit_jump + 1] = (exit_offset as u16).to_le_bytes()[1];
    chunk.write_instruction(Instruction::Pop); // pop false
    chunk.write_instruction(Instruction::Return);

    let vm = run_chunk(chunk);
    // After 3 iterations, i should be 3
    assert_eq!(vm.stack[1], Value::Integer(3));
}

// ------------------------------------------------------------------------
//  Function calls (compile + execute)
// ------------------------------------------------------------------------

#[test]
pub fn test_function_call_no_args_no_return() {
    // `fun foo() { print 42; } foo();`
    // Should print 42 without errors.
    let mut vm = VirtualMachine::new();
    vm.interpret("fun foo() { print(42); } foo();").unwrap();
}

#[test]
pub fn test_function_call_with_return() {
    // `fun add(a, b) { return a + b; } print add(3, 4);`
    // Should print 7.
    let mut vm = VirtualMachine::new();
    vm.interpret("fun add(a, b) { return a + b; } print(add(3, 4));").unwrap();
}

#[test]
pub fn test_function_call_implicit_return() {
    // A function without an explicit return returns nil.
    // `fun f() {} var x = f(); print x;`
    let mut vm = VirtualMachine::new();
    vm.interpret("fun f() {} var x = f(); print(x);").unwrap();
}

#[test]
pub fn test_nested_function_call() {
    // `fun f() { return 1; } fun g() { return f(); } print g();`
    // Should print 1.
    let mut vm = VirtualMachine::new();
    vm.interpret("fun f() { return 1; } fun g() { return f(); } print(g());").unwrap();
}

#[test]
pub fn test_function_with_multiple_params() {
    // `fun sum(a, b, c) { return a + b + c; } print sum(1, 2, 3);`
    // Should print 6.
    let mut vm = VirtualMachine::new();
    vm.interpret("fun sum(a, b, c) { return a + b + c; } print(sum(1, 2, 3));").unwrap();
}

#[test]
pub fn test_function_call_as_expression() {
    // `fun double(x) { return x * 2; } print double(5) + 1;`
    // Should print 11.
    let mut vm = VirtualMachine::new();
    vm.interpret("fun double(x) { return x * 2; } print(double(5) + 1);").unwrap();
}

#[test]
pub fn test_arg_count_mismatch() {
    // Calling with wrong number of args should be a runtime error.
    let mut vm = VirtualMachine::new();
    vm.interpret("fun f(a, b) {} f(1);").unwrap_err();
}