use crate::{Object, ObjectHandle, Method, ShrString, format_shr};
use super::{ExecuteError, ExecuteResult, VirtualMachine};

impl VirtualMachine {
    // ================================================================================== //
    //           Core dispatch helper
    // ================================================================================== //

    /// Look up `method_name` on the receiver's class and call it with the given
    /// `args`.  Works for both Builtin and User methods.
    fn dispatch_magic(&mut self, receiver: ObjectHandle, method_name: &'static str, args: &[ObjectHandle]) -> ExecuteResult<ObjectHandle> {
        let class_handle = self.get_instance(receiver)?.class;

        let method = {
            let class = self.get_class(class_handle)?;
            class.methods.get(method_name).copied()
                .ok_or_else(|| ExecuteError::NoImplementMethod(
                    class.name.to_string(),
                    method_name,
                ))?
        };

        match method {
            Method::User(closure_handle) => {
                self.invoke_method_sync(receiver, closure_handle, args)
            }
            Method::Builtin(handle) => {
                // let builtin_fn = self.resolve_builtin(handle);
                let builtin_fn = self.get_builtin_fn(handle).expect("must fn").function;
                self.push_stack(receiver);
                for &arg in args {
                    self.push_stack(arg);
                }
                let total = 1 + args.len();
                let result = builtin_fn(self, total)?;
                // The builtin function reads args from the stack and returns
                // the result value.  The caller (Invoke handler) would normally
                // truncate the stack and push the result.  We mirror that here.
                self.stack.truncate(self.stack.len() - total);
                self.push_stack(result);
                self.pop_stack()
            }
        }
    }

    // ================================================================================== //
    //           Unary ops
    // ================================================================================== //

    pub fn __neg__(&mut self, value: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        self.dispatch_magic(value, "__neg__", &[])
            .map_err(|e| self.remap_unary_error(e, "neg", value))
    }

    pub fn __not__(&mut self, value: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        match self.dispatch_magic(value, "__not__", &[]) {
            Ok(result) => Ok(result),
            Err(ExecuteError::NoImplementMethod(_, _)) => {
                // If __not__ is not implemented, fall back to __bool__ and invert.
                let b = self.__bool__(value)?;
                Ok(self.obj_heap.alloc_bool_instance(!b))
            }
            Err(other) => Err(other),
        }
    }

    // ================================================================================== //
    //           Binary arithmetic ops
    // ================================================================================== //

    pub fn __add__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        self.dispatch_magic(lhs, "__add__", &[rhs])
            .map_err(|e| self.remap_binary_error(e, "add", lhs, rhs))
    }

    pub fn __sub__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        self.dispatch_magic(lhs, "__sub__", &[rhs])
            .map_err(|e| self.remap_binary_error(e, "sub", lhs, rhs))
    }

    pub fn __mul__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        self.dispatch_magic(lhs, "__mul__", &[rhs])
            .map_err(|e| self.remap_binary_error(e, "mul", lhs, rhs))
    }

    pub fn __div__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        self.dispatch_magic(lhs, "__div__", &[rhs])
            .map_err(|e| self.remap_binary_error(e, "div", lhs, rhs))
    }

    // ================================================================================== //
    //           Comparison ops
    // ================================================================================== //

    pub fn __eq__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        // Fast path: same handle => always equal.
        if lhs == rhs {
            return Ok(self.obj_heap.alloc_bool_instance(true));
        }
        match self.dispatch_magic(lhs, "__eq__", &[rhs]) {
            Ok(result) => Ok(result),
            Err(ExecuteError::NoImplementMethod(_, _)) | Err(ExecuteError::UnexpectType(_, _)) => {
                // Different types that don't implement __eq__ are not equal.
                Ok(self.obj_heap.alloc_bool_instance(false))
            }
            Err(other) => Err(other),
        }
    }

    pub fn __ne__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        match self.dispatch_magic(lhs, "__ne__", &[rhs]) {
            Ok(result) => Ok(result),
            Err(ExecuteError::NoImplementMethod(_, _)) => {
                // Fallback: __eq__ + invert.
                let eq = self.__eq__(lhs, rhs)?;
                self.__not__(eq)
            }
            Err(other) => Err(other),
        }
    }

    pub fn __gt__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        self.dispatch_magic(lhs, "__gt__", &[rhs])
            .map_err(|e| self.remap_binary_error(e, "gt", lhs, rhs))
    }

    pub fn __ge__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        match self.dispatch_magic(lhs, "__ge__", &[rhs]) {
            Ok(result) => Ok(result),
            Err(ExecuteError::NoImplementMethod(_, _)) => {
                // Fallback: !(lhs < rhs).
                let lt = self.__lt__(lhs, rhs)?;
                self.__not__(lt)
            }
            Err(other) => Err(other),
        }
    }

    pub fn __lt__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        self.dispatch_magic(lhs, "__lt__", &[rhs])
            .map_err(|e| self.remap_binary_error(e, "lt", lhs, rhs))
    }

    pub fn __le__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        match self.dispatch_magic(lhs, "__le__", &[rhs]) {
            Ok(result) => Ok(result),
            Err(ExecuteError::NoImplementMethod(_, _)) => {
                // Fallback: !(lhs > rhs).
                let gt = self.__gt__(lhs, rhs)?;
                self.__not__(gt)
            }
            Err(other) => Err(other),
        }
    }

    // ================================================================================== //
    //           Type conversion & introspection
    // ================================================================================== //

    pub fn __str__(&mut self, handle: ObjectHandle) -> ExecuteResult<ShrString> {
        let object = self.obj_heap.get(handle);
        match object {
            Object::Instance(_) => {
                match self.dispatch_magic(handle, "__str__", &[]) {
                    Ok(result) => self.get_string_instance(result).cloned()
                        .map_err(|_| ExecuteError::BadStrResult(self.value_type_name(result))),
                    Err(ExecuteError::NoImplementMethod(_, _)) => {
                        // Default representation for instances without __str__.
                        let class_handle = self.get_instance(handle)?.class;
                        let class = self.get_class(class_handle)?;
                        Ok(format_shr!("<instance of {}>", class.name))
                    }
                    Err(other) => Err(other),
                }
            }
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
            Object::Instance(_) => {
                match self.dispatch_magic(handle, "__bool__", &[]) {
                    Ok(result) => Ok(*self.get_bool_instance(result)?),
                    Err(ExecuteError::NoImplementMethod(_, _)) => Ok(true),
                    Err(other) => Err(other),
                }
            }
            _ => Ok(true), 
        }
    }

    pub fn __len__(&mut self, handle: ObjectHandle) -> ExecuteResult<i64> {
        let result = self.dispatch_magic(handle, "__len__", &[])
            .map_err(|e| self.remap_len_error(e, handle))?;
        Ok(*self.get_integer_instance(result)?)
    }

    pub fn __getitem__(&mut self, collection: ObjectHandle, index: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        self.dispatch_magic(collection, "__getitem__", &[index])
    }

    pub fn __setitem__(&mut self, collection: ObjectHandle, index: ObjectHandle, value: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        self.dispatch_magic(collection, "__setitem__", &[index, value])
    }

    pub fn __int__(&mut self, handle: ObjectHandle) -> ExecuteResult<i64> {
        let result = self.dispatch_magic(handle, "__int__", &[])
            .map_err(|e| self.remap_int_error(e, handle))?;
        Ok(*self.get_integer_instance(result)?)
    }

    pub fn __float__(&mut self, handle: ObjectHandle) -> ExecuteResult<f64> {
        let result = self.dispatch_magic(handle, "__float__", &[])
            .map_err(|e| self.remap_float_error(e, handle))?;
        Ok(*self.get_float_instance(result)?)
    }

    // ================================================================================== //
    //           Error remapping helpers
    // ================================================================================== //

    fn remap_unary_error(&self, err: ExecuteError, op: &'static str, value: ObjectHandle) -> ExecuteError {
        match err {
            ExecuteError::NoImplementMethod(_, _) | ExecuteError::UnexpectType(_, _) => {
                ExecuteError::UnaryOpTypeMismatch(op, self.value_type_name(value))
            }
            other => other,
        }
    }

    fn remap_binary_error(&self, err: ExecuteError, op: &'static str, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteError {
        match err {
            ExecuteError::NoImplementMethod(_, _) | ExecuteError::UnexpectType(_, _) => {
                ExecuteError::BinaryOpTypeMismatch(op, self.value_type_name(lhs), self.value_type_name(rhs))
            }
            other => other,
        }
    }

    fn remap_len_error(&self, err: ExecuteError, handle: ObjectHandle) -> ExecuteError {
        match err {
            ExecuteError::NoImplementMethod(_, _) | ExecuteError::UnexpectType(_, _) => {
                ExecuteError::UnexpectType("sequence or mapping", self.value_type_name(handle))
            }
            other => other,
        }
    }

    fn remap_int_error(&self, err: ExecuteError, handle: ObjectHandle) -> ExecuteError {
        match err {
            ExecuteError::NoImplementMethod(_, _) | ExecuteError::UnexpectType(_, _) => {
                ExecuteError::UnexpectType("number or string", self.value_type_name(handle))
            }
            other => other,
        }
    }

    fn remap_float_error(&self, err: ExecuteError, handle: ObjectHandle) -> ExecuteError {
        match err {
            ExecuteError::NoImplementMethod(_, _) | ExecuteError::UnexpectType(_, _) => {
                ExecuteError::UnexpectType("number or string", self.value_type_name(handle))
            }
            other => other,
        }
    }

    // ================================================================================== //
    //           value_type_name
    // ================================================================================== //

    /// Return a human-readable type name for error messages.
    pub fn value_type_name(&self, handle: ObjectHandle) -> &'static str {
        if handle.is_nil() {
            return "nil";
        }
        let object = self.obj_heap.get(handle);
        match object {
            Object::Instance(inst) => {
                // Use the class name for user-defined types, otherwise
                // map known data variants to friendly names.
                use crate::ObjectInstanceData;
                match &inst.data {
                    ObjectInstanceData::Nil => "nil",
                    ObjectInstanceData::Bool(_) => "boolean",
                    ObjectInstanceData::Integer(_) => "integer",
                    ObjectInstanceData::Float(_) => "float",
                    ObjectInstanceData::String(_) => "string",
                    ObjectInstanceData::List(_) => "list",
                    ObjectInstanceData::Dict(_) => "dict",
                    ObjectInstanceData::Fields(_) => "instance",
                }
            }
            Object::BoundMethod(_) => "bound method",
            Object::BuiltinFn(_) => "built-in function",
            Object::Class(_) => "class",
            Object::Closure(_) => "closure",
            Object::Function(_) => "function",
            Object::Upvalue(_) => "upvalue",
        }
    }
}
