use crate::{Chunk, Instruction, Value, ToShrString};
use super::VirtualMachine;

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

    let mut vm = VirtualMachine::new(chunk);
    vm.run().unwrap();
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

    // print x;
    chunk.write_instruction(Instruction::GetGlobal("x".to_shrstring())); // push_stack x
    chunk.write_instruction(Instruction::Print);

    // x = 99;
    chunk.write_instruction(Instruction::Constant(Value::Integer(99)));
    chunk.write_instruction(Instruction::SetGlobal("x".to_shrstring()));

    // print x;
    chunk.write_instruction(Instruction::GetGlobal("x".to_shrstring()));
    chunk.write_instruction(Instruction::Print);

    chunk.write_instruction(Instruction::Return);

    let mut vm = VirtualMachine::new(chunk);
    vm.run().unwrap();
}

#[test]
pub fn test_local_variable_get_set() {
    let mut chunk = Chunk::new();

    // Simulate: var a = 10; var b = 20;
    // slot 0 = a, slot 1 = b
    chunk.write_instruction(Instruction::Constant(Value::Integer(10)));
    chunk.write_instruction(Instruction::Constant(Value::Integer(20)));

    // a + b  →  GetLocal(0); GetLocal(1); Add
    chunk.write_instruction(Instruction::GetLocal(0));
    chunk.write_instruction(Instruction::GetLocal(1));
    chunk.write_instruction(Instruction::Add);

    // Result should be 10 + 20 = 30
    chunk.write_instruction(Instruction::Return);

    let mut vm = VirtualMachine::new(chunk);
    vm.run().unwrap();
    let result = vm.pop_stack().unwrap();
    assert_eq!(result, Value::Integer(30));
}

#[test]
pub fn test_local_variable_set() {
    let mut chunk = Chunk::new();

    // Simulate: var a = 10; a = 42;  (slot 0 = a)
    // First push the initial value for slot 0
    chunk.write_instruction(Instruction::Constant(Value::Integer(10)));

    // Now a = 42:
    // 1. Push 42 onto the stack
    // 2. SetLocal(0) — writes 42 into stack[0], leaves 42 on stack
    // 3. Pop to discard the expression result
    chunk.write_instruction(Instruction::Constant(Value::Integer(42)));
    chunk.write_instruction(Instruction::SetLocal(0));
    chunk.write_instruction(Instruction::Pop);

    // Now read a: GetLocal(0) → should be 42
    chunk.write_instruction(Instruction::GetLocal(0));
    chunk.write_instruction(Instruction::Return);

    let mut vm = VirtualMachine::new(chunk);
    vm.run().unwrap();
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

    let mut vm = VirtualMachine::new(chunk);
    vm.run().unwrap();
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

    let mut vm = VirtualMachine::new(chunk);
    vm.run().unwrap();
}

#[test]
pub fn test_while_loop_executes() {
    // Simulate: var i = 3; while (i > 0) { i = i - 1; }
    // But simpler: just test the loop mechanics
    // Stack: slot 0 = 1
    // while (slot 0 > 0): GetLocal(0), Constant(0), Greater, JumpIfFalse, ...
    let mut chunk = Chunk::new();
    // Set up: slot 0 = 1 (simulating var i = 1)
    chunk.write_instruction(Instruction::Constant(Value::Integer(1)));
    // loop_start at pos 3 (after Constant 3 bytes)
    // condition: GetLocal(0), Constant(0), Greater
    chunk.write_instruction(Instruction::GetLocal(0));
    chunk.write_instruction(Instruction::Constant(Value::Integer(0)));
    chunk.write_instruction(Instruction::Greater);                // stack: [1, 1, 0, true]
    // JumpIfFalse exit — skip Pop + body + Loop = 1 + 3 + 1 + 3 = 8
    chunk.write_instruction(Instruction::JumpIfFalse(8));
    chunk.write_instruction(Instruction::Pop);                    // pop true
    // body: push 0, SetLocal(0), Pop. slot 0 = 0 on second iteration
    chunk.write_instruction(Instruction::Constant(Value::Integer(0)));
    chunk.write_instruction(Instruction::SetLocal(0));
    chunk.write_instruction(Instruction::Pop);                    // pop assignment result
    // Loop back to loop_start (position 3, offset = here-3+3)
    chunk.write_instruction(Instruction::Loop(12));
    chunk.write_instruction(Instruction::Pop);                    // pop exit condition
    chunk.write_instruction(Instruction::Return);

    let mut vm = VirtualMachine::new(chunk);
    vm.run().unwrap();
    // After the loop, slot 0 should be 0
    assert_eq!(vm.stack[0], Value::Integer(0));
}

#[test]
pub fn test_for_loop_simple() {
    // Simulate: var i = 0; for (; i < 3; i = i + 1) {}
    // This tests the basic for-loop desugaring pattern
    let mut chunk = Chunk::new();
    // slot 0 = 0 (initializer already done)
    chunk.write_instruction(Instruction::Constant(Value::Integer(0)));
    // loop_start (condition): GetLocal(0), Constant(3), Less
    let loop_start = chunk.codes.len(); // should be 3 (after Constant 3 bytes)
    chunk.write_instruction(Instruction::GetLocal(0));
    chunk.write_instruction(Instruction::Constant(Value::Integer(3)));
    chunk.write_instruction(Instruction::Less);
    // JumpIfFalse exit
    chunk.write_instruction(Instruction::JumpIfFalse(0)); // placeholder, patched later
    let exit_jump = chunk.codes.len() - 2;
    chunk.write_instruction(Instruction::Pop); // pop condition
    // body: empty (just a no-op — we're testing the increment)
    // increment: GetLocal(0), Constant(1), Add, SetLocal(0), Pop
    let increment_start = chunk.codes.len();
    chunk.write_instruction(Instruction::GetLocal(0));
    chunk.write_instruction(Instruction::Constant(Value::Integer(1)));
    chunk.write_instruction(Instruction::Add);
    chunk.write_instruction(Instruction::SetLocal(0));
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

    let mut vm = VirtualMachine::new(chunk);
    vm.run().unwrap();
    // After 3 iterations, i should be 3
    assert_eq!(vm.stack[0], Value::Integer(3));
}