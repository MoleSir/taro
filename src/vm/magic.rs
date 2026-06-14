use crate::{Object, ObjectHandle, Method, ShrString, format_shr};
use super::{ExecuteError, ExecuteResult, VirtualMachine};

impl VirtualMachine {
    // ================================================================================== //
    //           Core dispatch helper
    // ================================================================================== //

    /// Look up `method_name` on the receiver's class and call it with the given
    /// `args`.  Works for both Native and User methods.
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
            Method::Native(handle) => {
                let native_fn = self.get_native_fn(handle).expect("must fn").function;
                self.push_stack(receiver);
                for &arg in args {
                    self.push_stack(arg);
                }
                self.call_native_fn(native_fn, 1 + args.len(), false)?;
                self.pop_stack()
            }
        }
    }

    // ================================================================================== //
    //           __call__ — Python-style callable instances
    // ================================================================================== //

    /// Invoke `__call__` on `callee` (an Instance).
    ///
    /// The stack already contains `[callee, arg1, ..., argN]`.
    /// `arg_count` is the number of explicit arguments (the N in `callee(a1..aN)`),
    /// so the stack has `arg_count + 1` items belonging to this call.
    pub fn __call__(&mut self, callee: ObjectHandle, arg_count: usize) -> ExecuteResult<()> {
        let instance = self.get_instance(callee)
            .map_err(|_| ExecuteError::CanNotCall(self.value_type_name(callee)))?;
        let method = {
            let class = self.get_class(instance.class)?;
            class.methods.get("__call__").copied()
                .ok_or_else(|| ExecuteError::NoImplementMethod(
                    class.name.to_string(), "__call__",
                ))?
        };

        match method {
            Method::User(closure_handle) => {
                self.call_closure(closure_handle, arg_count + 1, false)
            }
            Method::Native(handle) => {
                let native_fn = self.get_native_fn(handle)?.function;
                self.call_native_fn(native_fn, arg_count + 1, false)
            }
        }
    }

    // ================================================================================== //
    //           Unary ops
    // ================================================================================== //

    pub fn __neg__(&mut self, value: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(value), Object::Instance(_)) {
            return Err(ExecuteError::UnaryOpTypeMismatch("neg", self.value_type_name(value)));
        }
        self.dispatch_magic(value, "__neg__", &[])
            .map_err(|e| self.remap_unary_error(e, "neg", value))
    }

    pub fn __not__(&mut self, value: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        // nil is falsy, so !nil == true.
        if value.is_nil() {
            return Ok(self.obj_heap.alloc_bool_instance(true));
        }
        // Non-Instance objects can't implement __not__ — fall back to __bool__ + invert.
        if !matches!(self.obj_heap.get(value), Object::Instance(_)) {
            let b = self.__bool__(value)?;
            return Ok(self.obj_heap.alloc_bool_instance(!b));
        }
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
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(ExecuteError::BinaryOpTypeMismatch("add", self.value_type_name(lhs), self.value_type_name(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(ExecuteError::BinaryOpTypeMismatch("add", self.value_type_name(lhs), self.value_type_name(rhs)));
        }
        self.dispatch_magic(lhs, "__add__", &[rhs])
            .map_err(|e| self.remap_binary_error(e, "add", lhs, rhs))
    }

    pub fn __sub__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(ExecuteError::BinaryOpTypeMismatch("sub", self.value_type_name(lhs), self.value_type_name(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(ExecuteError::BinaryOpTypeMismatch("sub", self.value_type_name(lhs), self.value_type_name(rhs)));
        }
        self.dispatch_magic(lhs, "__sub__", &[rhs])
            .map_err(|e| self.remap_binary_error(e, "sub", lhs, rhs))
    }

    pub fn __mul__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(ExecuteError::BinaryOpTypeMismatch("mul", self.value_type_name(lhs), self.value_type_name(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(ExecuteError::BinaryOpTypeMismatch("mul", self.value_type_name(lhs), self.value_type_name(rhs)));
        }
        self.dispatch_magic(lhs, "__mul__", &[rhs])
            .map_err(|e| self.remap_binary_error(e, "mul", lhs, rhs))
    }

    pub fn __div__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(ExecuteError::BinaryOpTypeMismatch("div", self.value_type_name(lhs), self.value_type_name(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(ExecuteError::BinaryOpTypeMismatch("div", self.value_type_name(lhs), self.value_type_name(rhs)));
        }
        self.dispatch_magic(lhs, "__div__", &[rhs])
            .map_err(|e| self.remap_binary_error(e, "div", lhs, rhs))
    }

    // ================================================================================== //
    //           Comparison ops
    // ================================================================================== //

    pub fn __eq__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        // Fast path: same handle => always equal (covers nil == nil).
        if lhs == rhs {
            return Ok(self.obj_heap.alloc_bool_instance(true));
        }
        // nil is only equal to nil; already handled above (different handles).
        if lhs.is_nil() || rhs.is_nil() {
            return Ok(self.obj_heap.alloc_bool_instance(false));
        }
        // Non‑Instance objects (Class, NativeFn, Closure, etc.) are only
        // equal to themselves, which the fast path already covers.
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Ok(self.obj_heap.alloc_bool_instance(false));
        }
        match self.dispatch_magic(lhs, "__eq__", &[rhs]) {
            Ok(result) => Ok(result),
            Err(ExecuteError::NoImplementMethod(_, _)) | Err(ExecuteError::UnexpectedType(_, _)) => {
                // Different types that don't implement __eq__ are not equal.
                Ok(self.obj_heap.alloc_bool_instance(false))
            }
            Err(other) => Err(other),
        }
    }

    pub fn __ne__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        // nil != nil is false; nil != anything-else is true.
        if lhs.is_nil() {
            return Ok(self.obj_heap.alloc_bool_instance(!rhs.is_nil()));
        }
        if rhs.is_nil() {
            return Ok(self.obj_heap.alloc_bool_instance(true));
        }
        // Non‑Instance objects: different handles => not equal.
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Ok(self.obj_heap.alloc_bool_instance(true));
        }
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
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(ExecuteError::BinaryOpTypeMismatch("gt", self.value_type_name(lhs), self.value_type_name(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(ExecuteError::BinaryOpTypeMismatch("gt", self.value_type_name(lhs), self.value_type_name(rhs)));
        }
        self.dispatch_magic(lhs, "__gt__", &[rhs])
            .map_err(|e| self.remap_binary_error(e, "gt", lhs, rhs))
    }

    pub fn __ge__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(ExecuteError::BinaryOpTypeMismatch("ge", self.value_type_name(lhs), self.value_type_name(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(ExecuteError::BinaryOpTypeMismatch("ge", self.value_type_name(lhs), self.value_type_name(rhs)));
        }
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
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(ExecuteError::BinaryOpTypeMismatch("lt", self.value_type_name(lhs), self.value_type_name(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(ExecuteError::BinaryOpTypeMismatch("lt", self.value_type_name(lhs), self.value_type_name(rhs)));
        }
        self.dispatch_magic(lhs, "__lt__", &[rhs])
            .map_err(|e| self.remap_binary_error(e, "lt", lhs, rhs))
    }

    pub fn __le__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(ExecuteError::BinaryOpTypeMismatch("le", self.value_type_name(lhs), self.value_type_name(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(ExecuteError::BinaryOpTypeMismatch("le", self.value_type_name(lhs), self.value_type_name(rhs)));
        }
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
        // nil is a special case — it has no class to dispatch to.
        if handle.is_nil() {
            return Ok("nil".into());
        }
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
            Object::NativeFn(function) => Ok(format_shr!("<native function {}>", function.name)),
            Object::Closure(_) => Ok("<closure>".into()),
            Object::Function(function) => Ok(format_shr!("<function {} at {}>", function.name, handle.0)),
            Object::Upvalue(_) => Ok("<upvalue>".into()),
        }
    }

    pub fn __bool__(&mut self, handle: ObjectHandle) -> ExecuteResult<bool> {
        // nil is falsy.
        if handle.is_nil() {
            return Ok(false);
        }
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
        if !matches!(self.obj_heap.get(handle), Object::Instance(_)) {
            return Err(ExecuteError::UnexpectedType("object with __len__", self.value_type_name(handle)));
        }
        let result = self.dispatch_magic(handle, "__len__", &[])
            .map_err(|e| self.remap_len_error(e, handle))?;
        Ok(*self.get_integer_instance(result)?)
    }

    pub fn __getitem__(&mut self, collection: ObjectHandle, index: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(collection), Object::Instance(_)) {
            return Err(ExecuteError::UnexpectedType("object with __getitem__", self.value_type_name(collection)));
        }
        self.dispatch_magic(collection, "__getitem__", &[index])
    }

    pub fn __setitem__(&mut self, collection: ObjectHandle, index: ObjectHandle, value: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(collection), Object::Instance(_)) {
            return Err(ExecuteError::UnexpectedType("object with __setitem__", self.value_type_name(collection)));
        }
        self.dispatch_magic(collection, "__setitem__", &[index, value])
    }

    pub fn __int__(&mut self, handle: ObjectHandle) -> ExecuteResult<i64> {
        if !matches!(self.obj_heap.get(handle), Object::Instance(_)) {
            return Err(ExecuteError::UnexpectedType("object with __int__", self.value_type_name(handle)));
        }
        let result = self.dispatch_magic(handle, "__int__", &[])
            .map_err(|e| self.remap_int_error(e, handle))?;
        Ok(*self.get_integer_instance(result)?)
    }

    pub fn __float__(&mut self, handle: ObjectHandle) -> ExecuteResult<f64> {
        if !matches!(self.obj_heap.get(handle), Object::Instance(_)) {
            return Err(ExecuteError::UnexpectedType("object with __float__", self.value_type_name(handle)));
        }
        let result = self.dispatch_magic(handle, "__float__", &[])
            .map_err(|e| self.remap_float_error(e, handle))?;
        Ok(*self.get_float_instance(result)?)
    }

    // ================================================================================== //
    //           Error remapping helpers
    // ================================================================================== //

    fn remap_unary_error(&self, err: ExecuteError, op: &'static str, value: ObjectHandle) -> ExecuteError {
        match err {
            ExecuteError::NoImplementMethod(_, _) | ExecuteError::UnexpectedType(_, _) => {
                ExecuteError::UnaryOpTypeMismatch(op, self.value_type_name(value))
            }
            other => other,
        }
    }

    fn remap_binary_error(&self, err: ExecuteError, op: &'static str, lhs: ObjectHandle, rhs: ObjectHandle) -> ExecuteError {
        match err {
            ExecuteError::NoImplementMethod(_, _) | ExecuteError::UnexpectedType(_, _) => {
                ExecuteError::BinaryOpTypeMismatch(op, self.value_type_name(lhs), self.value_type_name(rhs))
            }
            other => other,
        }
    }

    fn remap_len_error(&self, err: ExecuteError, handle: ObjectHandle) -> ExecuteError {
        match err {
            ExecuteError::UnexpectedType(_, _) => {
                ExecuteError::UnexpectedType("object with __len__", self.value_type_name(handle))
            }
            // NoImplementMethod already says which class is missing __len__ — keep it.
            other => other,
        }
    }

    fn remap_int_error(&self, err: ExecuteError, handle: ObjectHandle) -> ExecuteError {
        match err {
            ExecuteError::UnexpectedType(_, _) => {
                ExecuteError::UnexpectedType("object with __int__", self.value_type_name(handle))
            }
            // NoImplementMethod already says which class is missing __int__ — keep it.
            other => other,
        }
    }

    fn remap_float_error(&self, err: ExecuteError, handle: ObjectHandle) -> ExecuteError {
        match err {
            ExecuteError::UnexpectedType(_, _) => {
                ExecuteError::UnexpectedType("object with __float__", self.value_type_name(handle))
            }
            // NoImplementMethod already says which class is missing __float__ — keep it.
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
            Object::NativeFn(_) => "native function",
            Object::Class(_) => "class",
            Object::Closure(_) => "closure",
            Object::Function(_) => "function",
            Object::Upvalue(_) => "upvalue",
        }
    }
}
