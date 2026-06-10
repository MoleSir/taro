use super::*;
use super::parse::ParseReason;
use crate::{ByteCode, ToShrString};
use crate::Value;

// ------------------------------------------------------------------------
//  Helpers
// ------------------------------------------------------------------------

/// Compile source and return the chunk.
fn chunk(source: &str) -> Chunk {
    compile(source).expect("compilation should succeed")
}

/// Compile source and return just the code vector.
fn codes(source: &str) -> Vec<u8> {
    chunk(source).codes
}

/// Compile source and return code + constants.
fn compiled(source: &str) -> (Vec<u8>, Vec<Value>) {
    let c = chunk(source);
    (c.codes, c.constants)
}

/// Assert that source fails to compile.
fn assert_err(source: &str) {
    assert!(
        compile(source).is_err(),
        "expected compilation error for: {source:?}"
    );
}

// ------------------------------------------------------------------------
//  Number literals
// ------------------------------------------------------------------------

#[test]
fn test_integer_literal() {
    let (codes, constants) = compiled("42;");
    // Constant(0) 占 3 字节: [操作码, 索引低字节, 索引高字节]
    assert_eq!(&codes[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(constants[0], Value::Integer(42));
    
    // 因为 Constant 占了 0,1,2，所以下一个指令在索引 3
    assert_eq!(codes[3], ByteCode::Pop as u8);
    assert_eq!(*codes.last().unwrap(), ByteCode::Return as u8);
}

#[test]
fn test_decimal_literal() {
    let (_, constants) = compiled("3.14;");
    assert_eq!(constants[0], Value::Float(3.14));
}

// ------------------------------------------------------------------------
//  Keyword literals
// ------------------------------------------------------------------------

#[test]
fn test_true_literal() {
    let c = codes("true;");
    assert_eq!(c[0], ByteCode::True as u8);
    assert_eq!(c[1], ByteCode::Pop as u8);
}

#[test]
fn test_false_literal() {
    let c = codes("false;");
    assert_eq!(c[0], ByteCode::False as u8);
    assert_eq!(c[1], ByteCode::Pop as u8);
}

#[test]
fn test_nil_literal() {
    let c = codes("nil;");
    assert_eq!(c[0], ByteCode::Nil as u8);
    assert_eq!(c[1], ByteCode::Pop as u8);
}

// ------------------------------------------------------------------------
//  String literals
// ------------------------------------------------------------------------

#[test]
fn test_string_literal() {
    let (_, constants) = compiled("\"hello\";");
    assert_eq!(constants[0], Value::String("hello".to_shrstring()));
}

#[test]
fn test_empty_string() {
    let (_, constants) = compiled("\"\";");
    assert_eq!(constants[0], Value::String("".to_shrstring()));
}

// ------------------------------------------------------------------------
//  Unary expressions
// ------------------------------------------------------------------------

#[test]
fn test_unary_negate() {
    let c = codes("-5;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]); // push 5
    assert_eq!(c[3], ByteCode::Negate as u8);                // negate
}

#[test]
fn test_unary_not() {
    let c = codes("!true;");
    assert_eq!(c[0], ByteCode::True as u8);  // push true (dedicated opcode)
    assert_eq!(c[1], ByteCode::Not as u8);   // not
}

// ------------------------------------------------------------------------
//  Binary arithmetic
// ------------------------------------------------------------------------

#[test]
fn test_addition() {
    let c = codes("1 + 2;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]); // push 1 (index 0)
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]); // push 2 (index 1)
    assert_eq!(c[6], ByteCode::Add as u8);                   // add
}

#[test]
fn test_subtraction() {
    let c = codes("5 - 3;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[6], ByteCode::Sub as u8);
}

#[test]
fn test_multiplication() {
    let c = codes("6 * 7;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[6], ByteCode::Mul as u8);
}

#[test]
fn test_division() {
    let c = codes("8 / 2;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[6], ByteCode::Div as u8);
}

// ------------------------------------------------------------------------
//  Precedence
// ------------------------------------------------------------------------

#[test]
fn test_precedence_mul_before_add() {
    // 1 + 2 * 3  →  push 1; push 2; push 3; mul; add
    let c = codes("1 + 2 * 3;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]); // 1
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]); // 2
    assert_eq!(&c[6..9], &[ByteCode::Constant as u8, 2, 0]); // 3
    assert_eq!(c[9], ByteCode::Mul as u8);                   // 2*3
    assert_eq!(c[10], ByteCode::Add as u8);                  // 1+(2*3)
}

#[test]
fn test_grouping_overrides_precedence() {
    // (1 + 2) * 3  →  push 1; push 2; add; push 3; mul
    let c = codes("(1 + 2) * 3;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]); // 1
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]); // 2
    assert_eq!(c[6], ByteCode::Add as u8);                   // 1+2
    assert_eq!(&c[7..10], &[ByteCode::Constant as u8, 2, 0]);// 3
    assert_eq!(c[10], ByteCode::Mul as u8);                  // (1+2)*3
}

// ------------------------------------------------------------------------
//  Comparison / equality
// ------------------------------------------------------------------------

#[test]
fn test_equal() {
    let c = codes("1 == 2;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[6], ByteCode::Equal as u8);
}

#[test]
fn test_not_equal() {
    // `1 != 2` → push 1; push 2; not_equal
    let c = codes("1 != 2;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[6], ByteCode::NotEqual as u8);
}

#[test]
fn test_less() {
    let c = codes("1 < 2;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[6], ByteCode::Less as u8);
}

#[test]
fn test_greater() {
    let c = codes("1 > 2;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[6], ByteCode::Greater as u8);
}

#[test]
fn test_less_equal() {
    // `1 <= 2` → push 1; push 2; less_equal
    let c = codes("1 <= 2;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[6], ByteCode::LessEqual as u8);
}

#[test]
fn test_greater_equal() {
    // `1 >= 2` → push 1; push 2; greater_equal
    let c = codes("1 >= 2;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[6], ByteCode::GreaterEqual as u8);
}

// ------------------------------------------------------------------------
//  Print statement
// ------------------------------------------------------------------------

#[test]
fn test_print_statement() {
    let c = codes("print 42;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]); // push 42
    assert_eq!(c[3], ByteCode::Print as u8);                 // print
}

#[test]
fn test_print_expression() {
    let c = codes("print 1 + 2;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]); // push 1
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]); // push 2
    assert_eq!(c[6], ByteCode::Add as u8);                   // add
    assert_eq!(c[7], ByteCode::Print as u8);                 // print
}

// ------------------------------------------------------------------------
//  Multiple statements
// ------------------------------------------------------------------------

#[test]
fn test_multiple_statements() {
    let c = codes("1; 2;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]); // push 1
    assert_eq!(c[3], ByteCode::Pop as u8);                   // discard
    assert_eq!(&c[4..7], &[ByteCode::Constant as u8, 1, 0]); // push 2
    assert_eq!(c[7], ByteCode::Pop as u8);                   // discard
    assert_eq!(c[8], ByteCode::Return as u8);                // implicit return
}

// ------------------------------------------------------------------------
//  Complex expression
// ------------------------------------------------------------------------

#[test]
fn test_complex_expression() {
    // -5 * (3 + 2) / 4
    // Expected: push 5; neg; push 3; push 2; add; mul; push 4; div
    let c = codes("-5 * (3 + 2) / 4;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);  // 5 (index 0)
    assert_eq!(c[3], ByteCode::Negate as u8);                 // neg
    assert_eq!(&c[4..7], &[ByteCode::Constant as u8, 1, 0]);  // 3 (index 1)
    assert_eq!(&c[7..10], &[ByteCode::Constant as u8, 2, 0]); // 2 (index 2)
    assert_eq!(c[10], ByteCode::Add as u8);                   // 3+2
    assert_eq!(c[11], ByteCode::Mul as u8);                   // -5 * (3+2)
    assert_eq!(&c[12..15], &[ByteCode::Constant as u8, 3, 0]);// 4 (index 3)
    assert_eq!(c[15], ByteCode::Div as u8);                   // ... / 4
}

// ------------------------------------------------------------------------
//  Local variables
// ------------------------------------------------------------------------

#[test]
fn test_local_var_declaration() {
    // { var x = 42; }
    //   Constant(42)   ← initializer, x at slot 0
    //   Pop            ← end_scope cleans up x
    let c = codes("{ var x = 42; }");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]); // push 42
    assert_eq!(c[3], ByteCode::Pop as u8);                   // discard x
    assert_eq!(*c.last().unwrap(), ByteCode::Return as u8);
}

#[test]
fn test_local_var_without_initializer() {
    // { var x; }  →  Nil (implicit init); Pop (cleanup)
    let c = codes("{ var x; }");
    assert_eq!(c[0], ByteCode::Nil as u8);  // implicit nil
    assert_eq!(c[1], ByteCode::Pop as u8);  // discard x
    assert_eq!(*c.last().unwrap(), ByteCode::Return as u8);
}

#[test]
fn test_local_var_read() {
    // { var x = 5; print x; }
    //   Constant(5)   ← x initializer
    //   GetLocal(0)   ← read x from slot 0
    //   Print
    //   Pop           ← cleanup x
    let c = codes("{ var x = 5; print x; }");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]); // push 5
    assert_eq!(c[3], ByteCode::GetLocal as u8);              // get slot 0
    assert_eq!(c[4], 0u8);                                   // slot index
    assert_eq!(c[5], 0u8);                                   // slot index
    assert_eq!(c[6], ByteCode::Print as u8);
    assert_eq!(c[7], ByteCode::Pop as u8);                   // cleanup
}

#[test]
fn test_local_var_assignment() {
    // { var x = 42; x = 99; }
    //   Constant(42)   ← initializer
    //   Constant(99)   ← new value
    //   SetLocal(0)    ← write to slot 0 (value stays on stack)
    //   Pop            ← discard assignment result
    //   Pop            ← cleanup x
    let c = codes("{ var x = 42; x = 99; }");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);     // push 42
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);     // push 99
    assert_eq!(&c[6..9], &[ByteCode::SetLocal as u8, 0, 0]);     // set slot 0 (opcode + u16)
    assert_eq!(c[9], ByteCode::Pop as u8);                       // discard expr result
    assert_eq!(c[10], ByteCode::Pop as u8);                      // cleanup x
}

#[test]
fn test_multiple_locals() {
    // { var a = 1; var b = 2; print a + b; }
    //   Constant(1)   ← a at slot 0
    //   Constant(2)   ← b at slot 1
    //   GetLocal(0)   ← a
    //   GetLocal(1)   ← b
    //   Add
    //   Print
    //   Pop           ← cleanup b
    //   Pop           ← cleanup a
    let c = codes("{ var a = 1; var b = 2; print a + b; }");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);     // push 1 (a)
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);     // push 2 (b)
    assert_eq!(&c[6..9], &[ByteCode::GetLocal as u8, 0, 0]);     // get a (slot 0)
    assert_eq!(&c[9..12], &[ByteCode::GetLocal as u8, 1, 0]);    // get b (slot 1)
    assert_eq!(c[12], ByteCode::Add as u8);
    assert_eq!(c[13], ByteCode::Print as u8);
    assert_eq!(c[14], ByteCode::Pop as u8);                      // cleanup b
    assert_eq!(c[15], ByteCode::Pop as u8);                      // cleanup a
}

#[test]
fn test_local_in_arithmetic_expression() {
    // { var a = 10; var b = 20; a * b + 5; }
    //   Constant(10)
    //   Constant(20)
    //   GetLocal(0)   ← a
    //   GetLocal(1)   ← b
    //   Mul
    //   Constant(5)
    //   Add
    //   Pop           ← discard expression result
    //   Pop           ← cleanup b
    //   Pop           ← cleanup a
    let c = codes("{ var a = 10; var b = 20; a * b + 5; }");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);      // 10
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);      // 20
    assert_eq!(&c[6..9], &[ByteCode::GetLocal as u8, 0, 0]);      // get a (slot 0)
    assert_eq!(&c[9..12], &[ByteCode::GetLocal as u8, 1, 0]);     // get b (slot 1)
    assert_eq!(c[12], ByteCode::Mul as u8);                       // a * b
    assert_eq!(&c[13..16], &[ByteCode::Constant as u8, 2, 0]);    // 5
    assert_eq!(c[16], ByteCode::Add as u8);                       // (a*b) + 5
    assert_eq!(c[17], ByteCode::Pop as u8);                       // discard expr
    assert_eq!(c[18], ByteCode::Pop as u8);                       // cleanup b
    assert_eq!(c[19], ByteCode::Pop as u8);                       // cleanup a
}

#[test]
fn test_nested_block_locals() {
    // { var a = 1; { var b = 2; print a + b; } print a; }
    // Outer: Constant(1) as a (slot 0)
    // Inner: Constant(2) as b (slot 1)
    //        GetLocal(0), GetLocal(1), Add, Print
    //        Pop ← cleanup b
    // Outer: GetLocal(0), Print
    //        Pop ← cleanup a
    let c = codes("{ var a = 1; { var b = 2; print a + b; } print a; }");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);      // push 1 (a)
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);      // push 2 (b)
    assert_eq!(&c[6..9], &[ByteCode::GetLocal as u8, 0, 0]);      // get a (slot 0)
    assert_eq!(&c[9..12], &[ByteCode::GetLocal as u8, 1, 0]);     // get b (slot 1)
    assert_eq!(c[12], ByteCode::Add as u8);
    assert_eq!(c[13], ByteCode::Print as u8);
    assert_eq!(c[14], ByteCode::Pop as u8);                       // cleanup b
    assert_eq!(&c[15..18], &[ByteCode::GetLocal as u8, 0, 0]);    // get a (outer)
    assert_eq!(c[18], ByteCode::Print as u8);
    assert_eq!(c[19], ByteCode::Pop as u8);                       // cleanup a
}

#[test]
fn test_slot_reuse_after_block_exit() {
    // { var a = 1; { var b = 2; } var c = 3; print c; }
    // a = slot 0, b = slot 1 (then popped)
    // c reuses slot 1, read with GetLocal(1)
    let c = codes("{ var a = 1; { var b = 2; } var c = 3; print c; }");
    // a initializer
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);      // push 1 (a, slot 0)
    // b initializer
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);      // push 2 (b, slot 1)
    // b cleanup (end of inner scope)
    assert_eq!(c[6], ByteCode::Pop as u8);
    // c initializer — reuses slot 1
    assert_eq!(&c[7..10], &[ByteCode::Constant as u8, 2, 0]);     // push 3 (c, slot 1)
    // print c → GetLocal(1) (opcode + u16)
    assert_eq!(&c[10..13], &[ByteCode::GetLocal as u8, 1, 0]);    // get c (slot 1)
    assert_eq!(c[13], ByteCode::Print as u8);
    // cleanup c and a
    assert_eq!(c[14], ByteCode::Pop as u8);
    assert_eq!(c[15], ByteCode::Pop as u8);
}

#[test]
fn test_local_assignment_is_expression() {
    // { var x = 1; var y = (x = 5); }
    // The assignment x = 5 evaluates to 5,
    // so y should be initialized with 5.
    let c = codes("{ var x = 1; var y = (x = 5); }");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);      // push 1 (x, slot 0)
    // x = 5 as expression:
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);      // push 5
    assert_eq!(&c[6..9], &[ByteCode::SetLocal as u8, 0, 0]);      // x = 5 (slot 0)
    // setlocal leaves 5 on stack → that becomes y's initializer
    // Pop cleanup y
    assert_eq!(c[9], ByteCode::Pop as u8);
    // Pop cleanup x
    assert_eq!(c[10], ByteCode::Pop as u8);
}

// ------------------------------------------------------------------------
//  Local variable error cases
// ------------------------------------------------------------------------

#[test]
fn test_local_self_reference_is_error() {
    // { var a = a; }  — can't read 'a' in its own initializer
    let source = "{ var a = a; }";
    match compile(source) {
        Err(CompileError::Parse(errors)) => {
            assert!(!errors.is_empty(), "expected parse errors");
        }
        Err(CompileError::Scan(_)) => panic!("unexpected scan error"),
        Ok(_) => panic!("expected compilation to fail for self-referencing local"),
    }
}

#[test]
fn test_duplicate_local_is_error() {
    // { var a = 1; var a = 2; }  — redefinition in same scope
    let source = "{ var a = 1; var a = 2; }";
    match compile(source) {
        Err(CompileError::Parse(errors)) => {
            assert!(errors.iter().any(
                |e| matches!(e.reason, ParseReason::VariableRedefine(_))
            ), "expected VariableRedefine error");
        }
        Err(CompileError::Scan(_)) => panic!("unexpected scan error"),
        Ok(_) => panic!("expected compilation to fail for duplicate local"),
    }
}

// ------------------------------------------------------------------------
//  Control flow — if / else
// ------------------------------------------------------------------------

#[test]
fn test_if_statement() {
    // `if (true) 1;`
    //   True
    //   JumpIfFalse → skip then-branch
    //   Pop          ← pop condition when entering then-branch
    //   Constant(1)  ← then-branch
    //   Pop          ← discard expression result
    //   Jump         ← skip else (absent)
    //   Pop          ← pop condition when jumping from JumpIfFalse
    let c = codes("if (true) 1;");
    assert_eq!(c[0], ByteCode::True as u8);
    assert_eq!(c[1], ByteCode::JumpIfFalse as u8);
    // offset bytes at 2-3: should jump to the second Pop (after then-branch + Jump)
    assert_eq!(c[4], ByteCode::Pop as u8);
    assert_eq!(&c[5..8], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(c[8], ByteCode::Pop as u8);
    assert_eq!(c[9], ByteCode::Jump as u8);
    // The sequence ends with Pop (condition) + Return
    assert_eq!(*c.last().unwrap(), ByteCode::Return as u8);
}

#[test]
fn test_if_else_statement() {
    // `if (true) 1; else 2;`
    //   True
    //   JumpIfFalse → else branch
    //   Pop          ← pop condition
    //   Constant(1)  ← then-branch (index 0)
    //   Pop
    //   Jump         → skip else
    //   Pop          ← pop condition (else entry)
    //   Constant(2)  ← else-branch (index 1)
    //   Pop
    let c = codes("if (true) 1; else 2;");
    assert_eq!(c[0], ByteCode::True as u8);
    assert_eq!(c[1], ByteCode::JumpIfFalse as u8);
    assert_eq!(c[4], ByteCode::Pop as u8);
    // then-branch: push 1, pop
    assert_eq!(&c[5..8], &[ByteCode::Constant as u8, 0, 0]); // 1 at constant[0]
    assert_eq!(c[8], ByteCode::Pop as u8);
    assert_eq!(c[9], ByteCode::Jump as u8);
    // else entry pop
    // find the else constant: it's at index 1
}

#[test]
fn test_if_statement_condition_is_falsey_jumps() {
    // `if (false) 1;` — JumpIfFalse offset should skip the then-branch
    let c = codes("if (false) 1;");
    assert_eq!(c[0], ByteCode::False as u8);
    assert_eq!(c[1], ByteCode::JumpIfFalse as u8);
    // Verify structure: False, JumpIfFalse, Pop, Constant, Pop, Jump, Pop, Return
    let opcodes: Vec<u8> = c.iter().cloned()
        .filter(|&b| {
            // keep only opcode bytes (skip offset/operand data)
            b == ByteCode::Return as u8
                || b == ByteCode::Print as u8
                || b == ByteCode::Pop as u8
                || b == ByteCode::Nil as u8
                || b == ByteCode::True as u8
                || b == ByteCode::False as u8
                || b == ByteCode::Negate as u8
                || b == ByteCode::Not as u8
                || b == ByteCode::Add as u8
                || b == ByteCode::Sub as u8
                || b == ByteCode::Mul as u8
                || b == ByteCode::Div as u8
                || b == ByteCode::Equal as u8
                || b == ByteCode::NotEqual as u8
                || b == ByteCode::Greater as u8
                || b == ByteCode::GreaterEqual as u8
                || b == ByteCode::Less as u8
                || b == ByteCode::LessEqual as u8
                || b == ByteCode::Constant as u8
                || b == ByteCode::DefineGlobal as u8
                || b == ByteCode::GetGlobal as u8
                || b == ByteCode::SetGlobal as u8
                || b == ByteCode::GetLocal as u8
                || b == ByteCode::SetLocal as u8
                || b == ByteCode::JumpIfFalse as u8
                || b == ByteCode::Jump as u8
                || b == ByteCode::Loop as u8
        })
        .collect();
    // Expected opcode sequence
    assert!(opcodes.contains(&(ByteCode::JumpIfFalse as u8)));
    assert!(opcodes.contains(&(ByteCode::Jump as u8)));
}

#[test]
fn test_if_else_constants() {
    // Verify both branches' constants are present
    let (_, constants) = compiled("if (true) 42; else 99;");
    assert_eq!(constants.len(), 2);
    assert_eq!(constants[0], Value::Integer(42));
    assert_eq!(constants[1], Value::Integer(99));
}

#[test]
fn test_nested_if() {
    // `if (true) if (false) 1; else 2;`
    // The `else` binds to the nearest `if` (inner).
    let (_, constants) = compiled("if (true) if (false) 1; else 2;");
    // Both constants should be present
    assert!(constants.contains(&Value::Integer(1)));
    assert!(constants.contains(&Value::Integer(2)));
}

// ------------------------------------------------------------------------
//  Control flow — while
// ------------------------------------------------------------------------

#[test]
fn test_while_statement() {
    // `while (false) 1;`
    //   False
    //   JumpIfFalse → exit
    //   Pop          ← pop condition (entering body)
    //   Constant(1)
    //   Pop
    //   Loop         ← back to condition
    //   Pop          ← pop condition (exit)
    let c = codes("while (false) 1;");
    assert_eq!(c[0], ByteCode::False as u8);
    assert_eq!(c[1], ByteCode::JumpIfFalse as u8);
    // Pop after condition
    // (position depends on offset bytes, verify structural pattern)
    let has_loop = c.iter().any(|&b| b == ByteCode::Loop as u8);
    assert!(has_loop, "while loop should emit a Loop instruction");
    // The Loop should be before the final Pop/Return
}

#[test]
fn test_while_statement_loops_back() {
    // `while (true) 1;` — should have Loop jumping back to True
    let c = codes("while (true) 1;");
    assert_eq!(c[0], ByteCode::True as u8);       // condition: true
    // JumpIfFalse follows (offset bytes 1-2)
    assert_eq!(c[1], ByteCode::JumpIfFalse as u8);
    // Loop instruction must be present
    assert!(c.iter().any(|&b| b == ByteCode::Loop as u8));
}

#[test]
fn test_while_with_condition_variable() {
    // `{ var x = 0; while (x < 3) { print x; x = x + 1; } }`
    // Verifies that locals work inside while loops
    let c = codes("{ var x = 0; while (x < 3) { print x; x = x + 1; } }");
    // Should contain: GetLocal (read x), SetLocal (assign x), Loop
    assert!(c.iter().any(|&b| b == ByteCode::GetLocal as u8));
    assert!(c.iter().any(|&b| b == ByteCode::SetLocal as u8));
    assert!(c.iter().any(|&b| b == ByteCode::Loop as u8));
    assert!(c.iter().any(|&b| b == ByteCode::JumpIfFalse as u8));
}

// ------------------------------------------------------------------------
//  Control flow — for
// ------------------------------------------------------------------------

#[test]
fn test_for_statement_infinite() {
    // `for (;;) 1;` — infinite loop, no init/cond/incr
    let c = codes("for (;;) 1;");
    assert!(c.iter().any(|&b| b == ByteCode::Loop as u8),
        "infinite for-loop should emit a Loop instruction");
    // Should NOT have JumpIfFalse (no condition)
    assert!(!c.iter().any(|&b| b == ByteCode::JumpIfFalse as u8),
        "infinite for-loop should not have JumpIfFalse");
}

#[test]
fn test_for_statement_with_condition() {
    // `for (; true ;) 1;` — condition only, no init/incr
    let c = codes("for (; true ;) 1;");
    assert_eq!(c[0], ByteCode::True as u8);          // condition
    assert_eq!(c[1], ByteCode::JumpIfFalse as u8);   // exit jump
    assert!(c.iter().any(|&b| b == ByteCode::Loop as u8));
}

#[test]
fn test_for_statement_with_initializer() {
    // `for (var i = 0; i < 5; i = i + 1) print i;`
    let (c, constants) = compiled("for (var i = 0; i < 5; i = i + 1) print i;");
    // GetLocal for reading i in condition, body, and increment
    let get_local_count = c.windows(3)
        .filter(|w| w[0] == ByteCode::GetLocal as u8)
        .count();
    assert!(get_local_count >= 3,
        "expected at least 3 GetLocal (condition, body, increment), got {get_local_count}");
    // SetLocal for i = i + 1
    assert!(c.windows(3).any(|w| w[0] == ByteCode::SetLocal as u8),
        "for with increment should have SetLocal");
    // Should have Loop instructions (back to condition + back to increment)
    let loop_count = c.iter().filter(|&&b| b == ByteCode::Loop as u8).count();
    assert_eq!(loop_count, 2, "for-loop should emit 2 Loop instructions, got {loop_count}");
    // Constants: 0, 5, 1
    assert!(constants.contains(&Value::Integer(0)));
    assert!(constants.contains(&Value::Integer(5)));
    assert!(constants.contains(&Value::Integer(1)));
}

#[test]
fn test_for_statement_no_increment() {
    // `for (var i = 0; i < 3;) print i;` — no increment clause
    let c = codes("for (var i = 0; i < 3;) print i;");
    // Should have exactly 1 Loop (back to condition), no increment loop
    let loop_count = c.iter().filter(|&&b| b == ByteCode::Loop as u8).count();
    assert_eq!(loop_count, 1, "for without increment should have exactly 1 Loop, got {loop_count}");
    assert!(c.iter().any(|&b| b == ByteCode::JumpIfFalse as u8));
}

#[test]
fn test_for_statement_no_condition() {
    // `for (var i = 0;; i = i + 1) print i;` — no condition (infinite)
    let c = codes("for (var i = 0;; i = i + 1) print i;");
    // No JumpIfFalse (no condition to check)
    assert!(!c.iter().any(|&b| b == ByteCode::JumpIfFalse as u8),
        "for without condition should not have JumpIfFalse");
    // Should have 2 Loop instructions
    let loop_count = c.iter().filter(|&&b| b == ByteCode::Loop as u8).count();
    assert_eq!(loop_count, 2, "for with increment but no condition: 2 Loops, got {loop_count}");
}

#[test]
fn test_for_statement_variable_decl_in_initializer() {
    // `for (var i = 0; i < 10; i = i + 1) { print i; }`
    // The for-loop creates its own scope — i should be cleaned up by end_scope
    let c = codes("for (var i = 0; i < 10; i = i + 1) { print i; }");
    // After the for-loop, i should be popped (end_scope cleanup)
    // The last few bytes should be: ..., Pop (cleanup i), Return
    let last_bytes = &c[c.len() - 3..];
    // Should end with Pop (cleanup i) + Return
    assert_eq!(last_bytes[1], ByteCode::Pop as u8, "expected Pop for local cleanup before Return");
    assert_eq!(last_bytes[2], ByteCode::Return as u8);
}

// ------------------------------------------------------------------------
//  Control flow — error cases
// ------------------------------------------------------------------------

#[test]
fn test_if_missing_parens() {
    assert_err("if true) 1;");
}

#[test]
fn test_if_missing_condition() {
    assert_err("if ();");
}

#[test]
fn test_while_missing_parens() {
    assert_err("while true) 1;");
}

#[test]
fn test_while_missing_condition() {
    assert_err("while () 1;");
}

#[test]
fn test_for_missing_parens() {
    assert_err("for var i = 0; i < 10; i = i + 1) print i;");
}

// ------------------------------------------------------------------------
//  Error cases
// ------------------------------------------------------------------------

#[test]
fn test_missing_semicolon() {
    assert_err("42");
}

#[test]
fn test_unterminated_grouping() {
    assert_err("(1 + 2;");
}

#[test]
fn test_missing_expression_after_operator() {
    assert_err("1 + ;");
}

#[test]
fn test_more_errors() {
    let source = r#"
var a = ;
print 1 + ;
var b = ;
    "#;
    let res = compile(source);
    if let Err(CompileError::Parse(es)) = res {
        assert_eq!(es.len(), 3);
    }
}