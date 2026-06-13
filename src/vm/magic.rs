use crate::{format_shr, ObjectInstanceData, Method, Object, ObjectHandle, ShrString, ToShrString};
use super::{ExecuteError, ExecuteResult, VirtualMachine};

macro_rules! binary_magic_op {
    ($vm:ident, $lhs:ident, $rhs:ident, $method:ident) => {
        paste::paste! {
            // lhs is Instance & has method
            if let Object::Instance(instance) = $vm.obj_heap.get($lhs) {
                let class = $vm.obj_heap.get_class(instance.class)?;
                if let Some(&Method::User(method_handle)) = class.methods.get(stringify!([<__ $method __>])) {
                    return $vm.invoke_method_sync($lhs, method_handle, &[$rhs])
                }
            }
        }
    };
}

macro_rules! binary_magic_op_with_error {
    ($vm:ident, $lhs:ident, $rhs:ident, $method:ident) => {
        paste::paste! {{
            binary_magic_op!($vm, $lhs, $rhs, $method);
            Err(ExecuteError::BinaryOpTypeMismatch(stringify!($method), $vm.value_type_name($lhs), $vm.value_type_name($rhs)))
        }}
    };
}

macro_rules! unary_magic_op_with_error {
    ($vm:ident, $value:ident, $method:ident) => {
        paste::paste! {{
            if let Object::Instance(instance) = $vm.obj_heap.get($value) {
                let class = $vm.obj_heap.get_class(instance.class)?;
                if let Some(&Method::User(method_handle)) = class.methods.get(stringify!([<__ $method __>])) {
                    return $vm.invoke_method_sync($value, method_handle, &[])
                }
            }
            Err(ExecuteError::UnaryOpTypeMismatch(stringify!($method), $vm.value_type_name($value)))
        }}
    };
}

impl VirtualMachine {
    pub fn __neg__(&mut self, value: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let obj = self.obj_heap.get(value);
        match obj {
            Object::Instance(bi) => match &bi.data {
                ObjectInstanceData::Float(v) => Ok(self.obj_heap.alloc_float(-*v)),
                ObjectInstanceData::Integer(v) => Ok(self.obj_heap.alloc_integer(v.wrapping_neg())),
                _ => unary_magic_op_with_error!(self, value, neg),
            },
            _ => unary_magic_op_with_error!(self, value, neg),
        }
    }

    pub fn __not__(&mut self, value: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let obj = self.obj_heap.get(value);
        match obj {
            Object::Instance(inst) => match &inst.data {
                ObjectInstanceData::Nil => Ok(self.obj_heap.alloc_bool(true)),
                ObjectInstanceData::Bool(v) => Ok(self.obj_heap.alloc_bool(!*v)),
                ObjectInstanceData::Integer(v) => Ok(self.obj_heap.alloc_bool(*v == 0)),
                ObjectInstanceData::Float(v) => Ok(self.obj_heap.alloc_bool(*v == 0.0)),
                ObjectInstanceData::String(s) => Ok(self.obj_heap.alloc_bool(s.len() == 0)),
                ObjectInstanceData::List(items) => Ok(self.obj_heap.alloc_bool(items.is_empty())),
                ObjectInstanceData::Dict(entries) => Ok(self.obj_heap.alloc_bool(entries.is_empty())),
                ObjectInstanceData::Fields(_) => {
                    // Try __not__ magic method first for explicit control.
                    let class = self.obj_heap.get_class(inst.class)?;
                    if let Some(&Method::User(not_handle)) = class.methods.get("__not__") {
                        return self.invoke_method_sync(value, not_handle, &[]);
                    }
                    // Fallback: use __bool__ and invert the result.
                    let b = self.__bool__(value)?;
                    Ok(self.obj_heap.alloc_bool(!b))
                }
            },
            _ => {
                // Other objects are truthy.
                Ok(self.obj_heap.alloc_bool(false))
            }
        }
    }

    pub fn __add__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let l = self.obj_heap.get(lhs);
        let r = self.obj_heap.get(rhs);
        match (l, r) {
            (Object::Instance(li), Object::Instance(ri)) => match (&li.data, &ri.data) {
                // Same numeric types
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Integer(rv)) =>
                    Ok(self.obj_heap.alloc_integer(lv.wrapping_add(*rv))),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Float(rv)) =>
                    Ok(self.obj_heap.alloc_float(lv + rv)),
                // Cross-type numbers → promote to Float
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Float(rv)) =>
                    Ok(self.obj_heap.alloc_float(*lv as f64 + rv)),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Integer(rv)) =>
                    Ok(self.obj_heap.alloc_float(lv + *rv as f64)),
                // String concatenation
                (ObjectInstanceData::String(ls), ObjectInstanceData::String(rs)) => {
                    let result = format!("{}{}", ls.as_str(), rs.as_str());
                    Ok(self.obj_heap.alloc_string(result.into()))
                }
                _ => binary_magic_op_with_error!(self, lhs, rhs, add),
            },
            _ => binary_magic_op_with_error!(self, lhs, rhs, add),
        }
    }

    pub fn __sub__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let l = self.obj_heap.get(lhs);
        let r = self.obj_heap.get(rhs);
        match (l, r) {
            (Object::Instance(li), Object::Instance(ri)) => match (&li.data, &ri.data) {
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Integer(rv)) =>
                    Ok(self.obj_heap.alloc_integer(lv.wrapping_sub(*rv))),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Float(rv)) =>
                    Ok(self.obj_heap.alloc_float(lv - rv)),
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Float(rv)) =>
                    Ok(self.obj_heap.alloc_float(*lv as f64 - rv)),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Integer(rv)) =>
                    Ok(self.obj_heap.alloc_float(lv - *rv as f64)),
                _ => binary_magic_op_with_error!(self, lhs, rhs, sub),
            },
            _ => binary_magic_op_with_error!(self, lhs, rhs, sub),
        }
    }

    pub fn __mul__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let l = self.obj_heap.get(lhs);
        let r = self.obj_heap.get(rhs);
        match (l, r) {
            (Object::Instance(li), Object::Instance(ri)) => match (&li.data, &ri.data) {
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Integer(rv)) =>
                    Ok(self.obj_heap.alloc_integer(lv.wrapping_mul(*rv))),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Float(rv)) =>
                    Ok(self.obj_heap.alloc_float(lv * rv)),
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Float(rv)) =>
                    Ok(self.obj_heap.alloc_float(*lv as f64 * rv)),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Integer(rv)) =>
                    Ok(self.obj_heap.alloc_float(lv * *rv as f64)),
                _ => binary_magic_op_with_error!(self, lhs, rhs, mul),
            },
            _ => binary_magic_op_with_error!(self, lhs, rhs, mul),
        }
    }

    pub fn __div__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let l = self.obj_heap.get(lhs);
        let r = self.obj_heap.get(rhs);
        match (l, r) {
            (Object::Instance(li), Object::Instance(ri)) => match (&li.data, &ri.data) {
                (ObjectInstanceData::Integer(_), ObjectInstanceData::Integer(0)) |
                (ObjectInstanceData::Float(_), ObjectInstanceData::Float(0.0)) => {
                    Err(ExecuteError::DivideByZero)
                }
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Integer(rv)) =>
                    Ok(self.obj_heap.alloc_float(*lv as f64 / *rv as f64)),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Float(rv)) =>
                    Ok(self.obj_heap.alloc_float(lv / rv)),
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Float(rv)) =>
                    Ok(self.obj_heap.alloc_float(*lv as f64 / rv)),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Integer(rv)) =>
                    Ok(self.obj_heap.alloc_float(lv / *rv as f64)),
                _ => binary_magic_op_with_error!(self, lhs, rhs, div),
            },
            _ => binary_magic_op_with_error!(self, lhs, rhs, div),
        }
    }

    pub fn __eq__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        // Fast path: same handle
        if lhs == rhs {
            return Ok(self.obj_heap.alloc_bool(true));
        }
        let l = self.obj_heap.get(lhs);
        let r = self.obj_heap.get(rhs);
        match (l, r) {
            (Object::Instance(li), Object::Instance(ri)) => match (&li.data, &ri.data) {
                (ObjectInstanceData::Nil, ObjectInstanceData::Nil) => Ok(self.obj_heap.alloc_bool(true)),
                (ObjectInstanceData::Bool(lv), ObjectInstanceData::Bool(rv)) => Ok(self.obj_heap.alloc_bool(lv == rv)),
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Integer(rv)) => Ok(self.obj_heap.alloc_bool(lv == rv)),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Float(rv)) => Ok(self.obj_heap.alloc_bool(lv == rv)),
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Float(rv)) => Ok(self.obj_heap.alloc_bool(*lv as f64 == *rv)),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Integer(rv)) => Ok(self.obj_heap.alloc_bool(*lv == *rv as f64)),
                (ObjectInstanceData::String(ls), ObjectInstanceData::String(rs)) => Ok(self.obj_heap.alloc_bool(ls.as_str() == rs.as_str())),
                _ => {
                    binary_magic_op!(self, lhs, rhs, eq);
                    // Fall back: different types → false
                    Ok(self.obj_heap.alloc_bool(false))
                }
            },
            _ => {
                // For Instance objects, try __eq__ magic method
                binary_magic_op!(self, lhs, rhs, eq);
                // Fall back: different types → false
                Ok(self.obj_heap.alloc_bool(false))
            }
        }
    }

    pub fn __ne__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        // Try __ne__ magic method first.
        if let Object::Instance(instance) = self.obj_heap.get(lhs) {
            let class = self.obj_heap.get_class(instance.class)?;
            if let Some(&Method::User(method_handle)) = class.methods.get("__ne__") {
                return self.invoke_method_sync(lhs, method_handle, &[rhs]);
            }
        }
        // Fallback: __eq__ + invert.
        let eq = self.__eq__(lhs, rhs)?;
        self.__not__(eq)
    }

    pub fn __gt__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let l = self.obj_heap.get(lhs);
        let r = self.obj_heap.get(rhs);
        match (l, r) {
            (Object::Instance(li), Object::Instance(ri)) => match (&li.data, &ri.data) {
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Integer(rv)) => Ok(self.obj_heap.alloc_bool(lv > rv)),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Float(rv)) => Ok(self.obj_heap.alloc_bool(lv > rv)),
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Float(rv)) => Ok(self.obj_heap.alloc_bool(*lv as f64 > *rv)),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Integer(rv)) => Ok(self.obj_heap.alloc_bool(*lv > *rv as f64)),
                (ObjectInstanceData::String(ls), ObjectInstanceData::String(rs)) => Ok(self.obj_heap.alloc_bool(ls.as_str() > rs.as_str())),
                _ => binary_magic_op_with_error!(self, lhs, rhs, gt),
            },
            _ => binary_magic_op_with_error!(self, lhs, rhs, gt),
        }
    }

    pub fn __ge__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let l = self.obj_heap.get(lhs);
        let r = self.obj_heap.get(rhs);
        match (l, r) {
            (Object::Instance(li), Object::Instance(ri)) => match (&li.data, &ri.data) {
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Integer(rv)) => Ok(self.obj_heap.alloc_bool(lv >= rv)),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Float(rv)) => Ok(self.obj_heap.alloc_bool(lv >= rv)),
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Float(rv)) => Ok(self.obj_heap.alloc_bool(*lv as f64 >= *rv)),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Integer(rv)) => Ok(self.obj_heap.alloc_bool(*lv >= *rv as f64)),
                (ObjectInstanceData::String(ls), ObjectInstanceData::String(rs)) => Ok(self.obj_heap.alloc_bool(ls.as_str() >= rs.as_str())),
                _ => {
                    binary_magic_op!(self, lhs, rhs, ge);
                    let lt = self.__lt__(lhs, rhs)?;
                    self.__not__(lt)
                }
            },
            _ => {
                binary_magic_op!(self, lhs, rhs, ge);
                let lt = self.__lt__(lhs, rhs)?;
                self.__not__(lt)
            }
        }
    }

    pub fn __lt__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let l = self.obj_heap.get(lhs);
        let r = self.obj_heap.get(rhs);
        match (l, r) {
            (Object::Instance(li), Object::Instance(ri)) => match (&li.data, &ri.data) {
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Integer(rv)) => Ok(self.obj_heap.alloc_bool(lv < rv)),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Float(rv)) => Ok(self.obj_heap.alloc_bool(lv < rv)),
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Float(rv)) => Ok(self.obj_heap.alloc_bool((*lv as f64) < *rv)),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Integer(rv)) => Ok(self.obj_heap.alloc_bool(*lv < *rv as f64)),
                (ObjectInstanceData::String(ls), ObjectInstanceData::String(rs)) => Ok(self.obj_heap.alloc_bool(ls.as_str() < rs.as_str())),
                _ => binary_magic_op_with_error!(self, lhs, rhs, lt),
            },
            _ => binary_magic_op_with_error!(self, lhs, rhs, lt),
        }
    }

    pub fn __le__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let l = self.obj_heap.get(lhs);
        let r = self.obj_heap.get(rhs);
        match (l, r) {
            (Object::Instance(li), Object::Instance(ri)) => match (&li.data, &ri.data) {
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Integer(rv)) => Ok(self.obj_heap.alloc_bool(lv <= rv)),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Float(rv)) => Ok(self.obj_heap.alloc_bool(lv <= rv)),
                (ObjectInstanceData::Integer(lv), ObjectInstanceData::Float(rv)) => Ok(self.obj_heap.alloc_bool(*lv as f64 <= *rv)),
                (ObjectInstanceData::Float(lv), ObjectInstanceData::Integer(rv)) => Ok(self.obj_heap.alloc_bool(*lv <= *rv as f64)),
                (ObjectInstanceData::String(ls), ObjectInstanceData::String(rs)) => Ok(self.obj_heap.alloc_bool(ls.as_str() <= rs.as_str())),
                _ => {
                    binary_magic_op!(self, lhs, rhs, le);
                    let gt = self.__gt__(lhs, rhs)?;
                    self.__not__(gt)
                }
            },
            _ => {
                binary_magic_op!(self, lhs, rhs, le);
                let gt = self.__gt__(lhs, rhs)?;
                self.__not__(gt)
            }
        }
    }

    pub fn __str__(&mut self, handle: ObjectHandle) -> ExecuteResult<ShrString> {
        let object = self.obj_heap.get(handle);
        match object {
            Object::Instance(inst) => match &inst.data {
                ObjectInstanceData::Nil => Ok("nil".to_shrstring()),
                ObjectInstanceData::Bool(v) => Ok(format_shr!("{}", v)),
                ObjectInstanceData::Integer(v) => Ok(format_shr!("{}", v)),
                ObjectInstanceData::Float(v) => Ok(format_shr!("{}", v)),
                ObjectInstanceData::String(s) => Ok(s.clone()),
                ObjectInstanceData::List(items) => {
                    let items = items.clone();
                    let mut result = String::from("[");
                    for (i, item) in items.iter().enumerate() {
                        if i > 0 {
                            result.push_str(", ");
                        }
                        result.push_str(&self.__str__(*item)?);
                    }
                    result.push(']');
                    Ok(result.into())
                }
                ObjectInstanceData::Dict(entries) => {
                    let entries = entries.clone();
                    let mut result = String::from("{");
                    let mut first = true;
                    for (k, v) in entries.iter() {
                        if !first {
                            result.push_str(", ");
                        }
                        first = false;
                        result.push_str(&self.__str__(*k)?);
                        result.push_str(": ");
                        result.push_str(&self.__str__(*v)?);
                    }
                    result.push('}');
                    Ok(result.into())
                }
                ObjectInstanceData::Fields(_) => {
                    let class = self.obj_heap.get_class(inst.class)?;
                    if let Some(&Method::User(str_handle)) = class.methods.get("__str__") {
                        let result = self.invoke_method_sync(handle, str_handle, &[])?;
                        // Check if result is a string
                        if let Object::Instance(i) = self.obj_heap.get(result) {
                            if let ObjectInstanceData::String(s) = &i.data {
                                return Ok(s.clone());
                            }
                        }
                        Err(ExecuteError::BadStrResult(self.value_type_name(result)))
                    } else {
                        Ok(format_shr!("<instance of {}>", class.name))
                    }
                }
            },
            Object::Class(c) => Ok(format_shr!("<class '{}'>", c.name)),
            Object::BoundMethod(_) => Ok("<bound method>".into()),
            Object::BuiltinFn(function) => Ok(format_shr!("<built-in function {}>", function.name)),
            Object::Closure(_) => Ok("<closure>".into()),
            Object::Function(function) => Ok(format_shr!("<function {} at {}>", function.name, handle.0)),
            Object::Upvalue(_) => Ok("<upvalue>".into()),
        }
    }

    pub fn __bool__(&mut self, handle: ObjectHandle) -> ExecuteResult<bool> {
        let object = self.obj_heap.get(handle);
        match object {
            Object::Instance(inst) => match &inst.data {
                ObjectInstanceData::Nil => Ok(false),
                ObjectInstanceData::Bool(v) => Ok(*v),
                ObjectInstanceData::Integer(v) => Ok(*v != 0),
                ObjectInstanceData::Float(v) => Ok(*v != 0.0),
                ObjectInstanceData::String(s) => Ok(s.len() != 0),
                ObjectInstanceData::List(items) => Ok(!items.is_empty()),
                ObjectInstanceData::Dict(entries) => Ok(!entries.is_empty()),
                ObjectInstanceData::Fields(_) => {
                    let class = self.obj_heap.get_class(inst.class)?;
                    if let Some(&Method::User(bool_handle)) = class.methods.get("__bool__") {
                        let result = self.invoke_method_sync(handle, bool_handle, &[])?;
                        if let Object::Instance(i) = self.obj_heap.get(result) {
                            if let ObjectInstanceData::Bool(v) = &i.data {
                                return Ok(*v);
                            }
                        }
                        Err(ExecuteError::BadBoolResult(self.value_type_name(result)))
                    } else {
                        Ok(true) // Instance with no __bool__ is truthy
                    }
                }
            },
            _ => Ok(true), // Other objects are truthy
        }
    }

    pub fn __len__(&mut self, handle: ObjectHandle) -> ExecuteResult<i64> {
        let object = self.obj_heap.get(handle);
        match object {
            Object::Instance(inst) => match &inst.data {
                ObjectInstanceData::String(s) => Ok(s.len() as i64),
                ObjectInstanceData::List(items) => Ok(items.len() as i64),
                ObjectInstanceData::Dict(entries) => Ok(entries.len() as i64),
                ObjectInstanceData::Fields(_) => {
                    let class = self.obj_heap.get_class(inst.class)?;
                    if let Some(&Method::User(len_handle)) = class.methods.get("__len__") {
                        let result = self.invoke_method_sync(handle, len_handle, &[])?;
                        if let Object::Instance(bi) = self.obj_heap.get(result) {
                            if let ObjectInstanceData::Integer(v) = &bi.data {
                                return Ok(*v);
                            }
                        }
                        return Err(ExecuteError::BadLenResult(self.value_type_name(result)));
                    }
                    Err(ExecuteError::UnexpectType("sequence or mapping", self.value_type_name(handle)))?
                }
                _ => Err(ExecuteError::UnexpectType("sequence or mapping", self.value_type_name(handle))),
            },
            _ => Err(ExecuteError::UnexpectType("sequence or mapping", self.value_type_name(handle))),
        }
    }

    pub fn __getitem__(&mut self, collection: ObjectHandle, index: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let obj = self.obj_heap.get(collection);
        match obj {
            Object::Instance(inst) => match &inst.data {
                ObjectInstanceData::List(items) => {
                    let idx = self.get_integer(index)?;
                    let len = items.len();
                    let idx = if idx < 0 { len as i64 + idx } else { idx };
                    if idx < 0 || idx as usize >= len {
                        return Err(ExecuteError::IndexOutOfRange(idx, len));
                    }
                    Ok(items[idx as usize])
                }
                ObjectInstanceData::Dict(entries) => {
                    let entries = entries.clone();
                    for &(k, v) in &entries {
                        let eq = self.__eq__(k, index)?;
                        if self.__bool__(eq)? {
                            return Ok(v);
                        }
                    }
                    Err(ExecuteError::KeyNotFound)
                }
                &ObjectInstanceData::Fields(_) => {
                    let class = self.obj_heap.get_class(inst.class)?;
                    if let Some(&Method::User(method_handle)) = class.methods.get("__getitem__") {
                        return self.invoke_method_sync(collection, method_handle, &[index]);
                    }
                    Err(ExecuteError::UnexpectType("list, dict, or instance with __getitem__", self.value_type_name(collection)))
                }
                _ => Err(ExecuteError::UnexpectType("list, dict, or instance with __getitem__", self.value_type_name(collection))),
            },
            _ => Err(ExecuteError::UnexpectType("list, dict, or instance with __getitem__", self.value_type_name(collection))),
        }
    }

    pub fn __setitem__(&mut self, collection: ObjectHandle, index: ObjectHandle, value: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let obj = self.obj_heap.get(collection);
        match obj {
            Object::Instance(instance) => {
                match &instance.data {
                    ObjectInstanceData::List(items) => {
                        let idx = self.get_integer(index)?;
                        let len = items.len();
                        let idx = if idx < 0 { len as i64 + idx } else { idx };
                        if idx < 0 || idx as usize >= len {
                            return Err(ExecuteError::IndexOutOfRange(idx, len));
                        }
                        let bi_mut = self.obj_heap.get_instance_mut(collection)?;
                        match &mut bi_mut.data {
                            ObjectInstanceData::List(items) => {
                                items[idx as usize] = value;
                            }
                            _ => unreachable!(),
                        }
                        Ok(value)
                    }
                    ObjectInstanceData::Dict(_entries) => {
                        // Clone entries, find + remove by comparison, then push new
                        let entries = {
                            let bi = self.obj_heap.get_instance_mut(collection)?;
                            match &bi.data {
                                ObjectInstanceData::Dict(e) => e.clone(),
                                _ => unreachable!(),
                            }
                        };
                        // Find position of existing key (with released borrow)
                        let mut new_entries = entries;
                        let mut found_pos = None;
                        for (i, &(k, _)) in new_entries.iter().enumerate() {
                            let eq = self.__eq__(k, index)?;
                            if self.__bool__(eq)? {
                                found_pos = Some(i);
                                break;
                            }
                        }
                        if let Some(pos) = found_pos {
                            new_entries.remove(pos);
                        }
                        new_entries.push((index, value));
                        // Write back
                        let bi_mut = self.obj_heap.get_instance_mut(collection)?;
                        match &mut bi_mut.data {
                            ObjectInstanceData::Dict(entries) => *entries = new_entries,
                            _ => unreachable!(),
                        }
                        Ok(value)
                    }
                    ObjectInstanceData::Fields(_) => {
                        let class = self.obj_heap.get_class(instance.class)?;
                        if let Some(&Method::User(method_handle)) = class.methods.get("__setitem__") {
                            return self.invoke_method_sync(collection, method_handle, &[index, value]);
                        }
                        Err(ExecuteError::UnexpectType("list, dict, or instance with __setitem__", self.value_type_name(collection)))
                    }
                    _ => Err(ExecuteError::UnexpectType("list, dict, or instance with __setitem__", self.value_type_name(collection))),
                }
            }
            _ => Err(ExecuteError::UnexpectType("list, dict, or instance with __setitem__", self.value_type_name(collection))),
        }
    }

    pub fn __int__(&mut self, handle: ObjectHandle) -> ExecuteResult<i64> {
        let object = self.obj_heap.get(handle);
        match object {
            Object::Instance(inst) => match &inst.data {
                ObjectInstanceData::Integer(v) => Ok(*v),
                ObjectInstanceData::Float(v) => Ok(*v as i64),
                ObjectInstanceData::Bool(v) => Ok(if *v { 1 } else { 0 }),
                ObjectInstanceData::Fields(_) => {
                    let class = self.obj_heap.get_class(inst.class)?;
                    if let Some(&Method::User(method_handle)) = class.methods.get("__int__") {
                        let result = self.invoke_method_sync(handle, method_handle, &[])?;
                        return self.get_integer(result);
                    }
                    Err(ExecuteError::UnexpectType("instance with __int__", self.value_type_name(handle)))
                }
                _ => Err(ExecuteError::UnexpectType("number or string", self.value_type_name(handle))),
            },
            _ => Err(ExecuteError::UnexpectType("instance with __int__", self.value_type_name(handle))),
        }
    }

    pub fn __float__(&mut self, handle: ObjectHandle) -> ExecuteResult<f64> {
        let object = self.obj_heap.get(handle);
        match object {
            Object::Instance(inst) => match &inst.data {
                ObjectInstanceData::Float(v) => Ok(*v),
                ObjectInstanceData::Integer(v) => Ok(*v as f64),
                ObjectInstanceData::Bool(v) => Ok(if *v { 1.0 } else { 0.0 }),
                ObjectInstanceData::Fields(_) => {
                    let class = self.obj_heap.get_class(inst.class)?;
                    if let Some(&Method::User(method_handle)) = class.methods.get("__float__") {
                        let result = self.invoke_method_sync(handle, method_handle, &[])?;
                        return self.get_float(result);
                    }
                    Err(ExecuteError::UnexpectType("instance with __float__", self.value_type_name(handle)))
                }
                _ => Err(ExecuteError::UnexpectType("number or string", self.value_type_name(handle))),
            },
            _ => Err(ExecuteError::UnexpectType("instance with __float__", self.value_type_name(handle))),
        }
    }

    /// Extract i64 from an integer instance handle.
    pub fn get_integer(&self, handle: ObjectHandle) -> ExecuteResult<i64> {
        let obj = self.obj_heap.get(handle);
        match obj {
            Object::Instance(inst) => match &inst.data {
                ObjectInstanceData::Integer(v) => Ok(*v),
                _ => Err(ExecuteError::UnexpectType("integer", self.value_type_name(handle))),
            },
            _ => Err(ExecuteError::UnexpectType("integer", self.value_type_name(handle))),
        }
    }

    /// Extract f64 from a float instance handle.
    pub fn get_float(&self, handle: ObjectHandle) -> ExecuteResult<f64> {
        let obj = self.obj_heap.get(handle);
        match obj {
            Object::Instance(inst) => match &inst.data {
                ObjectInstanceData::Float(v) => Ok(*v),
                _ => Err(ExecuteError::UnexpectType("float", self.value_type_name(handle))),
            },
            _ => Err(ExecuteError::UnexpectType("float", self.value_type_name(handle))),
        }
    }

    /// Return a human-readable type name for error messages.
    pub fn value_type_name(&self, handle: ObjectHandle) -> &'static str {
        if handle.is_nil() {
            return "nil";
        }
        let object = self.obj_heap.get(handle);
        match object {
            Object::Instance(inst) => match &inst.data {
                ObjectInstanceData::Nil => "nil",
                ObjectInstanceData::Bool(_) => "boolean",
                ObjectInstanceData::Integer(_) => "integer",
                ObjectInstanceData::Float(_) => "float",
                ObjectInstanceData::String(_) => "string",
                ObjectInstanceData::List(_) => "list",
                ObjectInstanceData::Dict(_) => "dict",
                ObjectInstanceData::Fields(_) => "instance",
            },
            Object::BoundMethod(_) => "bound method",
            Object::BuiltinFn(_) => "built-in function",
            Object::Class(_) => "class",
            Object::Closure(_) => "closure",
            Object::Function(_) => "function",
            Object::Upvalue(_) => "upvalue",
        }
    }
}
