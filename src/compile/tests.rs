use super::parse::ParseErrorKind;
use super::*;
use crate::{ByteCode, Chunk, Instruction, ObjectHandle, ObjectHeap};

// ------------------------------------------------------------------------
//  Helpers
// ------------------------------------------------------------------------

/// Compile source and return (chunk, heap) so constants can be inspected.
fn compile_with_heap(source: &str) -> (Chunk, ObjectHeap) {
    let mut obj_heap = ObjectHeap::new();
    let h = compile(source, &mut obj_heap).expect("compilation should succeed");
    let mut chunk = Chunk::new();
    std::mem::swap(&mut chunk, &mut obj_heap.get_mut(h).as_function_mut().expect("must fun").chunk);
    (chunk, obj_heap)
}

/// Compile source and return just the code vector.
fn codes(source: &str) -> Vec<u8> {
    compile_with_heap(source).0.codes
}

/// Get integer value from a constant handle.
fn const_int(heap: &ObjectHeap, h: ObjectHandle) -> i64 {
    *heap.get_integer_instance(h).expect("int")
}

/// Get float value from a constant handle.
fn const_float(heap: &ObjectHeap, h: ObjectHandle) -> f64 {
    *heap.get_float_instance(h).expect("float")
}

/// Get string value from a constant handle.
fn const_string(heap: &ObjectHeap, h: ObjectHandle) -> String {
    heap.get_string_instance(h).expect("str").as_str().to_string()
}

/// Check if a constant handle contains the given integer.
fn is_const_int(heap: &ObjectHeap, h: ObjectHandle, expected: i64) -> bool {
    *heap.get_integer_instance(h).expect("int") == expected
}

/// Assert that source fails to compile.
fn assert_err(source: &str) {
    let mut obj_heap = ObjectHeap::new();
    assert!(compile(source, &mut obj_heap).is_err(), "expected compilation error for: {source:?}");
}

/// Decode every instruction from a chunk into a Vec.
fn instructions(chunk: &Chunk, heap: &ObjectHeap) -> Vec<Instruction> {
    let mut ip = 0;
    let mut insts = Vec::new();
    while ip < chunk.codes.len() {
        insts.push(chunk.read_instruction(&mut ip, heap).unwrap());
    }
    insts
}

// ------------------------------------------------------------------------
//  Number literals
// ------------------------------------------------------------------------

#[test]
fn test_integer_literal() {
    let (chunk, heap) = compile_with_heap("42;");
    let codes = &chunk.codes;
    // Constant(0): opcode + u16
    assert_eq!(&codes[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(const_int(&heap, chunk.constants[0]), 42);
    assert_eq!(codes[3], ByteCode::Pop as u8);
    assert_eq!(*codes.last().unwrap(), ByteCode::Return as u8);
}

#[test]
fn test_decimal_literal() {
    let (chunk, heap) = compile_with_heap("3.14;");
    assert!((const_float(&heap, chunk.constants[0]) - 3.14).abs() < 0.001);
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
    let (chunk, heap) = compile_with_heap("\"hello\";");
    assert_eq!(const_string(&heap, chunk.constants[0]), "hello");
}

#[test]
fn test_empty_string() {
    let (chunk, heap) = compile_with_heap("\"\";");
    assert_eq!(const_string(&heap, chunk.constants[0]), "");
}

#[test]
fn test_string_escape_newline() {
    let (chunk, heap) = compile_with_heap("\"a\\nb\";");
    assert_eq!(const_string(&heap, chunk.constants[0]), "a\nb");
}

#[test]
fn test_string_escape_tab() {
    let (chunk, heap) = compile_with_heap("\"a\\tb\";");
    assert_eq!(const_string(&heap, chunk.constants[0]), "a\tb");
}

#[test]
fn test_string_escape_quote() {
    let (chunk, heap) = compile_with_heap("\"a\\\"b\";");
    assert_eq!(const_string(&heap, chunk.constants[0]), "a\"b");
}

#[test]
fn test_string_escape_backslash() {
    let (chunk, heap) = compile_with_heap("\"a\\\\b\";");
    assert_eq!(const_string(&heap, chunk.constants[0]), "a\\b");
}

#[test]
fn test_string_escape_carriage_return() {
    let (chunk, heap) = compile_with_heap("\"a\\rb\";");
    assert_eq!(const_string(&heap, chunk.constants[0]), "a\rb");
}

#[test]
fn test_string_escape_null() {
    let (chunk, heap) = compile_with_heap("\"a\\0b\";");
    assert_eq!(const_string(&heap, chunk.constants[0]), "a\0b");
}

#[test]
fn test_string_escape_multiple() {
    let (chunk, heap) = compile_with_heap("\"\\n\\t\\r\\\\\\\"\";");
    assert_eq!(const_string(&heap, chunk.constants[0]), "\n\t\r\\\"");
}

#[test]
fn test_string_invalid_escape_is_error() {
    let mut obj_heap = ObjectHeap::new();
    match compile("\"hello\\xworld\";", &mut obj_heap) {
        Err(CompileError::Parse(errors)) => {
            assert!(errors.iter().any(|e| matches!(e.kind, ParseErrorKind::InvalidEscape('x'))), "expected InvalidEscape error");
        }
        Err(CompileError::Scan(_)) => panic!("unexpected scan error"),
        Ok(_) => panic!("expected compilation to fail for invalid escape"),
    }
}

// ------------------------------------------------------------------------
//  Unary expressions
// ------------------------------------------------------------------------

#[test]
fn test_unary_negate() {
    let c = codes("-5;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(c[3], ByteCode::Negate as u8);
}

#[test]
fn test_unary_not() {
    let c = codes("!true;");
    assert_eq!(c[0], ByteCode::True as u8);
    assert_eq!(c[1], ByteCode::Not as u8);
}

// ------------------------------------------------------------------------
//  Binary arithmetic
// ------------------------------------------------------------------------

#[test]
fn test_addition() {
    let c = codes("1 + 2;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[6], ByteCode::Add as u8);
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

#[test]
fn test_modulo() {
    let c = codes("7 % 3;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[6], ByteCode::Mod as u8);
}

#[test]
fn test_floordiv() {
    let c = codes("8 ~/ 3;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[6], ByteCode::FloorDiv as u8);
}

// ------------------------------------------------------------------------
//  Precedence
// ------------------------------------------------------------------------

#[test]
fn test_precedence_mul_before_add() {
    let c = codes("1 + 2 * 3;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(&c[6..9], &[ByteCode::Constant as u8, 2, 0]);
    assert_eq!(c[9], ByteCode::Mul as u8);
    assert_eq!(c[10], ByteCode::Add as u8);
}

#[test]
fn test_grouping_overrides_precedence() {
    let c = codes("(1 + 2) * 3;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[6], ByteCode::Add as u8);
    assert_eq!(&c[7..10], &[ByteCode::Constant as u8, 2, 0]);
    assert_eq!(c[10], ByteCode::Mul as u8);
}

#[test]
fn test_precedence_mod_before_add() {
    let c = codes("1 + 7 % 3;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(&c[6..9], &[ByteCode::Constant as u8, 2, 0]);
    assert_eq!(c[9], ByteCode::Mod as u8);
    assert_eq!(c[10], ByteCode::Add as u8);
}

#[test]
fn test_precedence_floordiv_before_add() {
    let c = codes("1 + 20 ~/ 3;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(&c[6..9], &[ByteCode::Constant as u8, 2, 0]);
    assert_eq!(c[9], ByteCode::FloorDiv as u8);
    assert_eq!(c[10], ByteCode::Add as u8);
}

#[test]
fn test_precedence_mul_and_mod_same_level() {
    // At same precedence, left-associative: (a * b) % c
    let c = codes("3 * 7 % 5;");
    // 3, 7 → Mul, 5 → Mod (left-to-right since left-assoc at Factor level)
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[6], ByteCode::Mul as u8);
    assert_eq!(&c[7..10], &[ByteCode::Constant as u8, 2, 0]);
    assert_eq!(c[10], ByteCode::Mod as u8);
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
    let c = codes("1 <= 2;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[6], ByteCode::LessEqual as u8);
}

#[test]
fn test_greater_equal() {
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
    let c = codes("print(42);");
    assert_eq!(&c[0..3], &[ByteCode::GetGlobal as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(&c[6..8], &[ByteCode::Call as u8, 1]);
    assert_eq!(c[8], ByteCode::Pop as u8);
}

#[test]
fn test_print_expression() {
    let c = codes("print(1 + 2);");
    assert_eq!(&c[0..3], &[ByteCode::GetGlobal as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(&c[6..9], &[ByteCode::Constant as u8, 2, 0]);
    assert_eq!(c[9], ByteCode::Add as u8);
    assert_eq!(&c[10..12], &[ByteCode::Call as u8, 1]);
    assert_eq!(c[12], ByteCode::Pop as u8);
}

// ------------------------------------------------------------------------
//  Multiple statements
// ------------------------------------------------------------------------

#[test]
fn test_multiple_statements() {
    let c = codes("1; 2;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(c[3], ByteCode::Pop as u8);
    assert_eq!(&c[4..7], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[7], ByteCode::Pop as u8);
    assert_eq!(c[8], ByteCode::Nil as u8);
    assert_eq!(c[9], ByteCode::Return as u8);
}

// ------------------------------------------------------------------------
//  Complex expression
// ------------------------------------------------------------------------

#[test]
fn test_complex_expression() {
    let c = codes("-5 * (3 + 2) / 4;");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]); // 5
    assert_eq!(c[3], ByteCode::Negate as u8);
    assert_eq!(&c[4..7], &[ByteCode::Constant as u8, 1, 0]); // 3
    assert_eq!(&c[7..10], &[ByteCode::Constant as u8, 2, 0]); // 2
    assert_eq!(c[10], ByteCode::Add as u8);
    assert_eq!(c[11], ByteCode::Mul as u8);
    assert_eq!(&c[12..15], &[ByteCode::Constant as u8, 3, 0]); // 4
    assert_eq!(c[15], ByteCode::Div as u8);
}

// ------------------------------------------------------------------------
//  Local variables
// ------------------------------------------------------------------------

#[test]
fn test_local_var_declaration() {
    let c = codes("{ var x = 42; }");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(c[3], ByteCode::Pop as u8);
    assert_eq!(*c.last().unwrap(), ByteCode::Return as u8);
}

#[test]
fn test_local_var_without_initializer() {
    let c = codes("{ var x; }");
    assert_eq!(c[0], ByteCode::Nil as u8);
    assert_eq!(c[1], ByteCode::Pop as u8);
    assert_eq!(*c.last().unwrap(), ByteCode::Return as u8);
}

#[test]
fn test_local_var_read() {
    let c = codes("{ var x = 5; print(x); }");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::GetGlobal as u8, 1, 0]);
    assert_eq!(&c[6..9], &[ByteCode::GetLocal as u8, 1, 0]);
    assert_eq!(&c[9..11], &[ByteCode::Call as u8, 1]);
    assert_eq!(c[11], ByteCode::Pop as u8);
    assert_eq!(c[12], ByteCode::Pop as u8);
}

#[test]
fn test_local_var_assignment() {
    let c = codes("{ var x = 42; x = 99; }");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(&c[6..9], &[ByteCode::SetLocal as u8, 1, 0]);
    assert_eq!(c[9], ByteCode::Pop as u8);
    assert_eq!(c[10], ByteCode::Pop as u8);
}

#[test]
fn test_multiple_locals() {
    let c = codes("{ var a = 1; var b = 2; print(a + b); }");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(&c[6..9], &[ByteCode::GetGlobal as u8, 2, 0]);
    assert_eq!(&c[9..12], &[ByteCode::GetLocal as u8, 1, 0]);
    assert_eq!(&c[12..15], &[ByteCode::GetLocal as u8, 2, 0]);
    assert_eq!(c[15], ByteCode::Add as u8);
    assert_eq!(&c[16..18], &[ByteCode::Call as u8, 1]);
    assert_eq!(c[18], ByteCode::Pop as u8);
    assert_eq!(c[19], ByteCode::Pop as u8);
    assert_eq!(c[20], ByteCode::Pop as u8);
}

#[test]
fn test_local_in_arithmetic_expression() {
    let c = codes("{ var a = 10; var b = 20; a * b + 5; }");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(&c[6..9], &[ByteCode::GetLocal as u8, 1, 0]);
    assert_eq!(&c[9..12], &[ByteCode::GetLocal as u8, 2, 0]);
    assert_eq!(c[12], ByteCode::Mul as u8);
    assert_eq!(&c[13..16], &[ByteCode::Constant as u8, 2, 0]);
    assert_eq!(c[16], ByteCode::Add as u8);
    assert_eq!(c[17], ByteCode::Pop as u8);
    assert_eq!(c[18], ByteCode::Pop as u8);
    assert_eq!(c[19], ByteCode::Pop as u8);
}

#[test]
fn test_nested_block_locals() {
    let c = codes("{ var a = 1; { var b = 2; print(a + b); } print(a); }");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(&c[6..9], &[ByteCode::GetGlobal as u8, 2, 0]);
    assert_eq!(&c[9..12], &[ByteCode::GetLocal as u8, 1, 0]);
    assert_eq!(&c[12..15], &[ByteCode::GetLocal as u8, 2, 0]);
    assert_eq!(c[15], ByteCode::Add as u8);
    assert_eq!(&c[16..18], &[ByteCode::Call as u8, 1]);
    assert_eq!(c[18], ByteCode::Pop as u8);
    assert_eq!(c[19], ByteCode::Pop as u8);
    assert_eq!(&c[20..23], &[ByteCode::GetGlobal as u8, 3, 0]);
    assert_eq!(&c[23..26], &[ByteCode::GetLocal as u8, 1, 0]);
    assert_eq!(&c[26..28], &[ByteCode::Call as u8, 1]);
    assert_eq!(c[28], ByteCode::Pop as u8);
    assert_eq!(c[29], ByteCode::Pop as u8);
}

#[test]
fn test_slot_reuse_after_block_exit() {
    let c = codes("{ var a = 1; { var b = 2; } var c = 3; print(c); }");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(c[6], ByteCode::Pop as u8);
    assert_eq!(&c[7..10], &[ByteCode::Constant as u8, 2, 0]);
    assert_eq!(&c[10..13], &[ByteCode::GetGlobal as u8, 3, 0]);
    assert_eq!(&c[13..16], &[ByteCode::GetLocal as u8, 2, 0]);
    assert_eq!(&c[16..18], &[ByteCode::Call as u8, 1]);
    assert_eq!(c[18], ByteCode::Pop as u8);
    assert_eq!(c[19], ByteCode::Pop as u8);
    assert_eq!(c[20], ByteCode::Pop as u8);
}

#[test]
fn test_local_assignment_is_expression() {
    let c = codes("{ var x = 1; var y = (x = 5); }");
    assert_eq!(&c[0..3], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(&c[3..6], &[ByteCode::Constant as u8, 1, 0]);
    assert_eq!(&c[6..9], &[ByteCode::SetLocal as u8, 1, 0]);
    assert_eq!(c[9], ByteCode::Pop as u8);
    assert_eq!(c[10], ByteCode::Pop as u8);
}

// ------------------------------------------------------------------------
//  Local variable error cases
// ------------------------------------------------------------------------

#[test]
fn test_local_self_reference_is_error() {
    let source = "{ var a = a; }";
    let mut obj_heap = ObjectHeap::new();
    match compile(source, &mut obj_heap) {
        Err(CompileError::Parse(errors)) => {
            assert!(!errors.is_empty(), "expected parse errors");
        }
        Err(CompileError::Scan(_)) => panic!("unexpected scan error"),
        Ok(_) => panic!("expected compilation to fail for self-referencing local"),
    }
}

#[test]
fn test_duplicate_local_is_error() {
    let source = "{ var a = 1; var a = 2; }";
    let mut obj_heap = ObjectHeap::new();
    match compile(source, &mut obj_heap) {
        Err(CompileError::Parse(errors)) => {
            assert!(errors.iter().any(|e| matches!(e.kind, ParseErrorKind::VariableRedefine(_))), "expected VariableRedefine error");
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
    let c = codes("if true { 1; }");
    assert_eq!(c[0], ByteCode::True as u8);
    assert_eq!(c[1], ByteCode::JumpIfFalse as u8);
    assert_eq!(c[4], ByteCode::Pop as u8);
    assert_eq!(&c[5..8], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(c[8], ByteCode::Pop as u8);
    assert_eq!(c[9], ByteCode::Jump as u8);
    assert_eq!(*c.last().unwrap(), ByteCode::Return as u8);
}

#[test]
fn test_if_else_statement() {
    let c = codes("if true { 1; } else { 2; }");
    assert_eq!(c[0], ByteCode::True as u8);
    assert_eq!(c[1], ByteCode::JumpIfFalse as u8);
    assert_eq!(c[4], ByteCode::Pop as u8);
    assert_eq!(&c[5..8], &[ByteCode::Constant as u8, 0, 0]);
    assert_eq!(c[8], ByteCode::Pop as u8);
    assert_eq!(c[9], ByteCode::Jump as u8);
}

#[test]
fn test_if_statement_condition_is_falsey_jumps() {
    let c = codes("if false { 1; }");
    assert_eq!(c[0], ByteCode::False as u8);
    assert_eq!(c[1], ByteCode::JumpIfFalse as u8);
}

#[test]
fn test_if_else_constants() {
    let (chunk, heap) = compile_with_heap("if true { 42; } else { 99; }");
    // Check that both constants exist
    let has_42 = chunk.constants.iter().any(|&h| is_const_int(&heap, h, 42));
    let has_99 = chunk.constants.iter().any(|&h| is_const_int(&heap, h, 99));
    assert!(has_42);
    assert!(has_99);
}

#[test]
fn test_nested_if() {
    let (chunk, heap) = compile_with_heap("if true { if false { 1; } else { 2; } }");
    let has_1 = chunk.constants.iter().any(|&h| is_const_int(&heap, h, 1));
    let has_2 = chunk.constants.iter().any(|&h| is_const_int(&heap, h, 2));
    assert!(has_1);
    assert!(has_2);
}

// ------------------------------------------------------------------------
//  Control flow — while
// ------------------------------------------------------------------------

#[test]
fn test_while_statement() {
    let c = codes("while false { 1; }");
    assert_eq!(c[0], ByteCode::False as u8);
    assert_eq!(c[1], ByteCode::JumpIfFalse as u8);
    let has_loop = c.iter().any(|&b| b == ByteCode::Loop as u8);
    assert!(has_loop);
}

#[test]
fn test_while_statement_loops_back() {
    let c = codes("while true { 1; }");
    assert_eq!(c[0], ByteCode::True as u8);
    assert_eq!(c[1], ByteCode::JumpIfFalse as u8);
    assert!(c.iter().any(|&b| b == ByteCode::Loop as u8));
}

#[test]
fn test_while_with_condition_variable() {
    let c = codes("{ var x = 0; while x < 3 { print(x); x = x + 1; } }");
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
    let c = codes("for (;;) { 1; }");
    assert!(c.iter().any(|&b| b == ByteCode::Loop as u8));
    assert!(!c.iter().any(|&b| b == ByteCode::JumpIfFalse as u8));
}

#[test]
fn test_for_statement_with_condition() {
    let c = codes("for (; true ;) { 1; }");
    assert_eq!(c[0], ByteCode::True as u8);
    assert_eq!(c[1], ByteCode::JumpIfFalse as u8);
    assert!(c.iter().any(|&b| b == ByteCode::Loop as u8));
}

#[test]
fn test_for_statement_with_initializer() {
    let (chunk, _heap) = compile_with_heap("for (var i = 0; i < 5; i = i + 1) { print(i); }");
    let c = &chunk.codes;
    let get_local_count = c.windows(3).filter(|w| w[0] == ByteCode::GetLocal as u8).count();
    assert!(get_local_count >= 3);
    assert!(c.windows(3).any(|w| w[0] == ByteCode::SetLocal as u8));
    let loop_count = c.iter().filter(|&&b| b == ByteCode::Loop as u8).count();
    assert_eq!(loop_count, 2);
}

#[test]
fn test_for_statement_no_increment() {
    let c = codes("for (var i = 0; i < 3;) { print(i); }");
    let loop_count = c.iter().filter(|&&b| b == ByteCode::Loop as u8).count();
    assert_eq!(loop_count, 1);
    assert!(c.iter().any(|&b| b == ByteCode::JumpIfFalse as u8));
}

#[test]
fn test_for_statement_no_condition() {
    let c = codes("for (var i = 0;; i = i + 1) { print(i); }");
    assert!(!c.iter().any(|&b| b == ByteCode::JumpIfFalse as u8));
    let loop_count = c.iter().filter(|&&b| b == ByteCode::Loop as u8).count();
    assert_eq!(loop_count, 2);
}

#[test]
fn test_for_statement_variable_decl_in_initializer() {
    let c = codes("for (var i = 0; i < 10; i = i + 1) { print(i); }");
    let last_bytes = &c[c.len() - 4..];
    assert_eq!(last_bytes[1], ByteCode::Pop as u8);
    assert_eq!(last_bytes[2], ByteCode::Nil as u8);
    assert_eq!(last_bytes[3], ByteCode::Return as u8);
}

// ------------------------------------------------------------------------
//  Control flow — error cases
// ------------------------------------------------------------------------

#[test]
fn test_if_missing_parens() {
    assert_err("if true 1;");
}
#[test]
fn test_if_missing_condition() {
    assert_err("if {};");
}
#[test]
fn test_while_missing_parens() {
    assert_err("while true 1;");
}
#[test]
fn test_while_missing_condition() {
    assert_err("while {};");
}
#[test]
fn test_for_missing_parens() {
    assert_err("for var i = 0; i < 10; i = i + 1) print(i);");
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
print(1 + );
var b = ;
    "#;
    let mut obj_heap = ObjectHeap::new();
    let res = compile(source, &mut obj_heap);
    if let Err(CompileError::Parse(es)) = res {
        assert_eq!(es.len(), 3);
    }
}

// ------------------------------------------------------------------------
//  Function declarations
// ------------------------------------------------------------------------

#[test]
fn test_empty_function_declaration() {
    let (chunk, heap) = compile_with_heap("fun foo() {}");
    assert_eq!(chunk.codes[0], ByteCode::Closure as u8);
    let const_idx = u16::from_le_bytes([chunk.codes[1], chunk.codes[2]]) as usize;
    let fn_handle = chunk.constants[const_idx];
    // upvalue count = 0
    assert_eq!(chunk.codes[3], 0u8);
    assert_eq!(chunk.codes[4], ByteCode::DefineGlobal as u8);
    // Verify the function object's chunk has Nil; Return
    let fn_chunk = &heap.get(fn_handle).as_function().unwrap().chunk;
    assert_eq!(fn_chunk.codes.len(), 2);
    assert_eq!(fn_chunk.codes[0], ByteCode::Nil as u8);
    assert_eq!(fn_chunk.codes[1], ByteCode::Return as u8);
    assert_eq!(*chunk.codes.last().unwrap(), ByteCode::Return as u8);
}

#[test]
fn test_function_with_return_value() {
    let (chunk, heap) = compile_with_heap("fun add(a, b) { return a + b; }");
    let fn_handle = chunk.constants[0];
    let fn_chunk = &heap.get(fn_handle).as_function().unwrap().chunk;
    assert_eq!(fn_chunk.codes[0], ByteCode::GetLocal as u8);
    assert_eq!(u16::from_le_bytes([fn_chunk.codes[1], fn_chunk.codes[2]]), 1);
    assert_eq!(fn_chunk.codes[3], ByteCode::GetLocal as u8);
    assert_eq!(u16::from_le_bytes([fn_chunk.codes[4], fn_chunk.codes[5]]), 2);
    assert_eq!(fn_chunk.codes[6], ByteCode::Add as u8);
    assert_eq!(fn_chunk.codes[7], ByteCode::Return as u8);
    let fn_obj = heap.get(fn_handle).as_function().unwrap();
    assert_eq!(fn_obj.arity, 2);
    assert_eq!(fn_obj.name.as_str(), "add");
}

#[test]
fn test_function_with_implicit_return() {
    let (chunk, heap) = compile_with_heap("fun f() { 42; }");
    let fn_handle = chunk.constants[0];
    let fn_chunk = &heap.get(fn_handle).as_function().unwrap().chunk;
    assert_eq!(fn_chunk.codes[0], ByteCode::Constant as u8);
    assert_eq!(fn_chunk.codes[3], ByteCode::Pop as u8);
    assert_eq!(fn_chunk.codes[4], ByteCode::Nil as u8);
    assert_eq!(fn_chunk.codes[5], ByteCode::Return as u8);
}

#[test]
fn test_function_call_expression() {
    let (chunk, _heap) = compile_with_heap("fun f() {} f();");
    assert_eq!(chunk.codes[0], ByteCode::Closure as u8);
    assert_eq!(chunk.codes[3], 0u8);
    assert_eq!(chunk.codes[4], ByteCode::DefineGlobal as u8);
    assert_eq!(chunk.codes[7], ByteCode::GetGlobal as u8);
    assert_eq!(chunk.codes[10], ByteCode::Call as u8);
    assert_eq!(chunk.codes[11], 0u8);
    assert_eq!(chunk.codes[12], ByteCode::Pop as u8);
    assert_eq!(chunk.codes[13], ByteCode::Nil as u8);
    assert_eq!(chunk.codes[14], ByteCode::Return as u8);
}

#[test]
fn test_function_call_with_args() {
    let (chunk, _heap) = compile_with_heap("fun add(a, b) { return a + b; } add(1, 2);");
    let mut pos = 7;
    assert_eq!(chunk.codes[pos], ByteCode::GetGlobal as u8);
    pos += 3;
    assert_eq!(chunk.codes[pos], ByteCode::Constant as u8);
    pos += 3;
    assert_eq!(chunk.codes[pos], ByteCode::Constant as u8);
    pos += 3;
    assert_eq!(chunk.codes[pos], ByteCode::Call as u8);
    assert_eq!(chunk.codes[pos + 1], 2u8);
    assert_eq!(chunk.codes[pos + 2], ByteCode::Pop as u8);
}

#[test]
fn test_nested_function_call() {
    let (chunk, heap) = compile_with_heap("fun f() { return 1; } fun g() { return f(); }");
    // Find g's function: it's the second function in constants
    let mut g_handle = None;
    for &h in &chunk.constants {
        if let Some(f) = heap.get(h).as_function() {
            if f.name.as_str() == "g" {
                g_handle = Some(h);
                break;
            }
        }
    }
    let g_handle = g_handle.expect("g function not found");
    let g_chunk = &heap.get(g_handle).as_function().unwrap().chunk;
    assert_eq!(g_chunk.codes[0], ByteCode::GetGlobal as u8);
    assert_eq!(g_chunk.codes[3], ByteCode::Call as u8);
    assert_eq!(g_chunk.codes[4], 0u8);
    assert_eq!(g_chunk.codes[5], ByteCode::Return as u8);
}

#[test]
fn test_return_in_top_level_is_error() {
    let mut obj_heap = ObjectHeap::new();
    match compile("return 5;", &mut obj_heap) {
        Err(CompileError::Parse(errors)) => {
            assert!(errors.iter().any(|e| matches!(e.kind, ParseErrorKind::ReturnInTop)), "expected ReturnInTop error");
        }
        _ => panic!("expected compilation to fail for top-level return"),
    }
}

// ------------------------------------------------------------------------
//  Closures & Upvalues
// ------------------------------------------------------------------------

#[test]
fn test_closure_single_upvalue_capture() {
    let (script_chunk, heap) = compile_with_heap("fun outer() { var x = 1; fun inner() { return x; } return inner; }");
    let script_insts = instructions(&script_chunk, &heap);
    let outer_fn_handle = match &script_insts[0] {
        Instruction::Closure { function, upvalues } => {
            assert!(upvalues.is_empty());
            *function
        }
        _ => panic!("expected Closure"),
    };
    let outer_fn = heap.get(outer_fn_handle).as_function().unwrap();
    let outer_insts = instructions(&outer_fn.chunk, &heap);

    let (inner_fn_handle, upvalues) = outer_insts
        .iter()
        .find_map(|inst| if let Instruction::Closure { function, upvalues } = inst { Some((*function, upvalues.clone())) } else { None })
        .expect("outer should contain a Closure for inner");

    assert_eq!(upvalues.len(), 1);
    assert!(upvalues[0].is_local);
    assert_eq!(upvalues[0].index, 1);

    let inner_fn = heap.get(inner_fn_handle).as_function().unwrap();
    let inner_insts = instructions(&inner_fn.chunk, &heap);
    assert!(inner_insts.iter().any(|i| matches!(i, Instruction::GetUpvalue(0))));
    assert!(matches!(inner_insts.last().unwrap(), Instruction::Return));
}

#[test]
fn test_closure_set_upvalue_bytecode() {
    let (script_chunk, heap) = compile_with_heap("fun outer() { var i = 0; fun inc() { i = i + 1; } }");
    let script_insts = instructions(&script_chunk, &heap);
    let outer_fn_handle = match &script_insts[0] {
        Instruction::Closure { function, .. } => *function,
        _ => panic!("expected Closure"),
    };
    let outer_fn = heap.get(outer_fn_handle).as_function().unwrap();
    let outer_insts = instructions(&outer_fn.chunk, &heap);
    let inner_fn_handle = outer_insts
        .iter()
        .find_map(|inst| if let Instruction::Closure { function, .. } = inst { Some(*function) } else { None })
        .expect("outer should contain a Closure for inc");

    let inner_fn = heap.get(inner_fn_handle).as_function().unwrap();
    let inner_insts = instructions(&inner_fn.chunk, &heap);
    assert!(inner_insts.iter().any(|i| matches!(i, Instruction::GetUpvalue(_))));
    assert!(inner_insts.iter().any(|i| matches!(i, Instruction::SetUpvalue(_))));
}

#[test]
fn test_close_upvalue_in_block_scope() {
    let (script_chunk, heap) = compile_with_heap("fun outer() { var x = 1; { var y = 2; fun f() { return y; } } return x; }");
    let script_insts = instructions(&script_chunk, &heap);
    let outer_fn_handle = match &script_insts[0] {
        Instruction::Closure { function, .. } => *function,
        _ => panic!("expected Closure"),
    };
    let outer_fn = heap.get(outer_fn_handle).as_function().unwrap();
    let outer_insts = instructions(&outer_fn.chunk, &heap);
    assert!(outer_insts.iter().any(|i| matches!(i, Instruction::CloseUpvalue)));
}

#[test]
fn test_closure_captures_parameter() {
    let (script_chunk, heap) = compile_with_heap("fun makeAdder(x) { fun adder(y) { return x + y; } return adder; }");
    let script_insts = instructions(&script_chunk, &heap);
    let outer_fn_handle = match &script_insts[0] {
        Instruction::Closure { function, .. } => *function,
        _ => panic!("expected Closure"),
    };
    let outer_fn = heap.get(outer_fn_handle).as_function().unwrap();
    let outer_insts = instructions(&outer_fn.chunk, &heap);
    let (_, upvalues) = outer_insts
        .iter()
        .find_map(|inst| if let Instruction::Closure { function, upvalues } = inst { Some((*function, upvalues.clone())) } else { None })
        .expect("makeAdder should contain a Closure for adder");

    assert_eq!(upvalues.len(), 1);
    assert!(upvalues[0].is_local);
    assert_eq!(upvalues[0].index, 1);
}

#[test]
fn test_nested_closure_upvalue_chain() {
    let (script_chunk, heap) =
        compile_with_heap("fun outer() { var a = 10; fun middle() { fun inner() { return a; } return inner; } return middle; }");
    let script_insts = instructions(&script_chunk, &heap);
    let outer_fn_handle = match &script_insts[0] {
        Instruction::Closure { function, .. } => *function,
        _ => panic!("expected Closure"),
    };
    let outer_fn = heap.get(outer_fn_handle).as_function().unwrap();
    let outer_insts = instructions(&outer_fn.chunk, &heap);

    let middle_fn_handle = outer_insts
        .iter()
        .find_map(|inst| {
            if let Instruction::Closure { function, upvalues } = inst {
                assert_eq!(upvalues.len(), 1);
                assert!(upvalues[0].is_local);
                assert_eq!(upvalues[0].index, 1);
                Some(*function)
            } else {
                None
            }
        })
        .expect("outer should contain a Closure for middle");

    let middle_fn = heap.get(middle_fn_handle).as_function().unwrap();
    let middle_insts = instructions(&middle_fn.chunk, &heap);
    let (inner_fn_handle, inner_upvalues) = middle_insts
        .iter()
        .find_map(|inst| if let Instruction::Closure { function, upvalues } = inst { Some((*function, upvalues.clone())) } else { None })
        .expect("middle should contain a Closure for inner");

    assert_eq!(inner_upvalues.len(), 1);
    assert!(!inner_upvalues[0].is_local);
    assert_eq!(inner_upvalues[0].index, 0);

    let inner_fn = heap.get(inner_fn_handle).as_function().unwrap();
    let inner_insts = instructions(&inner_fn.chunk, &heap);
    assert!(inner_insts.iter().any(|i| matches!(i, Instruction::GetUpvalue(0))));
}

#[test]
fn test_closure_multiple_upvalues() {
    let (script_chunk, heap) = compile_with_heap("fun outer() { var a = 1; var b = 2; fun sum() { return a + b; } return sum; }");
    let script_insts = instructions(&script_chunk, &heap);
    let outer_fn_handle = match &script_insts[0] {
        Instruction::Closure { function, .. } => *function,
        _ => panic!("expected Closure"),
    };
    let outer_fn = heap.get(outer_fn_handle).as_function().unwrap();
    let outer_insts = instructions(&outer_fn.chunk, &heap);
    let (sum_fn_handle, upvalues) = outer_insts
        .iter()
        .find_map(|inst| if let Instruction::Closure { function, upvalues } = inst { Some((*function, upvalues.clone())) } else { None })
        .expect("outer should contain Closure for sum");

    assert_eq!(upvalues.len(), 2);
    assert!(upvalues[0].is_local);
    assert!(upvalues[1].is_local);
    assert_eq!(upvalues[0].index, 1);
    assert_eq!(upvalues[1].index, 2);

    let sum_fn = heap.get(sum_fn_handle).as_function().unwrap();
    let sum_insts = instructions(&sum_fn.chunk, &heap);
    assert!(sum_insts.iter().any(|i| matches!(i, Instruction::GetUpvalue(0))));
    assert!(sum_insts.iter().any(|i| matches!(i, Instruction::GetUpvalue(1))));
}

// ------------------------------------------------------------------------
//  Classes & Instances
// ------------------------------------------------------------------------

#[test]
fn test_class_declaration_bytecode() {
    let c = codes("class Toast {}");
    assert_eq!(c[0], ByteCode::Class as u8);
    assert_eq!(c[3], ByteCode::DefineGlobal as u8);
    assert_eq!(c[6], ByteCode::GetGlobal as u8);
    assert_eq!(c[9], ByteCode::Pop as u8);
}

#[test]
fn test_class_with_method_bytecode() {
    let c = codes("class Scone { fun topping() {} }");
    assert!(c.iter().any(|&b| b == ByteCode::Class as u8));
    assert!(c.iter().any(|&b| b == ByteCode::Method as u8));
}

#[test]
fn test_property_get_and_set_bytecode() {
    let c = codes("{ var p = Pair(); p.first = 1; print(p.first); }");
    assert!(c.iter().any(|&b| b == ByteCode::GetProperty as u8));
    assert!(c.iter().any(|&b| b == ByteCode::SetProperty as u8));
}

#[test]
fn test_invoke_bytecode() {
    let c = codes("{ var s = Scone(); s.topping(\"berries\", \"cream\"); }");
    assert!(c.iter().any(|&b| b == ByteCode::GetProperty as u8));
    assert!(c.iter().any(|&b| b == ByteCode::Call as u8));
}

#[test]
fn test_self_parameter_is_slot_one() {
    let (chunk, heap) = compile_with_heap("class Foo { fun m(self) { return self; } }");
    let script_insts = instructions(&chunk, &heap);
    for inst in &script_insts {
        if let Instruction::Closure { function, .. } = inst {
            let fn_obj = heap.get(*function).as_function().unwrap();
            if fn_obj.name.as_str() == "m" {
                let fn_insts = instructions(&fn_obj.chunk, &heap);
                assert!(fn_insts.iter().any(|i| matches!(i, Instruction::GetLocal(1))));
                return;
            }
        }
    }
    panic!("method 'm' not found");
}

#[test]
fn test_inheritance_bytecode() {
    let c = codes("class Base {} class Derived extends Base {}");
    assert!(c.iter().any(|&b| b == ByteCode::Inherit as u8));
}

#[test]
fn test_list_literal_bytecode() {
    let c = codes("[1, 2, 3];");
    assert!(c.iter().any(|&b| b == ByteCode::BuildList as u8));
}

#[test]
fn test_empty_list_bytecode() {
    let c = codes("[];");
    assert!(c.iter().any(|&b| b == ByteCode::BuildList as u8));
}

#[test]
fn test_index_get_bytecode() {
    let c = codes("a[0];");
    assert!(c.iter().any(|&b| b == ByteCode::IndexGet as u8));
}

#[test]
fn test_index_set_bytecode() {
    let c = codes("a[0] = 1;");
    assert!(c.iter().any(|&b| b == ByteCode::IndexSet as u8));
}

#[test]
fn test_dict_literal_bytecode() {
    let c = codes("var d = {\"a\": 1, \"b\": 2};");
    assert!(c.iter().any(|&b| b == ByteCode::BuildDict as u8));
}

#[test]
fn test_empty_dict_bytecode() {
    let c = codes("var d = {};");
    assert!(c.iter().any(|&b| b == ByteCode::BuildDict as u8));
}

#[test]
fn test_dict_colon_required() {
    assert_err("var d = {\"a\" 1};");
}

// ------------------------------------------------------------------------
//  Break / Continue — compiler tests
// ------------------------------------------------------------------------

#[test]
fn test_while_break_emits_jump() {
    let c = codes("while true { break; }");
    // Should contain Jump (for break) and JumpIfFalse (for while exit)
    assert!(c.iter().any(|&b| b == ByteCode::Jump as u8));
    assert!(c.iter().any(|&b| b == ByteCode::JumpIfFalse as u8));
}

#[test]
fn test_while_continue_emits_loop() {
    let c = codes("while true { continue; }");
    // Should contain 2 Loop instructions: one for continue, one for back-edge
    let loop_count = c.iter().filter(|&&b| b == ByteCode::Loop as u8).count();
    assert_eq!(loop_count, 2);
}

#[test]
fn test_for_break_emits_jump() {
    let c = codes("for (;;) { break; }");
    assert!(c.iter().any(|&b| b == ByteCode::Jump as u8));
}

#[test]
fn test_for_continue_to_increment() {
    let c = codes("for (var i = 0; i < 10; i = i + 1) { continue; }");
    // With full for-loop clauses there should be 4 Loop instructions:
    // - 1 from the increment-back-to-condition
    // - 1 from the body-back-to-increment
    // - 1 from the continue statement itself
    let loop_count = c.iter().filter(|&&b| b == ByteCode::Loop as u8).count();
    assert_eq!(loop_count, 3);
}

#[test]
fn test_for_break_with_full_clauses() {
    let c = codes("for (var i = 0; i < 10; i = i + 1) { break; }");
    assert!(c.iter().any(|&b| b == ByteCode::Jump as u8));
    assert!(c.iter().any(|&b| b == ByteCode::Loop as u8));
    assert!(c.iter().any(|&b| b == ByteCode::JumpIfFalse as u8));
}

#[test]
fn test_break_outside_loop_error() {
    assert_err("break;");
}

#[test]
fn test_continue_outside_loop_error() {
    assert_err("continue;");
}

#[test]
fn test_break_inside_function_inside_while_is_error() {
    // break inside a nested function should NOT see the enclosing while loop.
    assert_err("while true { fun f() { break; } }");
}

#[test]
fn test_continue_inside_function_inside_while_is_error() {
    // continue inside a nested function should NOT see the enclosing while loop.
    assert_err("while true { fun f() { continue; } }");
}

#[test]
fn test_break_inside_if_inside_while() {
    // break inside an if statement inside a while loop — valid.
    let c = codes("while true { if true { break; } }");
    assert!(c.iter().any(|&b| b == ByteCode::Jump as u8));
}

#[test]
fn test_break_inside_nested_while() {
    let c = codes("while true { while true { break; } }");
    // Inner break should be a Jump, and outer while still has Loop + JumpIfFalse
    assert!(c.iter().any(|&b| b == ByteCode::Jump as u8));
    assert!(c.iter().any(|&b| b == ByteCode::Loop as u8));
}

#[test]
fn test_continue_in_for_without_increment() {
    let c = codes("for (var i = 0; i < 10;) { continue; }");
    // Without increment, continue goes back to the condition.
    // One Loop for back-edge + one Loop for continue = 2 total.
    let loop_count = c.iter().filter(|&&b| b == ByteCode::Loop as u8).count();
    assert_eq!(loop_count, 2);
}

#[test]
fn test_break_in_for_without_condition() {
    let c = codes("for (;;) { break; }");
    // Infinite for with break — should have Jump but no JumpIfFalse.
    assert!(c.iter().any(|&b| b == ByteCode::Jump as u8));
    assert!(!c.iter().any(|&b| b == ByteCode::JumpIfFalse as u8));
}

// ------------------------------------------------------------------------
//  For-in — compiler tests
// ------------------------------------------------------------------------

#[test]
fn test_for_in_emits_forin_iter_and_next() {
    let c = codes("for x in [1] {}");
    assert!(c.iter().any(|&b| b == ByteCode::ForInIter as u8));
    assert!(c.iter().any(|&b| b == ByteCode::ForInNext as u8));
}

#[test]
fn test_for_in_missing_in_is_error() {
    let mut obj_heap = ObjectHeap::new();
    match compile("for x [1] {}", &mut obj_heap) {
        Err(CompileError::Parse(errors)) => {
            assert!(errors.iter().any(|e| matches!(e.kind, ParseErrorKind::ExpectedToken(_))));
        }
        _ => panic!("expected parse error for missing 'in'"),
    }
}

#[test]
fn test_for_in_string_literal() {
    let (chunk, _) = compile_with_heap("for c in \"ab\" {}");
    // Should compile successfully — verify ForInIter and ForInNext present.
    let codes = &chunk.codes;
    assert!(codes.iter().any(|&b| b == ByteCode::ForInIter as u8));
    assert!(codes.iter().any(|&b| b == ByteCode::ForInNext as u8));
}

// ------------------------------------------------------------------------
//  Lambda (anonymous function) expressions
// ------------------------------------------------------------------------

#[test]
fn test_lambda_no_params() {
    let (chunk, heap) = compile_with_heap("fun() { return 42; };");
    let script_insts = instructions(&chunk, &heap);
    // Closure + Pop + Nil + Return
    assert!(script_insts.iter().any(|i| matches!(i, Instruction::Closure { .. })));
    let fn_handle = script_insts
        .iter()
        .find_map(|i| if let Instruction::Closure { function, .. } = i { Some(*function) } else { None })
        .expect("should have Closure");
    let fn_obj = heap.get(fn_handle).as_function().unwrap();
    assert_eq!(fn_obj.arity, 0);
    assert_eq!(fn_obj.name.as_str(), "<lambda>");
}

#[test]
fn test_lambda_with_params() {
    let (chunk, heap) = compile_with_heap("fun(x, y) { return x + y; };");
    let script_insts = instructions(&chunk, &heap);
    let fn_handle = script_insts
        .iter()
        .find_map(|i| if let Instruction::Closure { function, .. } = i { Some(*function) } else { None })
        .expect("should have Closure");
    let fn_obj = heap.get(fn_handle).as_function().unwrap();
    assert_eq!(fn_obj.arity, 2);
    assert_eq!(fn_obj.required_arity, 2);
}

#[test]
fn test_lambda_assigned_to_var() {
    let (chunk, heap) = compile_with_heap("var f = fun(x) { return x + 1; };");
    let script_insts = instructions(&chunk, &heap);
    // Should have Closure + DefineGlobal (or SetLocal if in scope)
    assert!(script_insts.iter().any(|i| matches!(i, Instruction::Closure { .. })));
    // Verify the lambda has 1 param
    let fn_handle = script_insts
        .iter()
        .find_map(|i| if let Instruction::Closure { function, .. } = i { Some(*function) } else { None })
        .expect("should have Closure");
    let fn_obj = heap.get(fn_handle).as_function().unwrap();
    assert_eq!(fn_obj.arity, 1);
}

#[test]
fn test_lambda_passed_as_argument() {
    let (chunk, heap) = compile_with_heap("call(fun(x) { return x; });");
    let script_insts = instructions(&chunk, &heap);
    // Should have GetGlobal("call"), Closure, Call
    assert!(script_insts.iter().any(|i| matches!(i, Instruction::GetGlobal(_))));
    assert!(script_insts.iter().any(|i| matches!(i, Instruction::Closure { .. })));
    assert!(script_insts.iter().any(|i| matches!(i, Instruction::Call(_))));
}

#[test]
fn test_lambda_captures_upvalue() {
    let (chunk, heap) = compile_with_heap("fun outer() { var x = 1; var f = fun() { return x; }; }");
    let script_insts = instructions(&chunk, &heap);
    // Get the outer function
    let outer_fn_handle = script_insts
        .iter()
        .find_map(|i| if let Instruction::Closure { function, .. } = i { Some(*function) } else { None })
        .expect("should have outer Closure");
    let outer_fn = heap.get(outer_fn_handle).as_function().unwrap();
    let outer_insts = instructions(&outer_fn.chunk, &heap);
    // The lambda inside outer should capture x as an upvalue
    let (_, upvalues) = outer_insts
        .iter()
        .find_map(|i| if let Instruction::Closure { function, upvalues } = i { Some((*function, upvalues.clone())) } else { None })
        .expect("should have inner lambda Closure");
    assert_eq!(upvalues.len(), 1);
    assert!(upvalues[0].is_local);
    assert_eq!(upvalues[0].index, 1); // slot 1 = x
}

#[test]
fn test_lambda_immediately_called() {
    let (chunk, heap) = compile_with_heap("fun(x) { return x; }(42);");
    let script_insts = instructions(&chunk, &heap);
    // Closure + Constant(42) + Call(1) + Pop + Nil + Return
    assert!(script_insts.iter().any(|i| matches!(i, Instruction::Closure { .. })));
    assert!(script_insts.iter().any(|i| matches!(i, Instruction::Constant(_))));
    assert!(script_insts.iter().any(|i| matches!(i, Instruction::Call(_))));
}

#[test]
fn test_lambda_in_block_scope() {
    let (chunk, heap) = compile_with_heap("{ var f = fun(x) { return x * 2; }; }");
    let script_insts = instructions(&chunk, &heap);
    // Closure + SetLocal + Pop (end of scope) + Nil + Return
    assert!(script_insts.iter().any(|i| matches!(i, Instruction::Closure { .. })));
}

#[test]
fn test_lambda_named_function_unaffected() {
    // Verify that named function declarations still work correctly.
    let (chunk, heap) = compile_with_heap("fun add(a, b) { return a + b; }");
    let script_insts = instructions(&chunk, &heap);
    assert!(script_insts.iter().any(|i| matches!(i, Instruction::Closure { .. })));
    assert!(script_insts.iter().any(|i| matches!(i, Instruction::DefineGlobal(_))));
    // The function should have the correct name
    let fn_handle = script_insts
        .iter()
        .find_map(|i| if let Instruction::Closure { function, .. } = i { Some(*function) } else { None })
        .expect("should have Closure");
    let fn_obj = heap.get(fn_handle).as_function().unwrap();
    assert_eq!(fn_obj.name.as_str(), "add");
}

#[test]
fn test_lambda_nested_in_function() {
    let (chunk, heap) = compile_with_heap(
        "fun outer() { var f = fun(x, y) { return x + y; }; return f(1, 2); }",
    );
    let script_insts = instructions(&chunk, &heap);
    let outer_fn_handle = script_insts
        .iter()
        .find_map(|i| if let Instruction::Closure { function, .. } = i { Some(*function) } else { None })
        .expect("should have outer Closure");
    let outer_fn = heap.get(outer_fn_handle).as_function().unwrap();
    let outer_insts = instructions(&outer_fn.chunk, &heap);
    // The lambda inside should have 2 params
    let (inner_fn_handle, _) = outer_insts
        .iter()
        .find_map(|i| if let Instruction::Closure { function, upvalues } = i { Some((*function, upvalues.clone())) } else { None })
        .expect("should have inner lambda");
    let inner_fn = heap.get(inner_fn_handle).as_function().unwrap();
    assert_eq!(inner_fn.arity, 2);
    assert_eq!(inner_fn.name.as_str(), "<lambda>");
}

#[test]
fn test_lambda_expression_statement() {
    // A lambda as a standalone expression statement should compile
    // (closure is pushed then popped).
    let (chunk, heap) = compile_with_heap("fun(x) { return x; };");
    let script_insts = instructions(&chunk, &heap);
    assert!(script_insts.iter().any(|i| matches!(i, Instruction::Closure { .. })));
    assert!(script_insts.iter().any(|i| matches!(i, Instruction::Pop)));
}

// ------------------------------------------------------------------------
//  Two closures capturing the same upvalues — compile-time check
// ------------------------------------------------------------------------

/// Verify that at compile time, two closures in the same function that
/// capture the same upvalues produce correct upvalue descriptors.
#[test]
fn test_two_closures_same_upvalues_compile() {
    let (chunk, heap) = compile_with_heap(
        "fun outer(a, b) { fun inner1() { return a + b; } fun inner2() { return a * b; } }",
    );
    let script_insts = instructions(&chunk, &heap);

    // Find outer function
    let outer_fn_handle = script_insts
        .iter()
        .find_map(|i| if let Instruction::Closure { function, .. } = i { Some(*function) } else { None })
        .expect("should have outer Closure");
    let outer_fn = heap.get(outer_fn_handle).as_function().unwrap();
    let outer_insts = instructions(&outer_fn.chunk, &heap);

    // Both inner1 and inner2 should each capture 2 upvalues (a and b).
    let closures: Vec<_> = outer_insts
        .iter()
        .filter_map(|i| {
            if let Instruction::Closure { function, upvalues } = i {
                Some((*function, upvalues.clone()))
            } else {
                None
            }
        })
        .collect();

    assert_eq!(closures.len(), 2, "expected two inner closures (inner1 and inner2)");

    for (_fn_handle, upvalues) in &closures {
        assert_eq!(upvalues.len(), 2, "each closure should capture both a and b");
        assert!(upvalues[0].is_local);
        assert_eq!(upvalues[0].index, 1); // a
        assert!(upvalues[1].is_local);
        assert_eq!(upvalues[1].index, 2); // b
    }
}

/// Same as above but with lambdas instead of named functions.
#[test]
fn test_two_lambdas_same_upvalues_compile() {
    let (chunk, heap) = compile_with_heap(
        "fun outer(a, b) { var f1 = fun() { return a + b; }; var f2 = fun() { return a * b; }; }",
    );
    let script_insts = instructions(&chunk, &heap);

    let outer_fn_handle = script_insts
        .iter()
        .find_map(|i| if let Instruction::Closure { function, .. } = i { Some(*function) } else { None })
        .expect("should have outer Closure");
    let outer_fn = heap.get(outer_fn_handle).as_function().unwrap();
    let outer_insts = instructions(&outer_fn.chunk, &heap);

    let closures: Vec<_> = outer_insts
        .iter()
        .filter_map(|i| {
            if let Instruction::Closure { function, upvalues } = i {
                Some((*function, upvalues.clone()))
            } else {
                None
            }
        })
        .collect();

    assert_eq!(closures.len(), 2, "expected two inner lambda closures");

    for (_fn_handle, upvalues) in &closures {
        assert_eq!(upvalues.len(), 2);
        assert!(upvalues[0].is_local, "first upvalue should be local");
        assert_eq!(upvalues[0].index, 1); // a
        assert!(upvalues[1].is_local, "second upvalue should be local");
        assert_eq!(upvalues[1].index, 2); // b
    }
}
