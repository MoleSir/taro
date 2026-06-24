use std::any::Any;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::{ObjectHeap, ObjectInstanceData};
use crate::{
    NativeFunction, ObjectHandle, impl_object_instance_data, native_a1,
    vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine},
};

// ========================================================================== //
//  ObjectBytesIterator (iterator state)
// ========================================================================== //

/// Iterator state for bytes — yields each byte as an integer.
pub struct ObjectBytesIterator {
    pub bytes_handle: ObjectHandle,
    pub index: usize,
}

impl ObjectInstanceData for ObjectBytesIterator {
    fn mark_references(&self, heap: &mut ObjectHeap) {
        heap.mark_object(self.bytes_handle);
    }
    fn type_name(&self) -> &'static str {
        "bytes iterator"
    }
    fn as_any_ref(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ========================================================================== //
//  ObjectBytes
// ========================================================================== //

/// Represents the `Bytes` built-in type.
pub struct ObjectBytes {
    pub data: Vec<u8>,
}

impl_object_instance_data!(ObjectBytes, "bytes");

// Free function that returns the receiver unchanged — used for iterator
// `__iter__` implementations that just return `self`.
fn identity_iter(_vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    Ok(receiver)
}

impl ObjectBytes {
    // ---- construction helpers ----

    /// Build a `Bytes` instance from a string by encoding to UTF-8.
    pub fn from_string(vm: &mut VirtualMachine, s: &str) -> RuntimeResult<ObjectHandle> {
        Ok(vm.obj_heap.alloc_bytes_instance(s.as_bytes().to_vec()))
    }

    /// Build a `Bytes` instance from a list of integers (0-255).
    pub fn from_list(vm: &mut VirtualMachine, list_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let items = vm.obj_heap.expect_list(list_handle)?.clone();
        let mut data = Vec::with_capacity(items.len());
        for &item in &items {
            let b = *vm.obj_heap.expect_integer(item)?;
            if b < 0 || b > 255 {
                return Err(RuntimeErrorKind::UnexpectedType("byte value in range 0-255", vm.obj_heap.type_of(item)));
            }
            data.push(b as u8);
        }
        Ok(vm.obj_heap.alloc_bytes_instance(data))
    }

    // ---- magic methods ----

    native_a1!(__not__, data: &Vec<u8>, { data.is_empty() });

    pub fn __str__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let data = vm.obj_heap.expect_bytes(receiver)?.clone();
        let mut result = String::from("b\"");
        for &byte in &data {
            match byte {
                // Printable ASCII (space through ~) except backslash and double-quote.
                b' '..=b'!' | b'#'..=b'[' | b']'..=b'~' => {
                    result.push(byte as char);
                }
                b'"' => result.push_str("\\\""),
                b'\\' => result.push_str("\\\\"),
                b'\n' => result.push_str("\\n"),
                b'\r' => result.push_str("\\r"),
                b'\t' => result.push_str("\\t"),
                b'\0' => result.push_str("\\0"),
                _ => {
                    result.push_str(&format!("\\x{:02x}", byte));
                }
            }
        }
        result.push('"');
        Ok(vm.obj_heap.alloc_string_instance(result.into()))
    }

    native_a1!(__bool__, data: &Vec<u8>, { !data.is_empty() });

    native_a1!(__len__, data: &Vec<u8>, { data.len() as i64 });

    /// Alias for `__len__`.
    pub fn len(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        Self::__len__(vm, receiver)
    }

    pub fn __getitem__(vm: &mut VirtualMachine, receiver: ObjectHandle, idx_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let data = vm.obj_heap.expect_bytes(receiver)?.clone();
        let idx_val = *vm.obj_heap.expect_integer(idx_handle)?;
        let len = data.len();
        let idx = if idx_val < 0 { len as i64 + idx_val } else { idx_val };
        if idx < 0 || idx as usize >= len {
            return Err(RuntimeErrorKind::IndexOutOfRange(idx, len));
        }
        Ok(vm.obj_heap.alloc_integer_instance(data[idx as usize] as i64))
    }

    pub fn __eq__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if let Some(rhs_data) = vm.obj_heap.get_bytes_instance(rhs) {
            let lhs_data = vm.obj_heap.expect_bytes(lhs)?;
            Ok(vm.obj_heap.alloc_bool_instance(lhs_data == rhs_data))
        } else {
            Ok(vm.obj_heap.alloc_bool_instance(false))
        }
    }

    pub fn __ne__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let eq = Self::__eq__(vm, lhs, rhs)?;
        let b = *vm.obj_heap.expect_bool(eq)?;
        Ok(vm.obj_heap.alloc_bool_instance(!b))
    }

    pub fn __add__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let mut lhs_data = vm.obj_heap.expect_bytes(lhs)?.clone();
        if let Some(rhs_data) = vm.obj_heap.get_bytes_instance(rhs) {
            lhs_data.extend_from_slice(rhs_data);
            return Ok(vm.obj_heap.alloc_bytes_instance(lhs_data));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("add", "bytes", vm.obj_heap.type_of(rhs)))
    }

    pub fn __contains__(vm: &mut VirtualMachine, receiver: ObjectHandle, item: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let byte_val = *vm.obj_heap.expect_integer(item)?;
        if byte_val < 0 || byte_val > 255 {
            return Ok(vm.obj_heap.alloc_bool_instance(false));
        }
        let data = vm.obj_heap.expect_bytes(receiver)?;
        Ok(vm.obj_heap.alloc_bool_instance(data.contains(&(byte_val as u8))))
    }

    pub fn __hash__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let data = vm.obj_heap.expect_bytes(receiver)?;
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        Ok(vm.obj_heap.alloc_integer_instance(hasher.finish() as i64))
    }

    // ---- regular methods ----

    /// `bytes.decode("utf-8")` — decode bytes to a string.
    pub fn decode(vm: &mut VirtualMachine, receiver: ObjectHandle, encoding: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let enc = vm.obj_heap.expect_string(encoding)?.as_str().to_ascii_lowercase();
        if enc != "utf-8" && enc != "utf8" {
            return Err(RuntimeErrorKind::TypeMismatch { expected: "encoding 'utf-8'", found: "other encoding" });
        }
        let data = vm.obj_heap.expect_bytes(receiver)?;
        let s = String::from_utf8(data.clone())
            .map_err(|_| RuntimeErrorKind::TypeMismatch { expected: "valid utf-8 bytes", found: "invalid utf-8 bytes" })?;
        Ok(vm.obj_heap.alloc_string_instance(s.into()))
    }

    /// `bytes.hex()` — return a hex string representation.
    pub fn hex(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let data = vm.obj_heap.expect_bytes(receiver)?;
        let hex_str: String = data.iter().map(|b| format!("{:02x}", b)).collect();
        Ok(vm.obj_heap.alloc_string_instance(hex_str.into()))
    }

    /// `bytes.to_list()` — convert bytes to a list of integers.
    pub fn to_list(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let data = vm.obj_heap.expect_bytes(receiver)?.clone();
        let items: Vec<ObjectHandle> = data.iter().map(|&b| vm.obj_heap.alloc_integer_instance(b as i64)).collect();
        Ok(vm.obj_heap.alloc_list_instance(items))
    }

    // ---- iteration protocol ----

    pub fn __iter__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let iter = ObjectBytesIterator { bytes_handle: receiver, index: 0 };
        Ok(vm.obj_heap.alloc_instance(vm.obj_heap.bytes_iter_class, iter))
    }

    pub fn iter_next(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let (bytes_handle, idx) = {
            let iter = vm.obj_heap.expect_bytes_iter(receiver)?;
            (iter.bytes_handle, iter.index)
        };
        let data = vm.obj_heap.expect_bytes(bytes_handle)?;
        if idx >= data.len() {
            return Ok(ObjectHandle::ITER_END);
        }
        let value = data[idx];
        // NLL drops `data` reference here; mut borrow below is now exclusive.
        let found = vm.obj_heap.type_of(receiver);
        let iter = vm
            .obj_heap
            .get_bytes_iter_mut(receiver)
            .ok_or_else(|| RuntimeErrorKind::TypeMismatch { expected: "bytes iterator", found })?;
        iter.index = idx + 1;
        Ok(vm.obj_heap.alloc_integer_instance(value as i64))
    }
}

// ========================================================================== //
//  Registration
// ========================================================================== //

/// Register all `Bytes` magic methods directly on the class during heap init.
pub fn register_bytes_builtins(heap: &mut ObjectHeap) {
    let bc = heap.bytes_class;
    heap.register_native_method(bc, "__str__", NativeFunction::a1(ObjectBytes::__str__));
    heap.register_native_method(bc, "__bool__", NativeFunction::a1(ObjectBytes::__bool__));
    heap.register_native_method(bc, "__not__", NativeFunction::a1(ObjectBytes::__not__));
    heap.register_native_method(bc, "__len__", NativeFunction::a1(ObjectBytes::__len__));
    heap.register_native_method(bc, "__getitem__", NativeFunction::a2(ObjectBytes::__getitem__));
    heap.register_native_method(bc, "__eq__", NativeFunction::a2(ObjectBytes::__eq__));
    heap.register_native_method(bc, "__ne__", NativeFunction::a2(ObjectBytes::__ne__));
    heap.register_native_method(bc, "__add__", NativeFunction::a2(ObjectBytes::__add__));
    heap.register_native_method(bc, "__contains__", NativeFunction::a2(ObjectBytes::__contains__));
    heap.register_native_method(bc, "__hash__", NativeFunction::a1(ObjectBytes::__hash__));
    heap.register_native_method(bc, "__iter__", NativeFunction::a1(ObjectBytes::__iter__));
    heap.register_native_method(bc, "len", NativeFunction::a1(ObjectBytes::len));
    heap.register_native_method(bc, "decode", NativeFunction::a2(ObjectBytes::decode));
    heap.register_native_method(bc, "hex", NativeFunction::a1(ObjectBytes::hex));
    heap.register_native_method(bc, "to_list", NativeFunction::a1(ObjectBytes::to_list));

    let bic = heap.bytes_iter_class;
    heap.register_native_method(bic, "__iter__", NativeFunction::a1(identity_iter));
    heap.register_native_method(bic, "__next__", NativeFunction::a1(ObjectBytes::iter_next));
}

// ========================================================================== //
//  Tests
// ========================================================================== //

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: get string from handle.
    fn get_str(vm: &VirtualMachine, handle: ObjectHandle) -> String {
        vm.obj_heap.get_string_instance(handle).unwrap().as_str().to_string()
    }

    /// Helper: get int from handle.
    fn get_int(vm: &VirtualMachine, handle: ObjectHandle) -> i64 {
        *vm.obj_heap.get_integer_instance(handle).unwrap()
    }

    /// Helper: get bool from handle.
    fn get_bool(vm: &VirtualMachine, handle: ObjectHandle) -> bool {
        *vm.obj_heap.get_bool_instance(handle).unwrap()
    }

    /// Helper: get Vec<u8> from bytes handle.
    fn get_bytes_data(vm: &VirtualMachine, handle: ObjectHandle) -> Vec<u8> {
        vm.obj_heap.get_bytes_instance(handle).unwrap().clone()
    }

    // ------------------------------------------------------------------------
    //  Construction
    // ------------------------------------------------------------------------

    #[test]
    fn test_bytes_from_string() {
        let mut vm = VirtualMachine::new();
        let handle = ObjectBytes::from_string(&mut vm, "hello").unwrap();
        assert_eq!(get_bytes_data(&vm, handle), b"hello".to_vec());
    }

    #[test]
    fn test_bytes_from_list_of_ints() {
        let mut vm = VirtualMachine::new();
        let h104 = vm.obj_heap.alloc_integer_instance(104);
        let h101 = vm.obj_heap.alloc_integer_instance(101);
        let h108 = vm.obj_heap.alloc_integer_instance(108);
        let h111 = vm.obj_heap.alloc_integer_instance(111);
        let list_handle = vm.obj_heap.alloc_list_instance(vec![h104, h101, h108, h108, h111]);
        let handle = ObjectBytes::from_list(&mut vm, list_handle).unwrap();
        assert_eq!(get_bytes_data(&vm, handle), b"hello".to_vec());
    }

    #[test]
    fn test_bytes_from_list_out_of_range() {
        let mut vm = VirtualMachine::new();
        let h256 = vm.obj_heap.alloc_integer_instance(256);
        let list_handle = vm.obj_heap.alloc_list_instance(vec![h256]);
        let result = ObjectBytes::from_list(&mut vm, list_handle);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------------
    //  __str__
    // ------------------------------------------------------------------------

    #[test]
    fn test_str_printable() {
        let mut vm = VirtualMachine::new();
        let handle = ObjectBytes::from_string(&mut vm, "hello").unwrap();
        let str_handle = ObjectBytes::__str__(&mut vm, handle).unwrap();
        assert_eq!(get_str(&vm, str_handle), "b\"hello\"");
    }

    #[test]
    fn test_str_empty() {
        let mut vm = VirtualMachine::new();
        let handle = vm.obj_heap.alloc_bytes_instance(vec![]);
        let str_handle = ObjectBytes::__str__(&mut vm, handle).unwrap();
        assert_eq!(get_str(&vm, str_handle), "b\"\"");
    }

    #[test]
    fn test_str_with_special_chars() {
        let mut vm = VirtualMachine::new();
        let data = vec![0, 10, 13, 9, 92, 34, 255];
        let handle = vm.obj_heap.alloc_bytes_instance(data);
        let str_handle = ObjectBytes::__str__(&mut vm, handle).unwrap();
        let s = get_str(&vm, str_handle);
        assert!(s.contains("\\0"), "expected \\0 in: {s}");
        assert!(s.contains("\\n"), "expected \\n in: {s}");
        assert!(s.contains("\\r"), "expected \\r in: {s}");
        assert!(s.contains("\\t"), "expected \\t in: {s}");
        assert!(s.contains("\\\\"), "expected \\\\ in: {s}");
        assert!(s.contains("\\\""), "expected \\\" in: {s}");
        assert!(s.contains("\\xff"), "expected \\xff in: {s}");
    }

    // ------------------------------------------------------------------------
    //  __bool__ / __not__
    // ------------------------------------------------------------------------

    #[test]
    fn test_bool_nonempty() {
        let mut vm = VirtualMachine::new();
        let handle = ObjectBytes::from_string(&mut vm, "x").unwrap();
        let result = ObjectBytes::__bool__(&mut vm, handle).unwrap();
        assert!(get_bool(&vm, result));
    }

    #[test]
    fn test_bool_empty() {
        let mut vm = VirtualMachine::new();
        let handle = vm.obj_heap.alloc_bytes_instance(vec![]);
        let result = ObjectBytes::__bool__(&mut vm, handle).unwrap();
        assert!(!get_bool(&vm, result));
    }

    // ------------------------------------------------------------------------
    //  __len__
    // ------------------------------------------------------------------------

    #[test]
    fn test_len() {
        let mut vm = VirtualMachine::new();
        let handle = ObjectBytes::from_string(&mut vm, "abc").unwrap();
        let result = ObjectBytes::__len__(&mut vm, handle).unwrap();
        assert_eq!(get_int(&vm, result), 3);
    }

    // ------------------------------------------------------------------------
    //  __getitem__
    // ------------------------------------------------------------------------

    #[test]
    fn test_index_positive() {
        let mut vm = VirtualMachine::new();
        let handle = ObjectBytes::from_string(&mut vm, "abc").unwrap();
        let idx = vm.obj_heap.alloc_integer_instance(1);
        let result = ObjectBytes::__getitem__(&mut vm, handle, idx).unwrap();
        assert_eq!(get_int(&vm, result), b'b' as i64);
    }

    #[test]
    fn test_index_negative() {
        let mut vm = VirtualMachine::new();
        let handle = ObjectBytes::from_string(&mut vm, "abc").unwrap();
        let idx = vm.obj_heap.alloc_integer_instance(-1);
        let result = ObjectBytes::__getitem__(&mut vm, handle, idx).unwrap();
        assert_eq!(get_int(&vm, result), b'c' as i64);
    }

    #[test]
    fn test_index_out_of_bounds() {
        let mut vm = VirtualMachine::new();
        let handle = ObjectBytes::from_string(&mut vm, "abc").unwrap();
        let idx = vm.obj_heap.alloc_integer_instance(10);
        let result = ObjectBytes::__getitem__(&mut vm, handle, idx);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------------
    //  __eq__ / __ne__
    // ------------------------------------------------------------------------

    #[test]
    fn test_eq() {
        let mut vm = VirtualMachine::new();
        let a = ObjectBytes::from_string(&mut vm, "hello").unwrap();
        let b = ObjectBytes::from_string(&mut vm, "hello").unwrap();
        let c = ObjectBytes::from_string(&mut vm, "world").unwrap();
        let r1 = ObjectBytes::__eq__(&mut vm, a, b).unwrap();
        assert!(get_bool(&vm, r1));
        let r2 = ObjectBytes::__eq__(&mut vm, a, c).unwrap();
        assert!(!get_bool(&vm, r2));
    }

    #[test]
    fn test_eq_different_type() {
        let mut vm = VirtualMachine::new();
        let a = ObjectBytes::from_string(&mut vm, "x").unwrap();
        let s = vm.obj_heap.alloc_string_instance("x".into());
        let r = ObjectBytes::__eq__(&mut vm, a, s).unwrap();
        assert!(!get_bool(&vm, r));
    }

    // ------------------------------------------------------------------------
    //  __add__
    // ------------------------------------------------------------------------

    #[test]
    fn test_add() {
        let mut vm = VirtualMachine::new();
        let a = ObjectBytes::from_string(&mut vm, "ab").unwrap();
        let b = ObjectBytes::from_string(&mut vm, "cd").unwrap();
        let result = ObjectBytes::__add__(&mut vm, a, b).unwrap();
        assert_eq!(get_bytes_data(&vm, result), b"abcd".to_vec());
    }

    #[test]
    fn test_add_wrong_type() {
        let mut vm = VirtualMachine::new();
        let a = ObjectBytes::from_string(&mut vm, "ab").unwrap();
        let s = vm.obj_heap.alloc_string_instance("cd".into());
        let result = ObjectBytes::__add__(&mut vm, a, s);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------------
    //  __contains__
    // ------------------------------------------------------------------------

    #[test]
    fn test_contains_true() {
        let mut vm = VirtualMachine::new();
        let handle = ObjectBytes::from_string(&mut vm, "hello").unwrap();
        let item = vm.obj_heap.alloc_integer_instance(b'e' as i64);
        let r = ObjectBytes::__contains__(&mut vm, handle, item).unwrap();
        assert!(get_bool(&vm, r));
    }

    #[test]
    fn test_contains_false() {
        let mut vm = VirtualMachine::new();
        let handle = ObjectBytes::from_string(&mut vm, "hello").unwrap();
        let item = vm.obj_heap.alloc_integer_instance(b'z' as i64);
        let r = ObjectBytes::__contains__(&mut vm, handle, item).unwrap();
        assert!(!get_bool(&vm, r));
    }

    // ------------------------------------------------------------------------
    //  __hash__
    // ------------------------------------------------------------------------

    #[test]
    fn test_hash_same_data_same_hash() {
        let mut vm = VirtualMachine::new();
        let a = ObjectBytes::from_string(&mut vm, "hello").unwrap();
        let b = ObjectBytes::from_string(&mut vm, "hello").unwrap();
        let ra = ObjectBytes::__hash__(&mut vm, a).unwrap();
        let rb = ObjectBytes::__hash__(&mut vm, b).unwrap();
        assert_eq!(get_int(&vm, ra), get_int(&vm, rb));
    }

    // ------------------------------------------------------------------------
    //  decode
    // ------------------------------------------------------------------------

    #[test]
    fn test_decode_utf8() {
        let mut vm = VirtualMachine::new();
        let handle = ObjectBytes::from_string(&mut vm, "café").unwrap();
        let enc = vm.obj_heap.alloc_string_instance("utf-8".into());
        let result = ObjectBytes::decode(&mut vm, handle, enc).unwrap();
        assert_eq!(get_str(&vm, result), "café");
    }

    #[test]
    fn test_decode_invalid_utf8() {
        let mut vm = VirtualMachine::new();
        let handle = vm.obj_heap.alloc_bytes_instance(vec![0xff, 0xfe, 0xfd]);
        let enc = vm.obj_heap.alloc_string_instance("utf-8".into());
        let result = ObjectBytes::decode(&mut vm, handle, enc);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------------
    //  hex
    // ------------------------------------------------------------------------

    #[test]
    fn test_hex() {
        let mut vm = VirtualMachine::new();
        let handle = vm.obj_heap.alloc_bytes_instance(vec![0xde, 0xad, 0xbe, 0xef]);
        let result = ObjectBytes::hex(&mut vm, handle).unwrap();
        assert_eq!(get_str(&vm, result), "deadbeef");
    }
}
