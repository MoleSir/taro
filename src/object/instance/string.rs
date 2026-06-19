use crate::{
    NativeFunction, NativeData, ObjectHandle, ObjectInstanceData, ToNativeData,
    vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine},
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
        pub fn $name(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
            let lhs_s = vm.get_string_instance(lhs)?.clone();
            if let Ok(rhs_s) = vm.get_string_instance(rhs) {
                return Ok(vm.obj_heap.alloc_bool_instance($op(lhs_s.as_str(), rhs_s.as_str())));
            }
            Err(RuntimeErrorKind::BinaryOpTypeMismatch($op_name, "string", vm.value_type_name(rhs)))
        }
    };
}

impl ObjectString {
    pub fn __add__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_s = vm.get_string_instance(lhs)?.clone();
        if let Ok(rhs_s) = vm.get_string_instance(rhs) {
            let result = format!("{}{}", lhs_s.as_str(), rhs_s.as_str());
            return Ok(vm.obj_heap.alloc_string_instance(result.into()));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("add", "string", vm.value_type_name(rhs)))
    }

    string_cmp_op!(__eq__, |a, b| a == b, "eq");
    string_cmp_op!(__ne__, |a, b| a != b, "ne");
    string_cmp_op!(__gt__, |a, b| a > b, "gt");
    string_cmp_op!(__ge__, |a, b| a >= b, "ge");
    string_cmp_op!(__lt__, |a, b| a < b, "lt");
    string_cmp_op!(__le__, |a, b| a <= b, "le");

    pub fn __not__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        Ok(vm.obj_heap.alloc_bool_instance(s.is_empty()))
    }

    pub fn __str__(_vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        Ok(receiver)
    }

    pub fn __bool__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        Ok(vm.obj_heap.alloc_bool_instance(!s.is_empty()))
    }

    pub fn __hash__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        use std::hash::Hasher;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(s.as_str(), &mut h);
        Ok(vm.obj_heap.alloc_integer_instance(h.finish() as i64))
    }

    pub fn __int__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        let val: i64 = s.as_str().parse().map_err(|_| {
            RuntimeErrorKind::BadIntResult("string")
        })?;
        Ok(vm.obj_heap.alloc_integer_instance(val))
    }

    pub fn __float__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        let val: f64 = s.as_str().parse().map_err(|_| {
            RuntimeErrorKind::BadFloatResult("string")
        })?;
        Ok(vm.obj_heap.alloc_float_instance(val))
    }

    pub fn __len__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        Ok(vm.obj_heap.alloc_integer_instance(s.len() as i64))
    }

    pub fn len(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        Self::__len__(vm, receiver)
    }

    /// `string.upper()` — convert all characters to uppercase.
    pub fn upper(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        Ok(vm.obj_heap.alloc_string_instance(s.to_uppercase().into()))
    }

    /// `string.lower()` — convert all characters to lowercase.
    pub fn lower(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        Ok(vm.obj_heap.alloc_string_instance(s.to_lowercase().into()))
    }

    /// `string.trim()` — remove leading and trailing whitespace.
    pub fn trim(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        Ok(vm.obj_heap.alloc_string_instance(s.trim().to_string().into()))
    }

    /// `string.trim_start()` — remove leading whitespace.
    pub fn trim_start(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        Ok(vm.obj_heap.alloc_string_instance(s.trim_start().to_string().into()))
    }

    /// `string.trim_end()` — remove trailing whitespace.
    pub fn trim_end(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        Ok(vm.obj_heap.alloc_string_instance(s.trim_end().to_string().into()))
    }

    /// `string.starts_with(prefix)` — return true if the string starts with `prefix`.
    pub fn starts_with(vm: &mut VirtualMachine, receiver: ObjectHandle, prefix_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        let prefix = vm.get_string_instance(prefix_handle)?;
        Ok(vm.obj_heap.alloc_bool_instance(s.starts_with(prefix.as_str())))
    }

    /// `string.ends_with(suffix)` — return true if the string ends with `suffix`.
    pub fn ends_with(vm: &mut VirtualMachine, receiver: ObjectHandle, suffix_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        let suffix = vm.get_string_instance(suffix_handle)?;
        Ok(vm.obj_heap.alloc_bool_instance(s.ends_with(suffix.as_str())))
    }

    /// `string.contains(sub)` — return true if the string contains `sub`.
    pub fn contains(vm: &mut VirtualMachine, receiver: ObjectHandle, sub_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        let sub = vm.get_string_instance(sub_handle)?;
        Ok(vm.obj_heap.alloc_bool_instance(s.contains(sub.as_str())))
    }

    /// `string.replace(old, new)` — replace all occurrences of `old` with `new`.
    pub fn replace(vm: &mut VirtualMachine, receiver: ObjectHandle, old_handle: ObjectHandle, new_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        let old = vm.get_string_instance(old_handle)?;
        let new = vm.get_string_instance(new_handle)?;
        Ok(vm.obj_heap.alloc_string_instance(s.replace(old.as_str(), new.as_str()).into()))
    }

    /// `string.split(delim)` — split the string by `delim` and return a list.
    pub fn split(vm: &mut VirtualMachine, receiver: ObjectHandle, delim_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?.clone();
        let delim = vm.get_string_instance(delim_handle)?.clone();
        let parts: Vec<ObjectHandle> = s.split(delim.as_str())
            .map(|part| vm.obj_heap.alloc_string_instance(part.to_string().into()))
            .collect();
        Ok(vm.obj_heap.alloc_list_instance(parts))
    }

    /// `string.substr(start, length)` — extract a substring.
    pub fn substr(vm: &mut VirtualMachine, receiver: ObjectHandle, start_handle: ObjectHandle, length_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?.clone();
        let start = *vm.get_integer_instance(start_handle)?;
        let length = *vm.get_integer_instance(length_handle)?;
        let len = s.len() as i64;

        // Handle negative start (count from end).
        let byte_start = if start < 0 {
            let adjusted = len + start;
            if adjusted < 0 { 0 } else { adjusted }
        } else if start > len {
            len
        } else {
            start
        } as usize;

        let byte_end = if length < 0 {
            // Negative length: clamp to 0 (like JavaScript).
            byte_start
        } else {
            let end = byte_start as i64 + length;
            if end > len { len as usize } else { end as usize }
        };

        let result = &s.as_str()[byte_start..byte_end];
        Ok(vm.obj_heap.alloc_string_instance(result.to_string().into()))
    }

    /// `string.find(sub)` — return the index of the first occurrence of `sub`,
    /// or -1 if not found.
    pub fn find(vm: &mut VirtualMachine, receiver: ObjectHandle, sub_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        let sub = vm.get_string_instance(sub_handle)?;
        match s.find(sub.as_str()) {
            Some(idx) => Ok(vm.obj_heap.alloc_integer_instance(idx as i64)),
            None => Ok(vm.obj_heap.alloc_integer_instance(-1)),
        }
    }

    /// `string.rfind(sub)` — return the index of the last occurrence of `sub`,
    /// or -1 if not found.
    pub fn rfind(vm: &mut VirtualMachine, receiver: ObjectHandle, sub_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        let sub = vm.get_string_instance(sub_handle)?;
        match s.rfind(sub.as_str()) {
            Some(idx) => Ok(vm.obj_heap.alloc_integer_instance(idx as i64)),
            None => Ok(vm.obj_heap.alloc_integer_instance(-1)),
        }
    }

    /// `string.is_empty()` — return true if the string is empty.
    pub fn is_empty(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?;
        Ok(vm.obj_heap.alloc_bool_instance(s.is_empty()))
    }

    /// `string.repeat(n)` — repeat the string `n` times.
    pub fn repeat(vm: &mut VirtualMachine, receiver: ObjectHandle, n_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?.clone();
        let n = *vm.get_integer_instance(n_handle)?;
        if n < 0 {
            return Err(RuntimeErrorKind::IndexOutOfRange(n, 0));
        }
        let result = s.repeat(n as usize);
        Ok(vm.obj_heap.alloc_string_instance(result.into()))
    }

    pub fn __getitem__(vm: &mut VirtualMachine, receiver: ObjectHandle, idx_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.get_string_instance(receiver)?.clone();
        let idx_val = *vm.get_integer_instance(idx_handle)?;
        let len = s.len();
        let idx = if idx_val < 0 { len as i64 + idx_val } else { idx_val };
        if idx < 0 || idx as usize >= len {
            return Err(RuntimeErrorKind::IndexOutOfRange(idx, len));
        }
        let ch = s.as_str()[idx as usize..idx as usize + 1].to_string();
        Ok(vm.obj_heap.alloc_string_instance(ch.into()))
    }

    // ---- iteration protocol ----

    pub fn __iter__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let iter = StringIterator { string_handle: receiver, byte_index: 0 };
        Ok(vm.obj_heap.alloc_instance(
            vm.obj_heap.string_iter_class,
            ObjectInstanceData::Native(NativeData::new(iter)),
        ))
    }

    pub fn iter_next(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
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
fn identity_iter(_vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
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
    heap.register_native_method(sc, "upper",        NativeFunction::a1(ObjectString::upper));
    heap.register_native_method(sc, "lower",        NativeFunction::a1(ObjectString::lower));
    heap.register_native_method(sc, "trim",         NativeFunction::a1(ObjectString::trim));
    heap.register_native_method(sc, "trim_start",   NativeFunction::a1(ObjectString::trim_start));
    heap.register_native_method(sc, "trim_end",     NativeFunction::a1(ObjectString::trim_end));
    heap.register_native_method(sc, "starts_with",  NativeFunction::a2(ObjectString::starts_with));
    heap.register_native_method(sc, "ends_with",    NativeFunction::a2(ObjectString::ends_with));
    heap.register_native_method(sc, "contains",     NativeFunction::a2(ObjectString::contains));
    heap.register_native_method(sc, "replace",      NativeFunction::a3(ObjectString::replace));
    heap.register_native_method(sc, "split",        NativeFunction::a2(ObjectString::split));
    heap.register_native_method(sc, "substr",       NativeFunction::a3(ObjectString::substr));
    heap.register_native_method(sc, "find",         NativeFunction::a2(ObjectString::find));
    heap.register_native_method(sc, "rfind",        NativeFunction::a2(ObjectString::rfind));
    heap.register_native_method(sc, "is_empty",     NativeFunction::a1(ObjectString::is_empty));
    heap.register_native_method(sc, "repeat",       NativeFunction::a2(ObjectString::repeat));

    // Aliases
    heap.register_native_method(sc, "to_uppercase",   NativeFunction::a1(ObjectString::upper));
    heap.register_native_method(sc, "to_lowercase",   NativeFunction::a1(ObjectString::lower));
    heap.register_native_method(sc, "strip",          NativeFunction::a1(ObjectString::trim));
    heap.register_native_method(sc, "lstrip",         NativeFunction::a1(ObjectString::trim_start));
    heap.register_native_method(sc, "rstrip",         NativeFunction::a1(ObjectString::trim_end));
    heap.register_native_method(sc, "index_of",       NativeFunction::a2(ObjectString::find));
    heap.register_native_method(sc, "last_index_of",  NativeFunction::a2(ObjectString::rfind));

    heap.register_native_method(sc, "__iter__",     NativeFunction::a1(ObjectString::__iter__));

    let sic = heap.string_iter_class;
    heap.register_native_method(sic, "__iter__", NativeFunction::a1(identity_iter));
    heap.register_native_method(sic, "__next__", NativeFunction::a1(ObjectString::iter_next));
}
