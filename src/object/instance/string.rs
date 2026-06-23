use super::{ObjectHeap, ObjectInstanceData};
use crate::{
    NativeFunction, ObjectHandle, ShrString, impl_object_instance_data, native_a1, native_a2, native_a3,
    vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine},
};

// ========================================================================== //
//  ObjectStringIterator (iterator state)
// ========================================================================== //

/// Iterator state for a string iterator (iterates Unicode characters).
pub struct ObjectStringIterator {
    pub string_handle: ObjectHandle,
    pub byte_index: usize,
}

impl ObjectInstanceData for ObjectStringIterator {
    fn mark_references(&self, heap: &mut ObjectHeap) {
        heap.mark_object(self.string_handle);
    }
    fn type_name(&self) -> &'static str {
        "string iterator"
    }
    fn as_any_ref(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl ObjectStringIterator {
    pub fn new(string_handle: ObjectHandle) -> Self {
        Self { string_handle, byte_index: 0 }
    }
}

// ========================================================================== //
//  Helpers — character ↔ byte index conversion
// ========================================================================== //

/// Return the number of Unicode characters (not bytes) in `s`.
#[inline]
fn char_count(s: &str) -> usize {
    s.chars().count()
}

/// Convert a byte index to a character index.
#[inline]
fn byte_to_char_index(s: &str, byte_idx: usize) -> usize {
    s[..byte_idx.min(s.len())].chars().count()
}

// ========================================================================== //
//  ObjectString
// ========================================================================== //

/// Represents the `String` built-in type.
pub struct ObjectString {
    pub value: ShrString,
}

impl_object_instance_data!(ObjectString, "string");

macro_rules! string_cmp_op {
    ($name:ident, $op:expr, $op_name:literal) => {
        pub fn $name(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
            let lhs_s = vm.expect_type(vm.obj_heap.get_string_instance(lhs), lhs, "string")?.clone();
            if let Some(rhs_s) = vm.obj_heap.get_string_instance(rhs) {
                return Ok(vm.obj_heap.alloc_bool_instance($op(lhs_s.as_str(), rhs_s.as_str())));
            }
            Err(RuntimeErrorKind::BinaryOpTypeMismatch($op_name, "string", vm.value_type_name(rhs)))
        }
    };
}

impl ObjectString {
    pub fn new(value: ShrString) -> Self {
        Self { value }
    }

    pub fn __add__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_s = vm.expect_type(vm.obj_heap.get_string_instance(lhs), lhs, "string")?.clone();
        if let Some(rhs_s) = vm.obj_heap.get_string_instance(rhs) {
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
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?;
        Ok(vm.obj_heap.alloc_bool_instance(s.is_empty()))
    }

    pub fn __str__(_vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        Ok(receiver)
    }

    native_a1!(__bool__, s: &ShrString, { !s.is_empty() });

    pub fn __hash__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?;
        use std::hash::Hasher;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(s.as_str(), &mut h);
        Ok(vm.obj_heap.alloc_integer_instance(h.finish() as i64))
    }

    pub fn __int__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?;
        let val: i64 = s.as_str().parse().map_err(|_| RuntimeErrorKind::BadIntResult("string"))?;
        Ok(vm.obj_heap.alloc_integer_instance(val))
    }

    pub fn __float__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?;
        let val: f64 = s.as_str().parse().map_err(|_| RuntimeErrorKind::BadFloatResult("string"))?;
        Ok(vm.obj_heap.alloc_float_instance(val))
    }

    native_a1!(__len__, s: &ShrString, { char_count(s.as_str()) as i64 });

    pub fn len(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        Self::__len__(vm, receiver)
    }

    native_a1!(byte_len, s: &ShrString, { s.len() as i64 });

    native_a1!(upper, s: &ShrString, { s.to_uppercase() });

    native_a1!(lower, s: &ShrString, { s.to_lowercase() });

    /// `string.capitalize()` — first character uppercase, rest lowercase.
    pub fn capitalize(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?;
        let mut chars = s.chars();
        match chars.next() {
            None => Ok(receiver),
            Some(first) => {
                let rest: String = chars.as_str().to_lowercase();
                let result = format!("{}{}", first.to_uppercase(), rest);
                Ok(vm.obj_heap.alloc_string_instance(result.into()))
            }
        }
    }

    native_a1!(casefold, s: &ShrString, { s.to_lowercase() });

    /// `string.swapcase()` — swap the case of each character.
    pub fn swapcase(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?;
        let result: String = s
            .chars()
            .map(|c| {
                if c.is_uppercase() {
                    c.to_lowercase().to_string()
                } else if c.is_lowercase() {
                    c.to_uppercase().to_string()
                } else {
                    c.to_string()
                }
            })
            .collect();
        Ok(vm.obj_heap.alloc_string_instance(result.into()))
    }

    /// `string.title()` — title-case the string (first char of each word uppercase).
    pub fn title(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?;
        let mut result = String::with_capacity(s.len());
        let mut new_word = true;
        for c in s.chars() {
            if new_word && c.is_alphanumeric() {
                result.extend(c.to_uppercase());
                new_word = false;
            } else {
                if !c.is_alphanumeric() {
                    new_word = true;
                }
                result.extend(c.to_lowercase());
            }
        }
        Ok(vm.obj_heap.alloc_string_instance(result.into()))
    }

    /// `string.trim()` — remove leading and trailing whitespace.
    pub fn trim(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?;
        Ok(vm.obj_heap.alloc_string_instance(s.trim().to_string().into()))
    }

    /// `string.trim_start()` — remove leading whitespace.
    pub fn trim_start(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?;
        Ok(vm.obj_heap.alloc_string_instance(s.trim_start().to_string().into()))
    }

    /// `string.trim_end()` — remove trailing whitespace.
    pub fn trim_end(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?;
        Ok(vm.obj_heap.alloc_string_instance(s.trim_end().to_string().into()))
    }

    native_a2!(starts_with, s: &ShrString, prefix: ShrString, { s.starts_with(prefix.as_str()) });
    native_a2!(ends_with, s: &ShrString, suffix: ShrString, { s.ends_with(suffix.as_str()) });
    native_a2!(contains, s: &ShrString, sub: ShrString, { s.contains(sub.as_str()) });
    native_a3!(replace, s: &ShrString, old: ShrString, new: ShrString, { s.replace(old.as_str(), new.as_str()) });

    /// `string.split(delim)` — split the string by `delim` and return a list.
    pub fn split(vm: &mut VirtualMachine, receiver: ObjectHandle, delim_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?.clone();
        let delim = vm.expect_type(vm.obj_heap.get_string_instance(delim_handle), delim_handle, "string")?.clone();
        let parts: Vec<ObjectHandle> =
            s.split(delim.as_str()).map(|part| vm.obj_heap.alloc_string_instance(part.to_string().into())).collect();
        Ok(vm.obj_heap.alloc_list_instance(parts))
    }

    /// `string.rsplit(delim)` — split from the right and return a list.
    /// Without a `maxsplit` argument this behaves identically to `split`
    /// (Python's `rsplit` only differs when `maxsplit` is given).
    pub fn rsplit(vm: &mut VirtualMachine, receiver: ObjectHandle, delim_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?.clone();
        let delim = vm.expect_type(vm.obj_heap.get_string_instance(delim_handle), delim_handle, "string")?.clone();
        let parts: Vec<ObjectHandle> =
            s.split(delim.as_str()).map(|part| vm.obj_heap.alloc_string_instance(part.to_string().into())).collect();
        Ok(vm.obj_heap.alloc_list_instance(parts))
    }

    /// `string.splitlines()` — split on line boundaries, return a list.
    pub fn splitlines(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?.clone();
        let parts: Vec<ObjectHandle> = s.lines().map(|part| vm.obj_heap.alloc_string_instance(part.to_string().into())).collect();
        Ok(vm.obj_heap.alloc_list_instance(parts))
    }

    /// `string.partition(sep)` — split at the first occurrence of `sep`, return
    /// a list of [before, sep, after]. If sep is not found, returns [string, "", ""].
    pub fn partition(vm: &mut VirtualMachine, receiver: ObjectHandle, sep_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?.clone();
        let sep = vm.expect_type(vm.obj_heap.get_string_instance(sep_handle), sep_handle, "string")?.clone();
        let sep_str = sep.as_str();
        if let Some(idx) = s.find(sep_str) {
            let before = &s.as_str()[..idx];
            let after = &s.as_str()[idx + sep_str.len()..];
            let parts = vec![
                vm.obj_heap.alloc_string_instance(before.to_string().into()),
                vm.obj_heap.alloc_string_instance(sep_str.to_string().into()),
                vm.obj_heap.alloc_string_instance(after.to_string().into()),
            ];
            Ok(vm.obj_heap.alloc_list_instance(parts))
        } else {
            let parts = vec![receiver, vm.obj_heap.alloc_string_instance("".into()), vm.obj_heap.alloc_string_instance("".into())];
            Ok(vm.obj_heap.alloc_list_instance(parts))
        }
    }

    /// `string.rpartition(sep)` — split at the last occurrence of `sep`, return
    /// a list of [before, sep, after]. If sep is not found, returns ["", "", string].
    pub fn rpartition(vm: &mut VirtualMachine, receiver: ObjectHandle, sep_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?.clone();
        let sep = vm.expect_type(vm.obj_heap.get_string_instance(sep_handle), sep_handle, "string")?.clone();
        let sep_str = sep.as_str();
        if let Some(idx) = s.rfind(sep_str) {
            let before = &s.as_str()[..idx];
            let after = &s.as_str()[idx + sep_str.len()..];
            let parts = vec![
                vm.obj_heap.alloc_string_instance(before.to_string().into()),
                vm.obj_heap.alloc_string_instance(sep_str.to_string().into()),
                vm.obj_heap.alloc_string_instance(after.to_string().into()),
            ];
            Ok(vm.obj_heap.alloc_list_instance(parts))
        } else {
            let parts = vec![vm.obj_heap.alloc_string_instance("".into()), vm.obj_heap.alloc_string_instance("".into()), receiver];
            Ok(vm.obj_heap.alloc_list_instance(parts))
        }
    }

    /// `string.substr(start, length)` — extract a substring by character index.
    pub fn substr(
        vm: &mut VirtualMachine,
        receiver: ObjectHandle,
        start_handle: ObjectHandle,
        length_handle: ObjectHandle,
    ) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?.clone();
        let start = *vm.expect_type(vm.obj_heap.get_integer_instance(start_handle), start_handle, "int")?;
        let length = *vm.expect_type(vm.obj_heap.get_integer_instance(length_handle), length_handle, "int")?;
        let s_str = s.as_str();
        let char_len = char_count(s_str) as i64;

        // Resolve start index (negative counts from end).
        let char_start = if start < 0 {
            let adjusted = char_len + start;
            if adjusted < 0 { 0 } else { adjusted }
        } else if start > char_len {
            char_len
        } else {
            start
        };

        if char_start >= char_len || length <= 0 {
            return Ok(vm.obj_heap.alloc_string_instance("".into()));
        }

        let char_end = {
            let end = char_start + length;
            if end > char_len { char_len } else { end }
        };

        // Convert character indices to byte offsets.
        let byte_start = s_str.char_indices().nth(char_start as usize).map(|(bi, _)| bi).unwrap_or(s_str.len());
        let byte_end = if char_end >= char_len {
            s_str.len()
        } else {
            s_str.char_indices().nth(char_end as usize).map(|(bi, _)| bi).unwrap_or(s_str.len())
        };

        let result = &s_str[byte_start..byte_end];
        Ok(vm.obj_heap.alloc_string_instance(result.to_string().into()))
    }

    native_a2!(find, s: &ShrString, sub: ShrString, {
        match s.find(sub.as_str()) {
            Some(byte_idx) => byte_to_char_index(s.as_str(), byte_idx) as i64,
            None => -1,
        }
    });

    native_a2!(rfind, s: &ShrString, sub: ShrString, {
        match s.rfind(sub.as_str()) {
            Some(byte_idx) => byte_to_char_index(s.as_str(), byte_idx) as i64,
            None => -1,
        }
    });

    native_a2!(count, s: &ShrString, sub: ShrString, { s.matches(sub.as_str()).count() as i64 });

    native_a1!(is_empty, s: &ShrString, { s.is_empty() });

    /// `string.repeat(n)` — repeat the string `n` times.
    pub fn repeat(vm: &mut VirtualMachine, receiver: ObjectHandle, n_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?.clone();
        let n = *vm.expect_type(vm.obj_heap.get_integer_instance(n_handle), n_handle, "int")?;
        if n < 0 {
            return Err(RuntimeErrorKind::IndexOutOfRange(n, 0));
        }
        let result = s.repeat(n as usize);
        Ok(vm.obj_heap.alloc_string_instance(result.into()))
    }

    native_a2!(removeprefix, s: &ShrString, prefix: ShrString, { s.strip_prefix(prefix.as_str()).unwrap_or(s.as_str()).to_string() });

    native_a2!(removesuffix, s: &ShrString, suffix: ShrString, { s.strip_suffix(suffix.as_str()).unwrap_or(s.as_str()).to_string() });

    /// `string.center(width, fillchar)` — center the string in a field of `width`
    /// characters, padding with `fillchar` (must be a single character).
    pub fn center(
        vm: &mut VirtualMachine,
        receiver: ObjectHandle,
        width_handle: ObjectHandle,
        fillchar_handle: ObjectHandle,
    ) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?;
        let width = *vm.expect_type(vm.obj_heap.get_integer_instance(width_handle), width_handle, "int")? as usize;
        let fill_str = vm.expect_type(vm.obj_heap.get_string_instance(fillchar_handle), fillchar_handle, "string")?;
        let fill_char = fill_str.chars().next().unwrap_or(' ');
        let s_str = s.as_str();
        let s_char_len = char_count(s_str);
        if width <= s_char_len {
            return Ok(receiver);
        }
        let left_pad = (width - s_char_len) / 2;
        let right_pad = width - s_char_len - left_pad;
        let mut result = String::with_capacity(width * fill_char.len_utf8());
        for _ in 0..left_pad {
            result.push(fill_char);
        }
        result.push_str(s_str);
        for _ in 0..right_pad {
            result.push(fill_char);
        }
        Ok(vm.obj_heap.alloc_string_instance(result.into()))
    }

    /// `string.ljust(width, fillchar)` — left-justify the string in a field of
    /// `width` characters, padding with `fillchar` on the right.
    pub fn ljust(
        vm: &mut VirtualMachine,
        receiver: ObjectHandle,
        width_handle: ObjectHandle,
        fillchar_handle: ObjectHandle,
    ) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?;
        let width = *vm.expect_type(vm.obj_heap.get_integer_instance(width_handle), width_handle, "int")? as usize;
        let fill_str = vm.expect_type(vm.obj_heap.get_string_instance(fillchar_handle), fillchar_handle, "string")?;
        let fill_char = fill_str.chars().next().unwrap_or(' ');
        let s_str = s.as_str();
        let s_char_len = char_count(s_str);
        if width <= s_char_len {
            return Ok(receiver);
        }
        let pad = width - s_char_len;
        let mut result = String::with_capacity(width * fill_char.len_utf8());
        result.push_str(s_str);
        for _ in 0..pad {
            result.push(fill_char);
        }
        Ok(vm.obj_heap.alloc_string_instance(result.into()))
    }

    /// `string.rjust(width, fillchar)` — right-justify the string in a field of
    /// `width` characters, padding with `fillchar` on the left.
    pub fn rjust(
        vm: &mut VirtualMachine,
        receiver: ObjectHandle,
        width_handle: ObjectHandle,
        fillchar_handle: ObjectHandle,
    ) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?;
        let width = *vm.expect_type(vm.obj_heap.get_integer_instance(width_handle), width_handle, "int")? as usize;
        let fill_str = vm.expect_type(vm.obj_heap.get_string_instance(fillchar_handle), fillchar_handle, "string")?;
        let fill_char = fill_str.chars().next().unwrap_or(' ');
        let s_str = s.as_str();
        let s_char_len = char_count(s_str);
        if width <= s_char_len {
            return Ok(receiver);
        }
        let pad = width - s_char_len;
        let mut result = String::with_capacity(width * fill_char.len_utf8());
        for _ in 0..pad {
            result.push(fill_char);
        }
        result.push_str(s_str);
        Ok(vm.obj_heap.alloc_string_instance(result.into()))
    }

    /// `string.zfill(width)` — pad the string on the left with zeros.
    pub fn zfill(vm: &mut VirtualMachine, receiver: ObjectHandle, width_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?.clone();
        let width = *vm.expect_type(vm.obj_heap.get_integer_instance(width_handle), width_handle, "int")? as usize;
        let s_str = s.as_str();
        let s_char_len = char_count(s_str);
        if width <= s_char_len {
            return Ok(receiver);
        }
        let pad = width - s_char_len;
        let mut result = String::with_capacity(width);
        for _ in 0..pad {
            result.push('0');
        }
        result.push_str(s_str);
        Ok(vm.obj_heap.alloc_string_instance(result.into()))
    }

    // ---- character-class predicates (a1) ----

    native_a1!(is_alnum, s: &ShrString, { !s.is_empty() && s.chars().all(|c| c.is_alphanumeric()) });
    native_a1!(is_alpha, s: &ShrString, { !s.is_empty() && s.chars().all(|c| c.is_alphabetic()) });
    native_a1!(is_ascii, s: &ShrString, { s.is_ascii() });
    native_a1!(is_decimal, s: &ShrString, { !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) });
    native_a1!(is_digit, s: &ShrString, { !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) });
    native_a1!(is_numeric, s: &ShrString, { !s.is_empty() && s.chars().all(|c| c.is_numeric()) });
    native_a1!(is_whitespace, s: &ShrString, { !s.is_empty() && s.chars().all(|c| c.is_whitespace()) });

    /// `string.is_lower()` — at least one cased character exists and all cased
    /// characters are lowercase.
    pub fn is_lower(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?;
        let mut has_cased = false;
        for c in s.chars() {
            if c.is_uppercase() {
                return Ok(vm.obj_heap.alloc_bool_instance(false));
            }
            if c.is_lowercase() {
                has_cased = true;
            }
        }
        Ok(vm.obj_heap.alloc_bool_instance(has_cased))
    }

    /// `string.is_upper()` — at least one cased character exists and all cased
    /// characters are uppercase.
    pub fn is_upper(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?;
        let mut has_cased = false;
        for c in s.chars() {
            if c.is_lowercase() {
                return Ok(vm.obj_heap.alloc_bool_instance(false));
            }
            if c.is_uppercase() {
                has_cased = true;
            }
        }
        Ok(vm.obj_heap.alloc_bool_instance(has_cased))
    }

    /// `string.is_title()` — the string is title-cased: each word starts with an
    /// uppercase character and the rest are lowercase.
    pub fn is_title(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?;
        let mut has_cased = false;
        let mut new_word = true;
        for c in s.chars() {
            if c.is_lowercase() {
                has_cased = true;
            }
            if c.is_uppercase() {
                has_cased = true;
            }
            if new_word {
                // First char in a word must be uppercase if it's a letter.
                if c.is_lowercase() {
                    return Ok(vm.obj_heap.alloc_bool_instance(false));
                }
                if c.is_alphanumeric() {
                    new_word = false;
                }
            } else {
                // Subsequent chars in a word must be lowercase if they're letters.
                if c.is_uppercase() {
                    return Ok(vm.obj_heap.alloc_bool_instance(false));
                }
                if !c.is_alphanumeric() {
                    new_word = true;
                }
            }
        }
        Ok(vm.obj_heap.alloc_bool_instance(has_cased))
    }

    pub fn __getitem__(vm: &mut VirtualMachine, receiver: ObjectHandle, idx_handle: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let s = vm.expect_type(vm.obj_heap.get_string_instance(receiver), receiver, "string")?.clone();
        let idx_val = *vm.expect_type(vm.obj_heap.get_integer_instance(idx_handle), idx_handle, "int")?;
        let char_len = char_count(s.as_str());
        let idx = if idx_val < 0 { char_len as i64 + idx_val } else { idx_val };
        if idx < 0 || idx as usize >= char_len {
            return Err(RuntimeErrorKind::IndexOutOfRange(idx, char_len));
        }
        let ch: String = s.chars().nth(idx as usize).unwrap().into();
        Ok(vm.obj_heap.alloc_string_instance(ch.into()))
    }

    // ---- iteration protocol ----

    pub fn __iter__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let iter = ObjectStringIterator::new(receiver);
        Ok(vm.obj_heap.alloc_instance(vm.obj_heap.string_iter_class, iter))
    }

    pub fn iter_next(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let (string_handle, byte_index) = {
            let iter = vm.expect_type(vm.obj_heap.get_string_iter(receiver), receiver, "string iterator")?;
            (iter.string_handle, iter.byte_index)
        };
        // Borrow the string immutably to extract the next character, then
        // drop the borrow before mutably updating byte_index in the iterator.
        let (char_str, char_len) = {
            let s = vm.expect_type(vm.obj_heap.get_string_instance(string_handle), string_handle, "string")?;
            let remaining = &s.as_str()[byte_index..];
            if let Some(ch) = remaining.chars().next() {
                let cs: String = ch.into();
                let len = cs.len();
                (cs, len)
            } else {
                return Ok(ObjectHandle::ITER_END);
            }
        };
        let found = vm.value_type_name(receiver);
        let iter = vm
            .obj_heap
            .get_string_iter_mut(receiver)
            .ok_or_else(|| RuntimeErrorKind::TypeMismatch { expected: "string iterator", found })?;
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

    // Magic / dunder methods
    heap.register_native_method(sc, "__not__", NativeFunction::a1(ObjectString::__not__));
    heap.register_native_method(sc, "__add__", NativeFunction::a2(ObjectString::__add__));
    heap.register_native_method(sc, "__eq__", NativeFunction::a2(ObjectString::__eq__));
    heap.register_native_method(sc, "__ne__", NativeFunction::a2(ObjectString::__ne__));
    heap.register_native_method(sc, "__gt__", NativeFunction::a2(ObjectString::__gt__));
    heap.register_native_method(sc, "__ge__", NativeFunction::a2(ObjectString::__ge__));
    heap.register_native_method(sc, "__lt__", NativeFunction::a2(ObjectString::__lt__));
    heap.register_native_method(sc, "__le__", NativeFunction::a2(ObjectString::__le__));
    heap.register_native_method(sc, "__str__", NativeFunction::a1(ObjectString::__str__));
    heap.register_native_method(sc, "__bool__", NativeFunction::a1(ObjectString::__bool__));
    heap.register_native_method(sc, "__hash__", NativeFunction::a1(ObjectString::__hash__));
    heap.register_native_method(sc, "__int__", NativeFunction::a1(ObjectString::__int__));
    heap.register_native_method(sc, "__float__", NativeFunction::a1(ObjectString::__float__));
    heap.register_native_method(sc, "__len__", NativeFunction::a1(ObjectString::__len__));
    heap.register_native_method(sc, "__getitem__", NativeFunction::a2(ObjectString::__getitem__));
    heap.register_native_method(sc, "__iter__", NativeFunction::a1(ObjectString::__iter__));

    // a1 — receiver only
    heap.register_native_method(sc, "len", NativeFunction::a1(ObjectString::len));
    heap.register_native_method(sc, "byte_len", NativeFunction::a1(ObjectString::byte_len));
    heap.register_native_method(sc, "upper", NativeFunction::a1(ObjectString::upper));
    heap.register_native_method(sc, "lower", NativeFunction::a1(ObjectString::lower));
    heap.register_native_method(sc, "capitalize", NativeFunction::a1(ObjectString::capitalize));
    heap.register_native_method(sc, "casefold", NativeFunction::a1(ObjectString::casefold));
    heap.register_native_method(sc, "swapcase", NativeFunction::a1(ObjectString::swapcase));
    heap.register_native_method(sc, "title", NativeFunction::a1(ObjectString::title));
    heap.register_native_method(sc, "trim", NativeFunction::a1(ObjectString::trim));
    heap.register_native_method(sc, "trim_start", NativeFunction::a1(ObjectString::trim_start));
    heap.register_native_method(sc, "trim_end", NativeFunction::a1(ObjectString::trim_end));
    heap.register_native_method(sc, "is_empty", NativeFunction::a1(ObjectString::is_empty));
    heap.register_native_method(sc, "is_alnum", NativeFunction::a1(ObjectString::is_alnum));
    heap.register_native_method(sc, "is_alpha", NativeFunction::a1(ObjectString::is_alpha));
    heap.register_native_method(sc, "is_ascii", NativeFunction::a1(ObjectString::is_ascii));
    heap.register_native_method(sc, "is_decimal", NativeFunction::a1(ObjectString::is_decimal));
    heap.register_native_method(sc, "is_digit", NativeFunction::a1(ObjectString::is_digit));
    heap.register_native_method(sc, "is_numeric", NativeFunction::a1(ObjectString::is_numeric));
    heap.register_native_method(sc, "is_whitespace", NativeFunction::a1(ObjectString::is_whitespace));
    heap.register_native_method(sc, "is_lower", NativeFunction::a1(ObjectString::is_lower));
    heap.register_native_method(sc, "is_upper", NativeFunction::a1(ObjectString::is_upper));
    heap.register_native_method(sc, "is_title", NativeFunction::a1(ObjectString::is_title));
    heap.register_native_method(sc, "splitlines", NativeFunction::a1(ObjectString::splitlines));

    // a2 — receiver + 1 argument
    heap.register_native_method(sc, "starts_with", NativeFunction::a2(ObjectString::starts_with));
    heap.register_native_method(sc, "ends_with", NativeFunction::a2(ObjectString::ends_with));
    heap.register_native_method(sc, "contains", NativeFunction::a2(ObjectString::contains));
    heap.register_native_method(sc, "split", NativeFunction::a2(ObjectString::split));
    heap.register_native_method(sc, "rsplit", NativeFunction::a2(ObjectString::rsplit));
    heap.register_native_method(sc, "find", NativeFunction::a2(ObjectString::find));
    heap.register_native_method(sc, "rfind", NativeFunction::a2(ObjectString::rfind));
    heap.register_native_method(sc, "count", NativeFunction::a2(ObjectString::count));
    heap.register_native_method(sc, "repeat", NativeFunction::a2(ObjectString::repeat));
    heap.register_native_method(sc, "removeprefix", NativeFunction::a2(ObjectString::removeprefix));
    heap.register_native_method(sc, "removesuffix", NativeFunction::a2(ObjectString::removesuffix));
    heap.register_native_method(sc, "zfill", NativeFunction::a2(ObjectString::zfill));
    heap.register_native_method(sc, "partition", NativeFunction::a2(ObjectString::partition));
    heap.register_native_method(sc, "rpartition", NativeFunction::a2(ObjectString::rpartition));

    // a3 — receiver + 2 arguments
    heap.register_native_method(sc, "replace", NativeFunction::a3(ObjectString::replace));
    heap.register_native_method(sc, "substr", NativeFunction::a3(ObjectString::substr));
    heap.register_native_method(sc, "center", NativeFunction::a3(ObjectString::center));
    heap.register_native_method(sc, "ljust", NativeFunction::a3(ObjectString::ljust));
    heap.register_native_method(sc, "rjust", NativeFunction::a3(ObjectString::rjust));

    // Aliases
    heap.register_native_method(sc, "to_uppercase", NativeFunction::a1(ObjectString::upper));
    heap.register_native_method(sc, "to_lowercase", NativeFunction::a1(ObjectString::lower));
    heap.register_native_method(sc, "strip", NativeFunction::a1(ObjectString::trim));
    heap.register_native_method(sc, "lstrip", NativeFunction::a1(ObjectString::trim_start));
    heap.register_native_method(sc, "rstrip", NativeFunction::a1(ObjectString::trim_end));
    heap.register_native_method(sc, "index_of", NativeFunction::a2(ObjectString::find));
    heap.register_native_method(sc, "last_index_of", NativeFunction::a2(ObjectString::rfind));

    // String-iterator class
    let sic = heap.string_iter_class;
    heap.register_native_method(sic, "__iter__", NativeFunction::a1(identity_iter));
    heap.register_native_method(sic, "__next__", NativeFunction::a1(ObjectString::iter_next));
}

// ========================================================================== //
//  Tests
// ========================================================================== //

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::VirtualMachine;

    // Helper: allocate a string and return its handle.
    fn str_handle(vm: &mut VirtualMachine, s: &str) -> ObjectHandle {
        vm.obj_heap.alloc_string_instance(s.to_string().into())
    }

    // Helper: call a native a1 method and extract the returned string.
    fn call_a1_str(
        vm: &mut VirtualMachine,
        f: fn(&mut VirtualMachine, ObjectHandle) -> RuntimeResult<ObjectHandle>,
        input: &str,
    ) -> String {
        let h = str_handle(vm, input);
        let r = f(vm, h).unwrap();
        vm.obj_heap.get_string_instance(r).unwrap().as_str().to_string()
    }

    // Helper: call a native a1 method and extract the returned bool.
    fn call_a1_bool(vm: &mut VirtualMachine, f: fn(&mut VirtualMachine, ObjectHandle) -> RuntimeResult<ObjectHandle>, input: &str) -> bool {
        let h = str_handle(vm, input);
        let r = f(vm, h).unwrap();
        *vm.obj_heap.get_bool_instance(r).unwrap()
    }

    // Helper: call a native a1 method and extract the returned int.
    fn call_a1_int(vm: &mut VirtualMachine, f: fn(&mut VirtualMachine, ObjectHandle) -> RuntimeResult<ObjectHandle>, input: &str) -> i64 {
        let h = str_handle(vm, input);
        let r = f(vm, h).unwrap();
        *vm.obj_heap.get_integer_instance(r).unwrap()
    }

    // Helper: call a2 method with string+string args and extract the returned string.
    fn call_a2_str(
        vm: &mut VirtualMachine,
        f: fn(&mut VirtualMachine, ObjectHandle, ObjectHandle) -> RuntimeResult<ObjectHandle>,
        input: &str,
        arg: &str,
    ) -> String {
        let h = str_handle(vm, input);
        let a = str_handle(vm, arg);
        let r = f(vm, h, a).unwrap();
        vm.obj_heap.get_string_instance(r).unwrap().as_str().to_string()
    }

    // Helper: call a2 method with string+string args and extract the returned bool.
    fn call_a2_bool(
        vm: &mut VirtualMachine,
        f: fn(&mut VirtualMachine, ObjectHandle, ObjectHandle) -> RuntimeResult<ObjectHandle>,
        input: &str,
        arg: &str,
    ) -> bool {
        let h = str_handle(vm, input);
        let a = str_handle(vm, arg);
        let r = f(vm, h, a).unwrap();
        *vm.obj_heap.get_bool_instance(r).unwrap()
    }

    // Helper: call a2 method with string+string args and extract the returned int.
    fn call_a2_int(
        vm: &mut VirtualMachine,
        f: fn(&mut VirtualMachine, ObjectHandle, ObjectHandle) -> RuntimeResult<ObjectHandle>,
        input: &str,
        arg: &str,
    ) -> i64 {
        let h = str_handle(vm, input);
        let a = str_handle(vm, arg);
        let r = f(vm, h, a).unwrap();
        *vm.obj_heap.get_integer_instance(r).unwrap()
    }

    // Helper: call a2 method and get the returned list as Vec<String>.
    fn call_a2_list(
        vm: &mut VirtualMachine,
        f: fn(&mut VirtualMachine, ObjectHandle, ObjectHandle) -> RuntimeResult<ObjectHandle>,
        input: &str,
        arg: &str,
    ) -> Vec<String> {
        let h = str_handle(vm, input);
        let a = str_handle(vm, arg);
        let r = f(vm, h, a).unwrap();
        vm.obj_heap
            .get_list_instance(r)
            .unwrap()
            .iter()
            .map(|&item| vm.obj_heap.get_string_instance(item).unwrap().as_str().to_string())
            .collect()
    }

    // Helper: call a1 method and get the returned list as Vec<String>.
    fn call_a1_list(
        vm: &mut VirtualMachine,
        f: fn(&mut VirtualMachine, ObjectHandle) -> RuntimeResult<ObjectHandle>,
        input: &str,
    ) -> Vec<String> {
        let h = str_handle(vm, input);
        let r = f(vm, h).unwrap();
        vm.obj_heap
            .get_list_instance(r)
            .unwrap()
            .iter()
            .map(|&item| vm.obj_heap.get_string_instance(item).unwrap().as_str().to_string())
            .collect()
    }

    // ======================================================================
    //  len / byte_len
    // ======================================================================

    #[test]
    fn test_len_ascii() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_a1_int(&mut vm, ObjectString::len, "hello"), 5);
        assert_eq!(call_a1_int(&mut vm, ObjectString::len, ""), 0);
    }

    #[test]
    fn test_len_unicode() {
        let mut vm = VirtualMachine::new();
        // "你好" = 2 chars, 6 bytes
        assert_eq!(call_a1_int(&mut vm, ObjectString::len, "你好"), 2);
        // "café" = 4 chars, 5 bytes
        assert_eq!(call_a1_int(&mut vm, ObjectString::len, "café"), 4);
        // emoji: "🙂" = 1 char, 4 bytes
        assert_eq!(call_a1_int(&mut vm, ObjectString::len, "🙂"), 1);
        // mixed: "a你好b" = 4 chars
        assert_eq!(call_a1_int(&mut vm, ObjectString::len, "a你好b"), 4);
    }

    #[test]
    fn test_byte_len() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_a1_int(&mut vm, ObjectString::byte_len, "hello"), 5);
        assert_eq!(call_a1_int(&mut vm, ObjectString::byte_len, ""), 0);
        assert_eq!(call_a1_int(&mut vm, ObjectString::byte_len, "你好"), 6);
        assert_eq!(call_a1_int(&mut vm, ObjectString::byte_len, "café"), 5);
        assert_eq!(call_a1_int(&mut vm, ObjectString::byte_len, "🙂"), 4);
    }

    // ======================================================================
    //  upper / lower / capitalize / casefold / swapcase / title
    // ======================================================================

    #[test]
    fn test_upper() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_a1_str(&mut vm, ObjectString::upper, "hello"), "HELLO");
        assert_eq!(call_a1_str(&mut vm, ObjectString::upper, "Hello"), "HELLO");
        assert_eq!(call_a1_str(&mut vm, ObjectString::upper, "HELLO"), "HELLO");
        assert_eq!(call_a1_str(&mut vm, ObjectString::upper, "hello world"), "HELLO WORLD");
    }

    #[test]
    fn test_lower() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_a1_str(&mut vm, ObjectString::lower, "HELLO"), "hello");
        assert_eq!(call_a1_str(&mut vm, ObjectString::lower, "Hello"), "hello");
        assert_eq!(call_a1_str(&mut vm, ObjectString::lower, "hello"), "hello");
    }

    #[test]
    fn test_capitalize() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_a1_str(&mut vm, ObjectString::capitalize, "hello"), "Hello");
        assert_eq!(call_a1_str(&mut vm, ObjectString::capitalize, "HELLO"), "Hello");
        assert_eq!(call_a1_str(&mut vm, ObjectString::capitalize, "hELLO"), "Hello");
        assert_eq!(call_a1_str(&mut vm, ObjectString::capitalize, ""), "");
        assert_eq!(call_a1_str(&mut vm, ObjectString::capitalize, "a"), "A");
        assert_eq!(call_a1_str(&mut vm, ObjectString::capitalize, "123"), "123");
    }

    #[test]
    fn test_casefold() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_a1_str(&mut vm, ObjectString::casefold, "HELLO"), "hello");
        assert_eq!(call_a1_str(&mut vm, ObjectString::casefold, "Straße"), "straße");
    }

    #[test]
    fn test_swapcase() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_a1_str(&mut vm, ObjectString::swapcase, "Hello"), "hELLO");
        assert_eq!(call_a1_str(&mut vm, ObjectString::swapcase, "HELLO"), "hello");
        assert_eq!(call_a1_str(&mut vm, ObjectString::swapcase, "hello"), "HELLO");
        assert_eq!(call_a1_str(&mut vm, ObjectString::swapcase, "hELLo"), "HellO");
        assert_eq!(call_a1_str(&mut vm, ObjectString::swapcase, "123"), "123");
    }

    #[test]
    fn test_title() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_a1_str(&mut vm, ObjectString::title, "hello world"), "Hello World");
        assert_eq!(call_a1_str(&mut vm, ObjectString::title, "HELLO WORLD"), "Hello World");
        assert_eq!(call_a1_str(&mut vm, ObjectString::title, "hELLo wORLD"), "Hello World");
        assert_eq!(call_a1_str(&mut vm, ObjectString::title, "hello-world"), "Hello-World");
        assert_eq!(call_a1_str(&mut vm, ObjectString::title, ""), "");
    }

    // ======================================================================
    //  trim / trim_start / trim_end (and aliases)
    // ======================================================================

    #[test]
    fn test_trim() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_a1_str(&mut vm, ObjectString::trim, "  hello  "), "hello");
        assert_eq!(call_a1_str(&mut vm, ObjectString::trim, "hello"), "hello");
        assert_eq!(call_a1_str(&mut vm, ObjectString::trim, "   "), "");
        assert_eq!(call_a1_str(&mut vm, ObjectString::trim, "\t\nhello\r\n"), "hello");
    }

    #[test]
    fn test_trim_start() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_a1_str(&mut vm, ObjectString::trim_start, "  hello  "), "hello  ");
        assert_eq!(call_a1_str(&mut vm, ObjectString::trim_start, "hello"), "hello");
    }

    #[test]
    fn test_trim_end() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_a1_str(&mut vm, ObjectString::trim_end, "  hello  "), "  hello");
        assert_eq!(call_a1_str(&mut vm, ObjectString::trim_end, "hello"), "hello");
    }

    // ======================================================================
    //  starts_with / ends_with / contains
    // ======================================================================

    #[test]
    fn test_starts_with() {
        let mut vm = VirtualMachine::new();
        assert!(call_a2_bool(&mut vm, ObjectString::starts_with, "hello", "hel"));
        assert!(call_a2_bool(&mut vm, ObjectString::starts_with, "hello", ""));
        assert!(!call_a2_bool(&mut vm, ObjectString::starts_with, "hello", "lo"));
        assert!(call_a2_bool(&mut vm, ObjectString::starts_with, "你好世界", "你好"));
        assert!(!call_a2_bool(&mut vm, ObjectString::starts_with, "你好世界", "世界"));
    }

    #[test]
    fn test_ends_with() {
        let mut vm = VirtualMachine::new();
        assert!(call_a2_bool(&mut vm, ObjectString::ends_with, "hello", "lo"));
        assert!(call_a2_bool(&mut vm, ObjectString::ends_with, "hello", ""));
        assert!(!call_a2_bool(&mut vm, ObjectString::ends_with, "hello", "hel"));
    }

    #[test]
    fn test_contains() {
        let mut vm = VirtualMachine::new();
        assert!(call_a2_bool(&mut vm, ObjectString::contains, "hello", "ell"));
        assert!(call_a2_bool(&mut vm, ObjectString::contains, "hello", ""));
        assert!(!call_a2_bool(&mut vm, ObjectString::contains, "hello", "xyz"));
        assert!(call_a2_bool(&mut vm, ObjectString::contains, "你好世界", "世界"));
    }

    // ======================================================================
    //  replace
    // ======================================================================

    #[test]
    fn test_replace() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "hello world");
        let old = str_handle(&mut vm, "world");
        let new = str_handle(&mut vm, "taro");
        let r = ObjectString::replace(&mut vm, h, old, new).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "hello taro");

        // replace all occurrences
        let h = str_handle(&mut vm, "aaa");
        let old = str_handle(&mut vm, "a");
        let new = str_handle(&mut vm, "b");
        let r = ObjectString::replace(&mut vm, h, old, new).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "bbb");

        // no match
        let h = str_handle(&mut vm, "hello");
        let old = str_handle(&mut vm, "xyz");
        let new = str_handle(&mut vm, "abc");
        let r = ObjectString::replace(&mut vm, h, old, new).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "hello");
    }

    // ======================================================================
    //  split / rsplit / splitlines / partition / rpartition
    // ======================================================================

    #[test]
    fn test_split() {
        let mut vm = VirtualMachine::new();
        let parts = call_a2_list(&mut vm, ObjectString::split, "a,b,c", ",");
        assert_eq!(parts, vec!["a", "b", "c"]);

        let parts = call_a2_list(&mut vm, ObjectString::split, "hello", ",");
        assert_eq!(parts, vec!["hello"]);

        let parts = call_a2_list(&mut vm, ObjectString::split, "", ",");
        assert_eq!(parts, vec![""]);
    }

    #[test]
    fn test_rsplit() {
        let mut vm = VirtualMachine::new();
        let parts = call_a2_list(&mut vm, ObjectString::rsplit, "a,b,c", ",");
        assert_eq!(parts, vec!["a", "b", "c"]);

        // Difference visible with maxsplit, but that's not implemented; still
        // verify correct basic behaviour.
        let parts = call_a2_list(&mut vm, ObjectString::rsplit, "a.b.c", ".");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_splitlines() {
        let mut vm = VirtualMachine::new();
        let parts = call_a1_list(&mut vm, ObjectString::splitlines, "a\nb\nc");
        assert_eq!(parts, vec!["a", "b", "c"]);

        let parts = call_a1_list(&mut vm, ObjectString::splitlines, "hello");
        assert_eq!(parts, vec!["hello"]);

        let parts = call_a1_list(&mut vm, ObjectString::splitlines, "");
        assert_eq!(parts, Vec::<String>::new());

        let parts = call_a1_list(&mut vm, ObjectString::splitlines, "line1\r\nline2");
        assert_eq!(parts, vec!["line1", "line2"]);
    }

    #[test]
    fn test_partition() {
        let mut vm = VirtualMachine::new();
        let parts = call_a2_list(&mut vm, ObjectString::partition, "hello world", " ");
        assert_eq!(parts, vec!["hello", " ", "world"]);

        // sep not found: [string, "", ""]
        let parts = call_a2_list(&mut vm, ObjectString::partition, "hello", ",");
        assert_eq!(parts, vec!["hello", "", ""]);

        // first occurrence only
        let parts = call_a2_list(&mut vm, ObjectString::partition, "a.b.c", ".");
        assert_eq!(parts, vec!["a", ".", "b.c"]);
    }

    #[test]
    fn test_rpartition() {
        let mut vm = VirtualMachine::new();
        let parts = call_a2_list(&mut vm, ObjectString::rpartition, "hello world", " ");
        assert_eq!(parts, vec!["hello", " ", "world"]);

        // sep not found: ["", "", string]
        let parts = call_a2_list(&mut vm, ObjectString::rpartition, "hello", ",");
        assert_eq!(parts, vec!["", "", "hello"]);

        // last occurrence
        let parts = call_a2_list(&mut vm, ObjectString::rpartition, "a.b.c", ".");
        assert_eq!(parts, vec!["a.b", ".", "c"]);
    }

    // ======================================================================
    //  substr
    // ======================================================================

    #[test]
    fn test_substr_ascii() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "hello");
        let start = vm.obj_heap.alloc_integer_instance(1);
        let len = vm.obj_heap.alloc_integer_instance(3);
        let r = ObjectString::substr(&mut vm, h, start, len).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "ell");

        // negative start
        let start = vm.obj_heap.alloc_integer_instance(-2);
        let len = vm.obj_heap.alloc_integer_instance(2);
        let r = ObjectString::substr(&mut vm, h, start, len).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "lo");

        // length zero or negative → empty
        let start = vm.obj_heap.alloc_integer_instance(0);
        let len = vm.obj_heap.alloc_integer_instance(0);
        let r = ObjectString::substr(&mut vm, h, start, len).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "");

        let len = vm.obj_heap.alloc_integer_instance(-1);
        let r = ObjectString::substr(&mut vm, h, start, len).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "");
    }

    #[test]
    fn test_substr_unicode() {
        let mut vm = VirtualMachine::new();
        // "你好世界" = 4 chars
        let h = str_handle(&mut vm, "你好世界");
        let start = vm.obj_heap.alloc_integer_instance(1);
        let len = vm.obj_heap.alloc_integer_instance(2);
        let r = ObjectString::substr(&mut vm, h, start, len).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "好世");

        // negative start
        let start = vm.obj_heap.alloc_integer_instance(-1);
        let len = vm.obj_heap.alloc_integer_instance(1);
        let r = ObjectString::substr(&mut vm, h, start, len).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "界");
    }

    // ======================================================================
    //  find / rfind
    // ======================================================================

    #[test]
    fn test_find() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_a2_int(&mut vm, ObjectString::find, "hello", "l"), 2);
        assert_eq!(call_a2_int(&mut vm, ObjectString::find, "hello", "h"), 0);
        assert_eq!(call_a2_int(&mut vm, ObjectString::find, "hello", "o"), 4);
        assert_eq!(call_a2_int(&mut vm, ObjectString::find, "hello", "xyz"), -1);
        assert_eq!(call_a2_int(&mut vm, ObjectString::find, "hello", ""), 0);
    }

    #[test]
    fn test_find_unicode() {
        let mut vm = VirtualMachine::new();
        // "你好世界" = 好 is at char index 1
        assert_eq!(call_a2_int(&mut vm, ObjectString::find, "你好世界", "好"), 1);
        assert_eq!(call_a2_int(&mut vm, ObjectString::find, "你好世界", "世界"), 2);
        assert_eq!(call_a2_int(&mut vm, ObjectString::find, "你好世界", "啊"), -1);
    }

    #[test]
    fn test_rfind() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_a2_int(&mut vm, ObjectString::rfind, "hello", "l"), 3);
        assert_eq!(call_a2_int(&mut vm, ObjectString::rfind, "hello", "h"), 0);
        assert_eq!(call_a2_int(&mut vm, ObjectString::rfind, "hello", "xyz"), -1);
        assert_eq!(call_a2_int(&mut vm, ObjectString::rfind, "hello", ""), 5);
    }

    #[test]
    fn test_rfind_unicode() {
        let mut vm = VirtualMachine::new();
        // "你好世界你好" = 6 chars; first "好" at index 1, last at index 5
        assert_eq!(call_a2_int(&mut vm, ObjectString::rfind, "你好世界你好", "好"), 5);
        assert_eq!(call_a2_int(&mut vm, ObjectString::rfind, "你好世界", "界"), 3);
    }

    // ======================================================================
    //  count
    // ======================================================================

    #[test]
    fn test_count() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_a2_int(&mut vm, ObjectString::count, "hello", "l"), 2);
        assert_eq!(call_a2_int(&mut vm, ObjectString::count, "hello", "h"), 1);
        assert_eq!(call_a2_int(&mut vm, ObjectString::count, "hello", "xyz"), 0);
        assert_eq!(call_a2_int(&mut vm, ObjectString::count, "aaa", "aa"), 1); // non-overlapping
        assert_eq!(call_a2_int(&mut vm, ObjectString::count, "aaaa", "aa"), 2);
        assert_eq!(call_a2_int(&mut vm, ObjectString::count, "你好你好", "你好"), 2);
    }

    // ======================================================================
    //  is_empty
    // ======================================================================

    #[test]
    fn test_is_empty() {
        let mut vm = VirtualMachine::new();
        assert!(call_a1_bool(&mut vm, ObjectString::is_empty, ""));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_empty, "a"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_empty, " "));
    }

    // ======================================================================
    //  repeat
    // ======================================================================

    #[test]
    fn test_repeat() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "ab");
        let n = vm.obj_heap.alloc_integer_instance(3);
        let r = ObjectString::repeat(&mut vm, h, n).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "ababab");

        let n = vm.obj_heap.alloc_integer_instance(0);
        let r = ObjectString::repeat(&mut vm, h, n).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "");
    }

    #[test]
    fn test_repeat_negative_errors() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "ab");
        let n = vm.obj_heap.alloc_integer_instance(-1);
        assert!(ObjectString::repeat(&mut vm, h, n).is_err());
    }

    // ======================================================================
    //  removeprefix / removesuffix
    // ======================================================================

    #[test]
    fn test_removeprefix() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_a2_str(&mut vm, ObjectString::removeprefix, "hello", "hel"), "lo");
        assert_eq!(call_a2_str(&mut vm, ObjectString::removeprefix, "hello", "xyz"), "hello");
        assert_eq!(call_a2_str(&mut vm, ObjectString::removeprefix, "hello", ""), "hello");
    }

    #[test]
    fn test_removesuffix() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_a2_str(&mut vm, ObjectString::removesuffix, "hello", "lo"), "hel");
        assert_eq!(call_a2_str(&mut vm, ObjectString::removesuffix, "hello", "xyz"), "hello");
        assert_eq!(call_a2_str(&mut vm, ObjectString::removesuffix, "hello", ""), "hello");
    }

    // ======================================================================
    //  center / ljust / rjust / zfill
    // ======================================================================

    fn call_justify_str(
        vm: &mut VirtualMachine,
        f: fn(&mut VirtualMachine, ObjectHandle, ObjectHandle, ObjectHandle) -> RuntimeResult<ObjectHandle>,
        input: &str,
        width: i64,
        fillchar: &str,
    ) -> String {
        let h = str_handle(vm, input);
        let w = vm.obj_heap.alloc_integer_instance(width);
        let fc = str_handle(vm, fillchar);
        let r = f(vm, h, w, fc).unwrap();
        vm.obj_heap.get_string_instance(r).unwrap().as_str().to_string()
    }

    #[test]
    fn test_center() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_justify_str(&mut vm, ObjectString::center, "hi", 6, "-"), "--hi--");
        assert_eq!(call_justify_str(&mut vm, ObjectString::center, "hi", 5, "-"), "-hi--"); // left bias
        assert_eq!(call_justify_str(&mut vm, ObjectString::center, "hi", 2, "-"), "hi");
        assert_eq!(call_justify_str(&mut vm, ObjectString::center, "hi", 0, "-"), "hi");
    }

    #[test]
    fn test_ljust() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_justify_str(&mut vm, ObjectString::ljust, "hi", 6, "-"), "hi----");
        assert_eq!(call_justify_str(&mut vm, ObjectString::ljust, "hi", 2, "-"), "hi");
    }

    #[test]
    fn test_rjust() {
        let mut vm = VirtualMachine::new();
        assert_eq!(call_justify_str(&mut vm, ObjectString::rjust, "hi", 6, "-"), "----hi");
        assert_eq!(call_justify_str(&mut vm, ObjectString::rjust, "hi", 2, "-"), "hi");
    }

    #[test]
    fn test_zfill() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "42");
        let w = vm.obj_heap.alloc_integer_instance(5);
        let r = ObjectString::zfill(&mut vm, h, w).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "00042");

        // width <= len → unchanged
        let h = str_handle(&mut vm, "hello");
        let w = vm.obj_heap.alloc_integer_instance(3);
        let r = ObjectString::zfill(&mut vm, h, w).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "hello");
    }

    // ======================================================================
    //  Character-class predicates
    // ======================================================================

    #[test]
    fn test_is_alnum() {
        let mut vm = VirtualMachine::new();
        assert!(call_a1_bool(&mut vm, ObjectString::is_alnum, "hello123"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_alnum, "hello 123"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_alnum, ""));
    }

    #[test]
    fn test_is_alpha() {
        let mut vm = VirtualMachine::new();
        assert!(call_a1_bool(&mut vm, ObjectString::is_alpha, "hello"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_alpha, "hello123"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_alpha, ""));
    }

    #[test]
    fn test_is_ascii() {
        let mut vm = VirtualMachine::new();
        assert!(call_a1_bool(&mut vm, ObjectString::is_ascii, "hello"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_ascii, "你好"));
        assert!(call_a1_bool(&mut vm, ObjectString::is_ascii, ""));
    }

    #[test]
    fn test_is_decimal() {
        let mut vm = VirtualMachine::new();
        assert!(call_a1_bool(&mut vm, ObjectString::is_decimal, "12345"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_decimal, "12.34"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_decimal, ""));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_decimal, " 123 "));
    }

    #[test]
    fn test_is_digit() {
        let mut vm = VirtualMachine::new();
        assert!(call_a1_bool(&mut vm, ObjectString::is_digit, "12345"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_digit, "12a45"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_digit, ""));
    }

    #[test]
    fn test_is_numeric() {
        let mut vm = VirtualMachine::new();
        assert!(call_a1_bool(&mut vm, ObjectString::is_numeric, "12345"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_numeric, "12.34"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_numeric, ""));
    }

    #[test]
    fn test_is_space() {
        let mut vm = VirtualMachine::new();
        assert!(call_a1_bool(&mut vm, ObjectString::is_whitespace, "   "));
        assert!(call_a1_bool(&mut vm, ObjectString::is_whitespace, "\t\n\r"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_whitespace, " a "));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_whitespace, ""));
    }

    #[test]
    fn test_is_lower() {
        let mut vm = VirtualMachine::new();
        assert!(call_a1_bool(&mut vm, ObjectString::is_lower, "hello"));
        assert!(call_a1_bool(&mut vm, ObjectString::is_lower, "hello world"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_lower, "Hello"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_lower, ""));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_lower, "123")); // no cased chars
    }

    #[test]
    fn test_is_upper() {
        let mut vm = VirtualMachine::new();
        assert!(call_a1_bool(&mut vm, ObjectString::is_upper, "HELLO"));
        assert!(call_a1_bool(&mut vm, ObjectString::is_upper, "HELLO WORLD"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_upper, "Hello"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_upper, ""));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_upper, "123")); // no cased chars
    }

    #[test]
    fn test_is_title() {
        let mut vm = VirtualMachine::new();
        assert!(call_a1_bool(&mut vm, ObjectString::is_title, "Hello World"));
        assert!(call_a1_bool(&mut vm, ObjectString::is_title, "Hello"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_title, "hello world"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_title, "Hello world"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_title, "HELLO WORLD"));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_title, ""));
        assert!(!call_a1_bool(&mut vm, ObjectString::is_title, "123")); // no cased chars
        assert!(call_a1_bool(&mut vm, ObjectString::is_title, "A1 B2"));
    }

    // ======================================================================
    //  __getitem__ (character indexing)
    // ======================================================================

    #[test]
    fn test_getitem_ascii() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "hello");
        let idx = vm.obj_heap.alloc_integer_instance(0);
        let r = ObjectString::__getitem__(&mut vm, h, idx).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "h");

        let idx = vm.obj_heap.alloc_integer_instance(4);
        let r = ObjectString::__getitem__(&mut vm, h, idx).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "o");

        // negative index
        let idx = vm.obj_heap.alloc_integer_instance(-1);
        let r = ObjectString::__getitem__(&mut vm, h, idx).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "o");
    }

    #[test]
    fn test_getitem_unicode() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "你好世界");
        let idx = vm.obj_heap.alloc_integer_instance(0);
        let r = ObjectString::__getitem__(&mut vm, h, idx).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "你");

        let idx = vm.obj_heap.alloc_integer_instance(3);
        let r = ObjectString::__getitem__(&mut vm, h, idx).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "界");

        let idx = vm.obj_heap.alloc_integer_instance(-1);
        let r = ObjectString::__getitem__(&mut vm, h, idx).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "界");
    }

    #[test]
    fn test_getitem_out_of_bounds() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "hi");
        let idx = vm.obj_heap.alloc_integer_instance(5);
        assert!(ObjectString::__getitem__(&mut vm, h, idx).is_err());

        let idx = vm.obj_heap.alloc_integer_instance(-3);
        assert!(ObjectString::__getitem__(&mut vm, h, idx).is_err());
    }

    // ======================================================================
    //  Iterator
    // ======================================================================

    #[test]
    fn test_iter_ascii() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "hi");
        let iter = ObjectString::__iter__(&mut vm, h).unwrap();
        let r1 = ObjectString::iter_next(&mut vm, iter).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r1).unwrap().as_str(), "h");
        let r2 = ObjectString::iter_next(&mut vm, iter).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r2).unwrap().as_str(), "i");
        let r3 = ObjectString::iter_next(&mut vm, iter).unwrap();
        assert!(r3.is_iter_end());
    }

    #[test]
    fn test_iter_unicode() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "a你b");
        let iter = ObjectString::__iter__(&mut vm, h).unwrap();
        let r1 = ObjectString::iter_next(&mut vm, iter).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r1).unwrap().as_str(), "a");
        let r2 = ObjectString::iter_next(&mut vm, iter).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r2).unwrap().as_str(), "你");
        let r3 = ObjectString::iter_next(&mut vm, iter).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r3).unwrap().as_str(), "b");
        let r4 = ObjectString::iter_next(&mut vm, iter).unwrap();
        assert!(r4.is_iter_end());
    }

    // ======================================================================
    //  Magic methods
    // ======================================================================

    #[test]
    fn test_bool_magic() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "hello");
        let r = ObjectString::__bool__(&mut vm, h).unwrap();
        assert!(*vm.obj_heap.get_bool_instance(r).unwrap());

        let h = str_handle(&mut vm, "");
        let r = ObjectString::__bool__(&mut vm, h).unwrap();
        assert!(!*vm.obj_heap.get_bool_instance(r).unwrap());
    }

    #[test]
    fn test_not_magic() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "hello");
        let r = ObjectString::__not__(&mut vm, h).unwrap();
        assert!(!*vm.obj_heap.get_bool_instance(r).unwrap());

        let h = str_handle(&mut vm, "");
        let r = ObjectString::__not__(&mut vm, h).unwrap();
        assert!(*vm.obj_heap.get_bool_instance(r).unwrap());
    }

    #[test]
    fn test_int_magic() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "42");
        let r = ObjectString::__int__(&mut vm, h).unwrap();
        assert_eq!(*vm.obj_heap.get_integer_instance(r).unwrap(), 42);
    }

    #[test]
    fn test_int_magic_invalid() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "hello");
        assert!(ObjectString::__int__(&mut vm, h).is_err());
    }

    #[test]
    fn test_float_magic() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "3.14");
        let r = ObjectString::__float__(&mut vm, h).unwrap();
        assert!((*vm.obj_heap.get_float_instance(r).unwrap() - 3.14).abs() < 0.001);
    }

    #[test]
    fn test_float_magic_invalid() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "hello");
        assert!(ObjectString::__float__(&mut vm, h).is_err());
    }

    #[test]
    fn test_hash_magic() {
        let mut vm = VirtualMachine::new();
        let h1 = str_handle(&mut vm, "hello");
        let h2 = str_handle(&mut vm, "hello");
        let r1 = ObjectString::__hash__(&mut vm, h1).unwrap();
        let r2 = ObjectString::__hash__(&mut vm, h2).unwrap();
        assert_eq!(*vm.obj_heap.get_integer_instance(r1).unwrap(), *vm.obj_heap.get_integer_instance(r2).unwrap(),);
    }

    #[test]
    fn test_str_magic() {
        let mut vm = VirtualMachine::new();
        let h = str_handle(&mut vm, "hello");
        let r = ObjectString::__str__(&mut vm, h).unwrap();
        // __str__ returns the receiver unchanged
        assert_eq!(r, h);
    }

    #[test]
    fn test_add_magic() {
        let mut vm = VirtualMachine::new();
        let h1 = str_handle(&mut vm, "hello");
        let h2 = str_handle(&mut vm, " world");
        let r = ObjectString::__add__(&mut vm, h1, h2).unwrap();
        assert_eq!(vm.obj_heap.get_string_instance(r).unwrap().as_str(), "hello world");
    }

    #[test]
    fn test_cmp_magic() {
        let mut vm = VirtualMachine::new();
        let a = str_handle(&mut vm, "abc");
        let b = str_handle(&mut vm, "abc");
        let c = str_handle(&mut vm, "xyz");

        let r = ObjectString::__eq__(&mut vm, a, b).unwrap();
        assert!(*vm.obj_heap.get_bool_instance(r).unwrap());
        let r = ObjectString::__eq__(&mut vm, a, c).unwrap();
        assert!(!*vm.obj_heap.get_bool_instance(r).unwrap());

        let r = ObjectString::__ne__(&mut vm, a, c).unwrap();
        assert!(*vm.obj_heap.get_bool_instance(r).unwrap());

        let r = ObjectString::__lt__(&mut vm, a, c).unwrap();
        assert!(*vm.obj_heap.get_bool_instance(r).unwrap());
        let r = ObjectString::__lt__(&mut vm, c, a).unwrap();
        assert!(!*vm.obj_heap.get_bool_instance(r).unwrap());

        let r = ObjectString::__gt__(&mut vm, c, a).unwrap();
        assert!(*vm.obj_heap.get_bool_instance(r).unwrap());
    }
}
