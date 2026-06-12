use crate::{vm::{ExecuteError, ExecuteResult}, format_shr, Method, Object, ShrString, ToShrString};
use super::{Value, VirtualMachine};

macro_rules! binary_magic_op {
    ($vm:ident, $lhs:ident, $rhs:ident, $method:ident) => {
        paste::paste! {
            // lhs is Object & Instance & has method
            if let Value::Object(handle) = $lhs && let Object::Instance(instance) = $vm.obj_heap.get(*handle) {
                let class = $vm.obj_heap.get_class(instance.class)?;
                if let Some(&Method::User(method_handle)) = class.methods.get(stringify!([<__ $method __>])) {
                    return $vm.invoke_method_sync(*handle, method_handle, &[$rhs.clone()])
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
            // lhs is Object & Instance & has method
            if let Value::Object(handle) = $value && let Object::Instance(instance) = $vm.obj_heap.get(*handle) {
                let class = $vm.obj_heap.get_class(instance.class)?;
                if let Some(&Method::User(method_handle)) = class.methods.get(stringify!([<__ $method __>])) {
                    return $vm.invoke_method_sync(*handle, method_handle, &[])
                }
            }
            Err(ExecuteError::UnaryOpTypeMismatch(stringify!($method), $vm.value_type_name($value)))
        }}
    };
}

impl VirtualMachine {
    pub fn __neg__(&mut self, value: &Value) -> ExecuteResult<Value> {
        match value {
            Value::Float(v) => Ok(Value::Float(-*v)),
            Value::Integer(v) => Ok(Value::Integer(v.wrapping_neg())),
            value => unary_magic_op_with_error!(self, value, neg)
        }
    }

    pub fn __not__(&mut self, value: &Value) -> ExecuteResult<Value> {
        match value {
            Value::Nil => Ok(Value::Bool(true)),
            Value::Bool(v) => Ok(Value::Bool(!*v)),
            Value::Integer(v) => Ok(Value::Bool(*v == 0)),
            Value::Float(v) => Ok(Value::Bool(*v == 0.0)),
            Value::String(s) => Ok(Value::Bool(s.len() == 0)),
            Value::Object(h) => {
                // Try __not__ magic method first for explicit control.
                let not_result = {
                    let object = self.obj_heap.get(*h);
                    if let Object::Instance(instance) = object {
                        let class = self.obj_heap.get_class(instance.class)?;
                        class.methods.get("__not__").copied()
                    } else {
                        None
                    }
                };
                if let Some(Method::User(not_handle)) = not_result {
                    return self.invoke_method_sync(*h, not_handle, &[]);
                }
                // Fallback: use __bool__ and invert the result.
                let b = self.__bool__(value)?;
                Ok(Value::Bool(!b))
            }
        }
    }

    pub fn __add__(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            // Same numeric types
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Integer(l.wrapping_add(*r))),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l + r)),
            // Cross-type numbers → promote to Float
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Float(*l as f64 + r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Float(l + *r as f64)),
            // String concatenation
            (Value::String(l), Value::String(r)) => {
                let result = format!("{}{}", l.as_str(), r.as_str());
                Ok(Value::String(result.into()))
            }
            (lhs, rhs) => binary_magic_op_with_error!(self, lhs, rhs, add),
        }
    }

    pub fn __sub__(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Integer(l.wrapping_sub(*r))),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l - r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Float(*l as f64 - r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Float(l - *r as f64)),
            (lhs, rhs) => binary_magic_op_with_error!(self, lhs, rhs, sub),
        }
    }

    pub fn __mul__(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Integer(l.wrapping_mul(*r))),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l * r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Float(*l as f64 * r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Float(l * *r as f64)),
            (lhs, rhs) => binary_magic_op_with_error!(self, lhs, rhs, mul),
        }
    }

    pub fn __div__(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Integer(..), Value::Integer(0)) | (Value::Float(..), Value::Float(0.0)) => {
                Err(ExecuteError::DivideByZero)
            }
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Float(*l as f64 / *r as f64)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l / r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Float(*l as f64 / r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Float(l / *r as f64)),
            (lhs, rhs) => binary_magic_op_with_error!(self, lhs, rhs, div),
        }
    }

    pub fn __eq__(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Nil, Value::Nil) => Ok(Value::Bool(true)),
            (Value::Bool(l), Value::Bool(r)) => Ok(Value::Bool(l == r)),
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Bool(l == r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l == r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Bool(*l as f64 == *r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Bool(*l == *r as f64)),
            (Value::String(l), Value::String(r)) => Ok(Value::Bool(l.as_str() == r.as_str())),
            // Object identity: same handle → equal.
            (Value::Object(l), Value::Object(r)) => Ok(Value::Bool(l == r)),
            (lhs, rhs) => binary_magic_op_with_error!(self, lhs, rhs, eq),

        }
    }

    pub fn __ne__(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Nil, Value::Nil) => Ok(Value::Bool(false)),
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Bool(l != r)),
            (Value::Bool(l), Value::Bool(r)) => Ok(Value::Bool(l != r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l != r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Bool(*l as f64 != *r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Bool(*l != *r as f64)),
            (Value::String(l), Value::String(r)) => Ok(Value::Bool(l.as_str() != r.as_str())),
            // Object identity: same handle → not unequal.
            (Value::Object(l), Value::Object(r)) => Ok(Value::Bool(l != r)),
            (lhs, rhs) => {
                binary_magic_op!(self, lhs, rhs, ne);
                let invert = self.__eq__(lhs, rhs)?;
                self.__not__(&invert)
            }
        }
    }

    pub fn __gt__(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Bool(l > r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l > r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Bool(*l as f64 > *r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Bool(*l > *r as f64)),
            (Value::String(l), Value::String(r)) => Ok(Value::Bool(l.as_str() > r.as_str())),
            (lhs, rhs) => binary_magic_op_with_error!(self, lhs, rhs, gt),
        }
    }

    pub fn __ge__(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Bool(l >= r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l >= r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Bool(*l as f64 >= *r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Bool(*l >= *r as f64)),
            (Value::String(l), Value::String(r)) => Ok(Value::Bool(l.as_str() >= r.as_str())),
            (lhs, rhs) => {
                binary_magic_op!(self, lhs, rhs, ge);
                let invert = self.__lt__(lhs, rhs)?;
                self.__not__(&invert)
            }
        }
    }

    pub fn __lt__(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Bool(l < r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l < r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Bool((*l as f64) < *r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Bool(*l < *r as f64)),
            (Value::String(l), Value::String(r)) => Ok(Value::Bool(l.as_str() < r.as_str())),
            (lhs, rhs) => binary_magic_op_with_error!(self, lhs, rhs, lt),
        }
    }

    pub fn __le__(&mut self, lhs: &Value, rhs: &Value) -> ExecuteResult<Value> {
        match (lhs, rhs) {
            (Value::Integer(l), Value::Integer(r)) => Ok(Value::Bool(l <= r)),
            (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l <= r)),
            (Value::Integer(l), Value::Float(r)) => Ok(Value::Bool(*l as f64 <= *r)),
            (Value::Float(l), Value::Integer(r)) => Ok(Value::Bool(*l <= *r as f64)),
            (Value::String(l), Value::String(r)) => Ok(Value::Bool(l.as_str() <= r.as_str())),
            (lhs, rhs) => {
                binary_magic_op!(self, lhs, rhs, le);
                let invert = self.__gt__(lhs, rhs)?;
                self.__not__(&invert)
            }
        }
    }

    pub fn __str__(&mut self, value: &Value) -> ExecuteResult<ShrString> {
        match value {
            Value::Float(v) => Ok(format_shr!("{}", v)),
            Value::Integer(v) => Ok(format_shr!("{}", v)),
            Value::Bool(v) => Ok(format_shr!("{}", v)),
            Value::Nil => Ok("nil".to_shrstring()),
            Value::String(s) => Ok(s.clone()),
            Value::Object(h) => {
                let object = self.obj_heap.get(*h);
                match object {
                    Object::Class(c) => Ok(format_shr!("<class '{}'>", c.name)),
                    Object::BoundMethod(_) => Ok("<bound method>".into()),
                    Object::BuiltinFn(function) => Ok(format_shr!("<built-in function {}>", function.name)),
                    Object::Closure(_) => Ok("<closure>".into()),
                    Object::Function(function) => Ok(format_shr!("<function {} at {}>", function.name, h.0)),
                    Object::Instance(instance) => {
                        let class = self.obj_heap.get_class(instance.class)?;
                        if let Some(&Method::User(str_handle)) = class.methods.get("__str__") {
                            let result = self.invoke_method_sync(*h, str_handle, &[])?;
                            if let Value::String(s) = result {
                                return Ok(s);
                            }
                            Err(ExecuteError::BadStrResult(self.value_type_name(&result)))
                        } else {
                            Ok(format_shr!("<instance of {}>", class.name))
                        }
                    }
                    Object::Upvalue(_) => Ok("<upvalue>".into()),
                    Object::List(list) => {
                        let items = list.items.clone();
                        let mut result = String::from("[");
                        for (i, item) in items.iter().enumerate() {
                            if i > 0 {
                                result.push_str(", ");
                            }
                            result.push_str(&self.__str__(item)?);
                        }
                        result.push(']');
                        Ok(result.into())
                    }
                    Object::Dict(dict) => {
                        let items = dict.items.clone();
                        let mut result = String::from("{");
                        let mut first = true;
                        for (k, v) in items.iter() {
                            if !first {
                                result.push_str(", ");
                            }
                            first = false;
                            result.push_str(&self.__str__(k)?);
                            result.push_str(": ");
                            result.push_str(&self.__str__(v)?);
                        }
                        result.push('}');
                        Ok(result.into())
                    }
                }
            }
        }
    }

    pub fn __bool__(&mut self, value: &Value) -> ExecuteResult<bool> {
        match value {
            Value::Float(v) => Ok(*v != 0.0),
            Value::Integer(v) => Ok(*v != 0),
            Value::Bool(v) => Ok(*v),
            Value::Nil => Ok(false),
            Value::String(s) => Ok(s.len() != 0),
            Value::Object(h) => {
                let object = self.obj_heap.get(*h);
                match object {
                    Object::Instance(instance) => {
                        let class = self.obj_heap.get_class(instance.class)?;
                        if let Some(&Method::User(bool_handle)) = class.methods.get("__bool__") {
                            let result = self.invoke_method_sync(*h, bool_handle, &[])?;
                            if let Value::Bool(v) = result {
                                return Ok(v);
                            }
                            Err(ExecuteError::BadBoolResult(self.value_type_name(&result)))
                        } else {
                            Ok(true)
                        }
                    }
                    Object::List(list) => Ok(!list.items.is_empty()),
                    Object::Dict(dict) => Ok(!dict.items.is_empty()),
                    _ => Ok(true),
                }
            }
        }
    }

    pub fn __len__(&mut self, value: &Value) -> ExecuteResult<i64> {
        match value {
            Value::String(s) => Ok(s.len() as i64),
            Value::Object(h) => {
                match self.obj_heap.get(*h) {
                    Object::Instance(instance) => {
                        let class = self.obj_heap.get_class(instance.class)?;
                        if let Some(&Method::User(len_handle)) = class.methods.get("__len__") {
                            let result = self.invoke_method_sync(*h, len_handle, &[])?;
                            if let Value::Integer(v) = result {
                                return Ok(v);
                            }
                            return Err(ExecuteError::BadLenResult(self.value_type_name(&result)));
                        }
                    }
                    Object::List(list) => {
                        return Ok(list.items.len() as i64);
                    }
                    Object::Dict(dict) => {
                        return Ok(dict.items.len() as i64);
                    }
                    _ => {}
                };
                Err(ExecuteError::UnexpectType("string or instance with __len__", self.value_type_name(value)))
            }
            _ => Err(ExecuteError::UnexpectType("string or instance with __len__", self.value_type_name(value)))
        }
    }

    pub fn __getitem__(&mut self, collection: &Value, index: &Value) -> ExecuteResult<Value> {
        match collection {
            Value::Object(h) => match self.obj_heap.get(*h) {
                Object::List(list) => {
                    let i = match index {
                        Value::Integer(i) => *i,
                        _ => return Err(ExecuteError::UnexpectType("integer index", self.value_type_name(index))),
                    };
                    let len = list.items.len();
                    let idx = if i < 0 { len as i64 + i } else { i };
                    if idx < 0 || idx as usize >= len {
                        return Err(ExecuteError::IndexOutOfRange(i, len));
                    }
                    Ok(list.items[idx as usize].clone())
                }
                Object::Dict(dict) => {
                    dict.items.get(index).cloned()
                        .ok_or(ExecuteError::KeyNotFound)
                }
                Object::Instance(instance) => {
                    let class = self.obj_heap.get_class(instance.class)?;
                    if let Some(&Method::User(method_handle)) = class.methods.get("__getitem__") {
                        return self.invoke_method_sync(*h, method_handle, &[index.clone()]);
                    }
                    Err(ExecuteError::UnexpectType("list, dict, or instance with __getitem__", self.value_type_name(collection)))
                }
                _ => Err(ExecuteError::UnexpectType("list, dict, or instance with __getitem__", self.value_type_name(collection))),
            },
            _ => Err(ExecuteError::UnexpectType("list, dict, or instance with __getitem__", self.value_type_name(collection))),
        }
    }

    pub fn __setitem__(&mut self, collection: &Value, index: &Value, value: &Value) -> ExecuteResult<Value> {
        match collection {
            Value::Object(h) => match self.obj_heap.get(*h) {
                Object::List(list) => {
                    let i = match index {
                        Value::Integer(i) => *i,
                        _ => return Err(ExecuteError::UnexpectType("integer index", self.value_type_name(index))),
                    };
                    let len = list.items.len();
                    let idx = if i < 0 { len as i64 + i } else { i };
                    if idx < 0 || idx as usize >= len {
                        return Err(ExecuteError::IndexOutOfRange(i, len));
                    }
                    let list_mut = self.obj_heap.get_list_mut(*h)?;
                    list_mut.items[idx as usize] = value.clone();
                    Ok(value.clone())
                }
                Object::Dict(_dict) => {
                    let dict_mut = self.obj_heap.get_dict_mut(*h)?;
                    dict_mut.items.insert(index.clone(), value.clone());
                    Ok(value.clone())
                }
                Object::Instance(instance) => {
                    let class = self.obj_heap.get_class(instance.class)?;
                    if let Some(&Method::User(method_handle)) = class.methods.get("__setitem__") {
                        return self.invoke_method_sync(*h, method_handle, &[index.clone(), value.clone()]);
                    }
                    Err(ExecuteError::UnexpectType("list, dict, or instance with __setitem__", self.value_type_name(collection)))
                }
                _ => Err(ExecuteError::UnexpectType("list, dict, or instance with __setitem__", self.value_type_name(collection))),
            },
            _ => Err(ExecuteError::UnexpectType("list, dict, or instance with __setitem__", self.value_type_name(collection))),
        }
    }

    pub fn __int__(&mut self, value: &Value) -> ExecuteResult<i64> {
        match value {
            Value::Integer(v) => Ok(*v),
            Value::Float(v) => Ok(*v as i64),
            Value::Bool(v) => Ok(if *v { 1 } else { 0 }),
            Value::Object(h) => {
                if let Object::Instance(instance) = self.obj_heap.get(*h) {
                    let class = self.obj_heap.get_class(instance.class)?;
                    if let Some(&Method::User(method_handle)) = class.methods.get("__int__") {
                        let result = self.invoke_method_sync(*h, method_handle, &[])?;
                        if let Value::Integer(v) = result {
                            return Ok(v);
                        }
                        return Err(ExecuteError::BadIntResult(self.value_type_name(&result)));
                    }
                }
                Err(ExecuteError::UnexpectType("instance with __int__", self.value_type_name(value)))
            }
            _ => Err(ExecuteError::UnexpectType("instance with __int__", self.value_type_name(value)))
        }
    }

    pub fn __float__(&mut self, value: &Value) -> ExecuteResult<f64> {
        match value {
            Value::Float(v) => Ok(*v),
            Value::Integer(v) => Ok(*v as f64),
            Value::Bool(v) => Ok(if *v { 1.0 } else { 0.0 }),
            Value::Object(h) => {
                if let Object::Instance(instance) = self.obj_heap.get(*h) {
                    let class = self.obj_heap.get_class(instance.class)?;
                    if let Some(&Method::User(method_handle)) = class.methods.get("__float__") {
                        let result = self.invoke_method_sync(*h, method_handle, &[])?;
                        if let Value::Float(v) = result {
                            return Ok(v);
                        }
                        return Err(ExecuteError::BadFloatResult(self.value_type_name(&result)));
                    }
                }
                Err(ExecuteError::UnexpectType("instance with __float__", self.value_type_name(value)))
            }
            _ => Err(ExecuteError::UnexpectType("instance with __float__", self.value_type_name(value)))
        }
    }

    pub fn value_type_name(&self, value: &Value) -> &'static str {
        match value {
            Value::Float(_) => "float",
            Value::Integer(_) => "integer",
            Value::Bool(_) => "boolean",
            Value::Nil => "nil",
            Value::String(_) => "string",
            Value::Object(handle) => {
                let object = self.obj_heap.get(*handle);
                match object {
                    Object::BoundMethod(_) => "bound method",
                    Object::BuiltinFn(_) => "built-in function",
                    Object::Class(_) => "class",
                    Object::Closure(_) => "closure",
                    Object::Function(_) => "function",
                    Object::Instance(_) => "instance",
                    Object::Upvalue(_) => "upvalue",
                    Object::List(_) => "list",
                    Object::Dict(_) => "dict",
                }
            }
        }
    }
}
