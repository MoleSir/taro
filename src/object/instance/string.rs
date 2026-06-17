use crate::{
    NativeFunction, NativeData, ObjectHandle, ObjectInstanceData, ToNativeData,
    vm::{ExecuteError, ExecuteResult, VirtualMachine},
};
use super::ObjectHeap;

// ========================================================================== //
//  StringIterator (native state)
// ========================================================================== //

/// Native state for a string iterator (iterates Unicode characters).
struct StringIterator {
    string_handle: ObjectHandle,
    byte_index: usize,
}

impl ToNativeData for StringIterator {
    fn mark_inner_object(&self, heap: &mut ObjectHeap) {
        heap.mark_object(self.string_handle);
    }
}

// ========================================================================== //
//  ObjectString
// ========================================================================== //

/// Represents the `String` built-in type.
pub struct ObjectString;

macro_rules! string_cmp_op {
    ($name:ident, $op:expr, $op_name:literal) => {
        pub fn $name(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
            let lhs_s = vm.get_string_instance(lhs)?.clone();
            if let Ok(rhs_s) = vm.get_string_instance(rhs) {
                return Ok(vm.obj_heap.alloc_bool_instance($op(lhs_s.as_str(), rhs_s.as_str())));
            }
            Err(ExecuteError::BinaryOpTypeMismatch($op_name, "string", vm.value_type_name(rhs)))
        }
    };
}

impl ObjectString {
    pub fn __add__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let lhs_s = vm.get_string_instance(lhs)?.clone();
        if let Ok(rhs_s) = vm.get_string_instance(rhs) {
            let result = format!("{}{}", lhs_s.as_str(), rhs_s.as_str());
            return Ok(vm.obj_heap.alloc_string_instance(result.into()));
        }
        Err(ExecuteError::BinaryOpTypeMismatch("add", "string", vm.value_type_name(rhs)))
    }

    string_cmp_op!(__eq__, |a, b| a == b, "eq");
    string_cmp_op!(__ne__, |a, b| a != b, "ne");
    string_cmp_op!(__gt__, |a, b| a > b, "gt");
    string_cmp_op!(__ge__, |a, b| a >= b, "ge");
    string_cmp_op!(__lt__, |a, b| a < b, "lt");
    string_cmp_op!(__le__, |a, b| a <= b, "le");

    pub fn __not__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        Ok(vm.obj_heap.alloc_bool_instance(s.is_empty()))
    }

    pub fn __str__(_vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        Ok(receiver)
    }

    pub fn __bool__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        Ok(vm.obj_heap.alloc_bool_instance(!s.is_empty()))
    }

    pub fn __hash__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        use std::hash::Hasher;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(s.as_str(), &mut h);
        Ok(vm.obj_heap.alloc_integer_instance(h.finish() as i64))
    }

    pub fn __int__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        let val: i64 = s.as_str().parse().map_err(|_| {
            ExecuteError::BadIntResult("string")
        })?;
        Ok(vm.obj_heap.alloc_integer_instance(val))
    }

    pub fn __float__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        let val: f64 = s.as_str().parse().map_err(|_| {
            ExecuteError::BadFloatResult("string")
        })?;
        Ok(vm.obj_heap.alloc_float_instance(val))
    }

    pub fn __len__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        Ok(vm.obj_heap.alloc_integer_instance(s.len() as i64))
    }

    pub fn len(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        Self::__len__(vm, receiver)
    }

    pub fn __getitem__(vm: &mut VirtualMachine, receiver: ObjectHandle, idx_handle: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?.clone();
        let idx_val = *vm.get_integer_instance(idx_handle)?;
        let len = s.len();
        let idx = if idx_val < 0 { len as i64 + idx_val } else { idx_val };
        if idx < 0 || idx as usize >= len {
            return Err(ExecuteError::IndexOutOfRange(idx, len));
        }
        let ch = s.as_str()[idx as usize..idx as usize + 1].to_string();
        Ok(vm.obj_heap.alloc_string_instance(ch.into()))
    }

    // ---- iteration protocol ----

    pub fn __iter__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let iter = StringIterator { string_handle: receiver, byte_index: 0 };
        Ok(vm.obj_heap.alloc_instance(
            vm.obj_heap.string_iter_class,
            ObjectInstanceData::Native(NativeData::new(iter)),
        ))
    }

    pub fn iter_next(vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let (string_handle, byte_index) = {
            let iter = vm.get_native::<StringIterator>(receiver)?;
            (iter.string_handle, iter.byte_index)
        };
        // Borrow the string immutably to extract the next character, then
        // drop the borrow before mutably updating byte_index in the iterator.
        let (char_str, char_len) = {
            let s = vm.get_string_instance(string_handle)?;
            let remaining = &s.as_str()[byte_index..];
            if let Some(ch) = remaining.chars().next() {
                let cs: String = ch.into();
                let len = cs.len();
                (cs, len)
            } else {
                return Ok(ObjectHandle::ITER_END);
            }
        };
        let iter = vm.get_native_mut::<StringIterator>(receiver)?;
        iter.byte_index += char_len;
        Ok(vm.obj_heap.alloc_string_instance(char_str.into()))
    }
}

// A free function that returns the receiver unchanged — used for iterator
// `__iter__` implementations that just return `self`.
fn identity_iter(_vm: &mut VirtualMachine, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
    Ok(receiver)
}

// ========================================================================== //
//  Registration
// ========================================================================== //

/// Register all `String` magic methods directly on the class during heap init.
pub fn register_string_builtins(heap: &mut ObjectHeap) {
    let sc = heap.string_class;
    heap.register_native_method(sc, "__not__",      NativeFunction::a1(ObjectString::__not__));
    heap.register_native_method(sc, "__add__",      NativeFunction::a2(ObjectString::__add__));
    heap.register_native_method(sc, "__eq__",       NativeFunction::a2(ObjectString::__eq__));
    heap.register_native_method(sc, "__ne__",       NativeFunction::a2(ObjectString::__ne__));
    heap.register_native_method(sc, "__gt__",       NativeFunction::a2(ObjectString::__gt__));
    heap.register_native_method(sc, "__ge__",       NativeFunction::a2(ObjectString::__ge__));
    heap.register_native_method(sc, "__lt__",       NativeFunction::a2(ObjectString::__lt__));
    heap.register_native_method(sc, "__le__",       NativeFunction::a2(ObjectString::__le__));
    heap.register_native_method(sc, "__str__",      NativeFunction::a1(ObjectString::__str__));
    heap.register_native_method(sc, "__bool__",     NativeFunction::a1(ObjectString::__bool__));
    heap.register_native_method(sc, "__hash__",     NativeFunction::a1(ObjectString::__hash__));
    heap.register_native_method(sc, "__int__",      NativeFunction::a1(ObjectString::__int__));
    heap.register_native_method(sc, "__float__",    NativeFunction::a1(ObjectString::__float__));
    heap.register_native_method(sc, "__len__",      NativeFunction::a1(ObjectString::__len__));
    heap.register_native_method(sc, "__getitem__",  NativeFunction::a2(ObjectString::__getitem__));
    heap.register_native_method(sc, "len",          NativeFunction::a1(ObjectString::len));
    heap.register_native_method(sc, "__iter__",     NativeFunction::a1(ObjectString::__iter__));

    let sic = heap.string_iter_class;
    heap.register_native_method(sic, "__iter__", NativeFunction::a1(identity_iter));
    heap.register_native_method(sic, "__next__", NativeFunction::a1(ObjectString::iter_next));
}
