use super::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{Method, Object, ObjectHandle, ShrString, format_shr};

impl VirtualMachine {
    // ================================================================================== //
    //           Core dispatch helper
    // ================================================================================== //

    /// Look up `method_name` on the receiver's class and call it with the given
    /// `args`.  Works for both Native and User methods.
    pub(crate) fn dispatch_magic(
        &mut self,
        receiver: ObjectHandle,
        method_name: &'static str,
        args: &[ObjectHandle],
    ) -> RuntimeResult<ObjectHandle> {
        let class_handle = self.obj_heap.expect_instance(receiver)?.class;

        let method = {
            let class = self.obj_heap.expect_class(class_handle)?;
            class
                .methods
                .get(method_name)
                .copied()
                .ok_or_else(|| RuntimeErrorKind::NoImplementMethod(class.name.to_string(), method_name))?
        };

        match method {
            Method::User(closure_handle) => self.invoke_method_sync(receiver, closure_handle, args),
            Method::Native(handle) => {
                let native_fn = self.obj_heap.get_native_fn(handle).expect("must fn").function;
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
    pub fn __call__(&mut self, callee: ObjectHandle, arg_count: usize) -> RuntimeResult<()> {
        let instance = self.obj_heap.get_instance(callee).ok_or_else(|| RuntimeErrorKind::CanNotCall(self.obj_heap.type_of(callee)))?;
        let method = {
            let class = self.obj_heap.expect_class(instance.class)?;
            class
                .methods
                .get("__call__")
                .copied()
                .ok_or_else(|| RuntimeErrorKind::NoImplementMethod(class.name.to_string(), "__call__"))?
        };

        match method {
            Method::User(closure_handle) => self.call_closure(closure_handle, arg_count + 1, false),
            Method::Native(handle) => {
                let native_fn = self.obj_heap.get_native_fn(handle).expect("must fn").function;
                self.call_native_fn(native_fn, arg_count + 1, false)
            }
        }
    }

    // ================================================================================== //
    //           Unary ops
    // ================================================================================== //

    pub fn __neg__(&mut self, value: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(value), Object::Instance(_)) {
            return Err(RuntimeErrorKind::UnaryOpTypeMismatch("neg", self.obj_heap.type_of(value)));
        }
        self.dispatch_magic(value, "__neg__", &[]).map_err(|e| self.remap_unary_error(e, "neg", value))
    }

    pub fn __not__(&mut self, value: ObjectHandle) -> RuntimeResult<ObjectHandle> {
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
            Err(RuntimeErrorKind::NoImplementMethod(_, _)) => {
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

    pub fn __add__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("add", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("add", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        self.dispatch_magic(lhs, "__add__", &[rhs]).map_err(|e| self.remap_binary_error(e, "add", lhs, rhs))
    }

    pub fn __sub__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("sub", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("sub", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        self.dispatch_magic(lhs, "__sub__", &[rhs]).map_err(|e| self.remap_binary_error(e, "sub", lhs, rhs))
    }

    pub fn __mul__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("mul", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("mul", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        self.dispatch_magic(lhs, "__mul__", &[rhs]).map_err(|e| self.remap_binary_error(e, "mul", lhs, rhs))
    }

    pub fn __div__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("div", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("div", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        self.dispatch_magic(lhs, "__div__", &[rhs]).map_err(|e| self.remap_binary_error(e, "div", lhs, rhs))
    }

    pub fn __mod__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("mod", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("mod", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        self.dispatch_magic(lhs, "__mod__", &[rhs]).map_err(|e| self.remap_binary_error(e, "mod", lhs, rhs))
    }

    pub fn __floordiv__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("floordiv", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("floordiv", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        self.dispatch_magic(lhs, "__floordiv__", &[rhs]).map_err(|e| self.remap_binary_error(e, "floordiv", lhs, rhs))
    }

    // ================================================================================== //
    //           Comparison ops
    // ================================================================================== //

    pub fn __eq__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
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
            Err(RuntimeErrorKind::NoImplementMethod(_, _)) | Err(RuntimeErrorKind::UnexpectedType(_, _)) => {
                // Different types that don't implement __eq__ are not equal.
                Ok(self.obj_heap.alloc_bool_instance(false))
            }
            Err(other) => Err(other),
        }
    }

    pub fn __ne__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
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
            Err(RuntimeErrorKind::NoImplementMethod(_, _)) => {
                // Fallback: __eq__ + invert.
                let eq = self.__eq__(lhs, rhs)?;
                self.__not__(eq)
            }
            Err(other) => Err(other),
        }
    }

    pub fn __gt__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("gt", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("gt", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        self.dispatch_magic(lhs, "__gt__", &[rhs]).map_err(|e| self.remap_binary_error(e, "gt", lhs, rhs))
    }

    pub fn __ge__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("ge", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("ge", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        match self.dispatch_magic(lhs, "__ge__", &[rhs]) {
            Ok(result) => Ok(result),
            Err(RuntimeErrorKind::NoImplementMethod(_, _)) => {
                // Fallback: !(lhs < rhs).
                let lt = self.__lt__(lhs, rhs)?;
                self.__not__(lt)
            }
            Err(other) => Err(other),
        }
    }

    pub fn __lt__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("lt", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("lt", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        self.dispatch_magic(lhs, "__lt__", &[rhs]).map_err(|e| self.remap_binary_error(e, "lt", lhs, rhs))
    }

    pub fn __le__(&mut self, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(lhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("le", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        if !matches!(self.obj_heap.get(rhs), Object::Instance(_)) {
            return Err(RuntimeErrorKind::BinaryOpTypeMismatch("le", self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs)));
        }
        match self.dispatch_magic(lhs, "__le__", &[rhs]) {
            Ok(result) => Ok(result),
            Err(RuntimeErrorKind::NoImplementMethod(_, _)) => {
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

    pub fn __str__(&mut self, handle: ObjectHandle) -> RuntimeResult<ShrString> {
        // Singletons that don't have a regular class to dispatch to.
        if handle.is_nil() {
            return Ok("nil".into());
        }
        if handle.is_iter_end() {
            return Ok("IterEnd".into());
        }
        let object = self.obj_heap.get(handle);
        match object {
            Object::Instance(_) => {
                match self.dispatch_magic(handle, "__str__", &[]) {
                    Ok(result) => self
                        .obj_heap
                        .get_string_instance(result)
                        .cloned()
                        .ok_or_else(|| RuntimeErrorKind::BadStrResult(self.obj_heap.type_of(result)).into()),
                    Err(RuntimeErrorKind::NoImplementMethod(_, _)) => {
                        // Default representation for instances without __str__.
                        let class_handle = self.obj_heap.expect_instance(handle)?.class;
                        let class = self.obj_heap.expect_class(class_handle)?;
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
            Object::Module(module) => Ok(format_shr!("<module {}>", module.name)),
        }
    }

    pub fn __bool__(&mut self, handle: ObjectHandle) -> RuntimeResult<bool> {
        // nil is falsy; everything else (including IterEnd) is truthy.
        if handle.is_nil() {
            return Ok(false);
        }
        if handle.is_iter_end() {
            return Ok(true);
        }
        let object = self.obj_heap.get(handle);
        match object {
            Object::Instance(_) => match self.dispatch_magic(handle, "__bool__", &[]) {
                Ok(result) => Ok(*self.obj_heap.expect_bool(result)?),
                Err(RuntimeErrorKind::NoImplementMethod(_, _)) => Ok(true),
                Err(other) => Err(other),
            },
            _ => Ok(true),
        }
    }

    pub fn __len__(&mut self, handle: ObjectHandle) -> RuntimeResult<i64> {
        if !matches!(self.obj_heap.get(handle), Object::Instance(_)) {
            return Err(RuntimeErrorKind::UnexpectedType("object with __len__", self.obj_heap.type_of(handle)));
        }
        let result = self.dispatch_magic(handle, "__len__", &[]).map_err(|e| self.remap_len_error(e, handle))?;
        Ok(*self.obj_heap.expect_integer(result)?)
    }

    pub fn __getitem__(&mut self, collection: ObjectHandle, index: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(collection), Object::Instance(_)) {
            return Err(RuntimeErrorKind::UnexpectedType("object with __getitem__", self.obj_heap.type_of(collection)));
        }
        self.dispatch_magic(collection, "__getitem__", &[index])
    }

    pub fn __setitem__(&mut self, collection: ObjectHandle, index: ObjectHandle, value: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(collection), Object::Instance(_)) {
            return Err(RuntimeErrorKind::UnexpectedType("object with __setitem__", self.obj_heap.type_of(collection)));
        }
        self.dispatch_magic(collection, "__setitem__", &[index, value])
    }

    // -- property access --------------------------------------------------------

    /// Attribute read fallback — `obj.field` when the field is neither an
    /// instance field nor a regular method.
    pub fn __getattr__(&mut self, receiver: ObjectHandle, field_name: &ShrString) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(receiver), Object::Instance(_)) {
            return Err(RuntimeErrorKind::UnexpectedType("object with __getattr__", self.obj_heap.type_of(receiver)));
        }
        let name_handle = self.obj_heap.alloc_string_instance(field_name.clone());
        self.dispatch_magic(receiver, "__getattr__", &[name_handle])
    }

    /// Attribute write fallback — `obj.field = value` for instances that
    /// don't use ObjectFields.
    pub fn __setattr__(&mut self, receiver: ObjectHandle, field_name: &ShrString, value: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(receiver), Object::Instance(_)) {
            return Err(RuntimeErrorKind::UnexpectedType("object with __setattr__", self.obj_heap.type_of(receiver)));
        }
        let name_handle = self.obj_heap.alloc_string_instance(field_name.clone());
        self.dispatch_magic(receiver, "__setattr__", &[name_handle, value])
    }

    pub fn __int__(&mut self, handle: ObjectHandle) -> RuntimeResult<i64> {
        if !matches!(self.obj_heap.get(handle), Object::Instance(_)) {
            return Err(RuntimeErrorKind::UnexpectedType("object with __int__", self.obj_heap.type_of(handle)));
        }
        let result = self.dispatch_magic(handle, "__int__", &[]).map_err(|e| self.remap_int_error(e, handle))?;
        Ok(*self.obj_heap.expect_integer(result)?)
    }

    pub fn __float__(&mut self, handle: ObjectHandle) -> RuntimeResult<f64> {
        if !matches!(self.obj_heap.get(handle), Object::Instance(_)) {
            return Err(RuntimeErrorKind::UnexpectedType("object with __float__", self.obj_heap.type_of(handle)));
        }
        let result = self.dispatch_magic(handle, "__float__", &[]).map_err(|e| self.remap_float_error(e, handle))?;
        Ok(*self.obj_heap.expect_float(result)?)
    }

    // ================================================================================== //
    //           __hash__ — hashing protocol
    // ================================================================================== //

    /// Return a hash for `handle`, suitable for use as a bucket key in Dict/Set.
    ///
    /// Each built-in type registers its own `__hash__` via its class methods.
    /// Types that don't implement `__hash__` fall back to identity-based hashing
    /// (`handle.0`), which is correct for mutable types (List, Dict, Set) and
    /// for user-defined objects that don't override it.
    pub fn __hash__(&mut self, handle: ObjectHandle) -> RuntimeResult<u64> {
        // Sentinel handles without classes.
        if handle.is_nil() {
            return Ok(0);
        }
        if handle.is_iter_end() {
            return Ok(1);
        }
        // Non-Instance types (Class, Closure, NativeFn, etc.) — identity hash.
        if !matches!(self.obj_heap.get(handle), Object::Instance(_)) {
            return Ok(handle.0 as u64);
        }
        // Instance types — dispatch to __hash__ class method.
        match self.dispatch_magic(handle, "__hash__", &[]) {
            Ok(result) => {
                let h = self.obj_heap.expect_integer(result)?;
                Ok(*h as u64)
            }
            Err(RuntimeErrorKind::NoImplementMethod(_, _)) => Ok(handle.0 as u64),
            Err(other) => Err(other),
        }
    }

    // ================================================================================== //
    //           Error remapping helpers
    // ================================================================================== //

    fn remap_unary_error(&self, err: RuntimeErrorKind, op: &'static str, value: ObjectHandle) -> RuntimeErrorKind {
        match err {
            RuntimeErrorKind::NoImplementMethod(_, _) | RuntimeErrorKind::UnexpectedType(_, _) => {
                RuntimeErrorKind::UnaryOpTypeMismatch(op, self.obj_heap.type_of(value))
            }
            other => other,
        }
    }

    fn remap_binary_error(&self, err: RuntimeErrorKind, op: &'static str, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeErrorKind {
        match err {
            RuntimeErrorKind::NoImplementMethod(_, _) | RuntimeErrorKind::UnexpectedType(_, _) => {
                RuntimeErrorKind::BinaryOpTypeMismatch(op, self.obj_heap.type_of(lhs), self.obj_heap.type_of(rhs))
            }
            other => other,
        }
    }

    fn remap_len_error(&self, err: RuntimeErrorKind, handle: ObjectHandle) -> RuntimeErrorKind {
        match err {
            RuntimeErrorKind::UnexpectedType(_, _) => {
                RuntimeErrorKind::UnexpectedType("object with __len__", self.obj_heap.type_of(handle))
            }
            // NoImplementMethod already says which class is missing __len__ — keep it.
            other => other,
        }
    }

    fn remap_int_error(&self, err: RuntimeErrorKind, handle: ObjectHandle) -> RuntimeErrorKind {
        match err {
            RuntimeErrorKind::UnexpectedType(_, _) => {
                RuntimeErrorKind::UnexpectedType("object with __int__", self.obj_heap.type_of(handle))
            }
            // NoImplementMethod already says which class is missing __int__ — keep it.
            other => other,
        }
    }

    fn remap_float_error(&self, err: RuntimeErrorKind, handle: ObjectHandle) -> RuntimeErrorKind {
        match err {
            RuntimeErrorKind::UnexpectedType(_, _) => {
                RuntimeErrorKind::UnexpectedType("object with __float__", self.obj_heap.type_of(handle))
            }
            // NoImplementMethod already says which class is missing __float__ — keep it.
            other => other,
        }
    }

    // ================================================================================== //
    //           __iter__ / __next__ — iteration protocol
    // ================================================================================== //

    /// Call `__iter__` on `iterable`, returning an iterator object.
    pub fn __iter__(&mut self, iterable: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(iterable), Object::Instance(_)) {
            return Err(RuntimeErrorKind::UnexpectedType("iterable", self.obj_heap.type_of(iterable)));
        }
        self.dispatch_magic(iterable, "__iter__", &[])
    }

    /// Call `__next__` on `iterator`, returning the next element or
    /// `ObjectHandle::ITER_END` when exhausted.
    pub fn __next__(&mut self, iterator: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        if !matches!(self.obj_heap.get(iterator), Object::Instance(_)) {
            return Err(RuntimeErrorKind::UnexpectedType("iterator", self.obj_heap.type_of(iterator)));
        }
        self.dispatch_magic(iterator, "__next__", &[])
    }
}
