use super::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{Method, NativeFunction, Object, ObjectFields, ObjectHandle, ShrString};

impl VirtualMachine {
    // -----------------------------------------------------------------
    //  call_value / call_value_kw  —  unified call dispatch
    // -----------------------------------------------------------------

    pub(crate) fn call_value(&mut self, callee: ObjectHandle, arg_count: usize) -> RuntimeResult<()> {
        let obj = self.obj_heap.get(callee);
        match obj {
            Object::Closure(_) => self.call_closure(callee, arg_count),
            Object::Class(_) => {
                let (new_method, init_method) = {
                    let mut cur = callee;
                    let mut new_m = None;
                    let mut init_m = None;
                    loop {
                        let class = self.obj_heap.get_class(cur).expect("must class");
                        if new_m.is_none() {
                            new_m = class.methods.get("__new__").copied();
                        }
                        if init_m.is_none() {
                            init_m = class.methods.get("__init__").copied();
                        }
                        if new_m.is_some() && init_m.is_some() {
                            break;
                        }
                        match class.superclass {
                            Some(sc) => cur = sc,
                            None => break,
                        }
                    }
                    (new_m, init_m)
                };

                // ---- create the instance ------------------------------------
                // If __new__ is defined, call it with the class as the first
                // argument followed by any user-provided constructor arguments.
                // It returns the fully-constructed instance.  Otherwise create
                // a bare ObjectFields instance.
                let instance = match new_method {
                    Some(method) => {
                        // Save user arguments before __new__ consumes them.
                        let callee_idx = self.callee_slot(arg_count);
                        let saved_args: Vec<ObjectHandle> = (callee_idx + 1..self.stack.len()).map(|i| self.stack[i]).collect();

                        // Truncate stack to just before the class slot,
                        // then push class + saved args for __new__ dispatch.
                        self.stack.truncate(callee_idx);
                        self.stack.push(callee);
                        for &arg in &saved_args {
                            self.stack.push(arg);
                        }

                        // Call __new__ synchronously.
                        let saved_frame_count = self.frames.len();
                        let total = 1 + saved_args.len();
                        self.insert_and_call_method(&method, callee_idx, total)?;
                        while self.frames.len() > saved_frame_count {
                            self.step()?;
                        }

                        // Pop the instance from __new__ and push it back
                        // together with the user arguments so the stack
                        // layout matches what __init__ expects:
                        //   [..., instance, arg1, arg2, ...]
                        let inst = self.pop_stack()?;
                        self.stack.push(inst);
                        for &arg in &saved_args {
                            self.stack.push(arg);
                        }
                        inst
                    }
                    None => self.obj_heap.alloc_instance_dyn(callee, Box::new(ObjectFields::default())),
                };

                // Replace the callee (class) with the instance.
                let index = self.callee_slot(arg_count);
                self.stack[index] = instance;

                // ---- optionally initialise ----------------------------------
                if let Some(method) = init_method {
                    // Run __init__ synchronously — its return value is
                    // discarded below; __init__ is only a configurator.
                    let saved_frame_count = self.frames.len();
                    let total = arg_count + 1;
                    self.insert_and_call_method(&method, index, total)?;
                    while self.frames.len() > saved_frame_count {
                        self.step()?;
                    }
                    // __init__'s return value is irrelevant — pop it and
                    // restore the instance as the result of the call.
                    self.pop_stack()?;
                    self.push_stack(instance);
                    Ok(())
                } else if new_method.is_none() && arg_count != 0 {
                    // Neither __new__ nor __init__ defined, but arguments
                    // were passed — that is an error.
                    Err(RuntimeErrorKind::ArgumentCountMismatch { expected: 0, got: arg_count }.into())
                } else {
                    // No __init__.  If __new__ restored user arguments on
                    // the stack for __init__'s benefit, discard them now
                    // so only the instance remains.
                    if new_method.is_some() {
                        self.stack.truncate(index + 1);
                    }
                    Ok(())
                }
            }
            Object::Instance(_) => self.__call__(callee, arg_count).map_err(|e| match e {
                RuntimeErrorKind::NoImplementMethod(_, _) => RuntimeErrorKind::CanNotCall(self.obj_heap.type_of(callee)),
                other => other,
            }),
            Object::BoundMethod(bound_method) => {
                // Replace BoundMethod with [callee, receiver] so the frame
                // sees slot 0 = callee, slot 1 = receiver (= self).
                let index = self.callee_slot(arg_count);
                let receiver = bound_method.receiver;
                let method = bound_method.method; // copy to end the immutable borrow
                self.stack[index] = receiver;
                self.insert_and_call_method(&method, index, arg_count + 1)
            }
            Object::NativeFn(native_fn) => self.call_native_fn(native_fn.function, arg_count),
            _ => Err(RuntimeErrorKind::CanNotCall(self.obj_heap.type_of(callee))),
        }
    }

    /// Call a value with keyword arguments — reorder args to match parameter
    /// order and fill in defaults.
    pub(crate) fn call_value_kw(
        &mut self,
        callee: ObjectHandle,
        pos_count: usize,
        kw_count: usize,
        kw_names: &[ShrString],
    ) -> RuntimeResult<()> {
        let total_on_stack = pos_count + kw_count;
        let callee_idx = self.stack.len() - total_on_stack - 1;
        let obj = self.obj_heap.get(callee);

        match obj {
            Object::Class(_) => {
                // Class construction: get __init__ params, skipping `self`.
                let (param_names, arity, required_arity, defaults) = self.get_callable_info(callee)?;
                // param_names[0] is "self" — skip it for keyword matching.
                let user_param_names: Vec<ShrString> = param_names.iter().skip(1).cloned().collect();
                let user_arity = arity.saturating_sub(1);
                let _user_required = required_arity.saturating_sub(1);
                // defaults correspond to user params (not including self).
                let default_offset = user_arity.saturating_sub(defaults.len());

                let mut resolved: Vec<Option<ObjectHandle>> = vec![None; user_arity];

                for i in 0..pos_count.min(user_arity) {
                    resolved[i] = Some(self.stack[callee_idx + 1 + i]);
                }
                for (kw_idx, kw_name) in kw_names.iter().enumerate() {
                    let kw_val = self.stack[callee_idx + 1 + pos_count + kw_idx];
                    if let Some(param_idx) = user_param_names.iter().position(|n| n == kw_name) {
                        if resolved[param_idx].is_some() {
                            return Err(RuntimeErrorKind::DuplicateKeywordArg(kw_name.to_string()));
                        }
                        resolved[param_idx] = Some(kw_val);
                    } else {
                        return Err(RuntimeErrorKind::UnknownKeywordArg(kw_name.to_string()));
                    }
                }
                for i in 0..user_arity {
                    if resolved[i].is_none() {
                        if i >= default_offset {
                            resolved[i] = Some(defaults[i - default_offset]);
                        } else {
                            return Err(RuntimeErrorKind::MissingArgument(user_param_names[i].to_string()));
                        }
                    }
                }

                // Build new stack: callee + resolved args.
                self.stack.truncate(callee_idx);
                self.stack.push(callee);
                for arg_opt in &resolved {
                    self.stack.push(arg_opt.unwrap());
                }
                // call_value(Class) will create instance and call __init__.
                self.call_value(callee, user_arity)
            }
            _ => {
                // Closure, BoundMethod, etc.
                let (param_names, arity, _required_arity, defaults) = self.get_callable_info(callee)?;
                let default_offset = arity.saturating_sub(defaults.len());

                let mut resolved: Vec<Option<ObjectHandle>> = vec![None; arity];

                for i in 0..pos_count.min(arity) {
                    resolved[i] = Some(self.stack[callee_idx + 1 + i]);
                }
                for (kw_idx, kw_name) in kw_names.iter().enumerate() {
                    let kw_val = self.stack[callee_idx + 1 + pos_count + kw_idx];
                    if let Some(param_idx) = param_names.iter().position(|n| n == kw_name) {
                        if resolved[param_idx].is_some() {
                            return Err(RuntimeErrorKind::DuplicateKeywordArg(kw_name.to_string()));
                        }
                        resolved[param_idx] = Some(kw_val);
                    } else {
                        return Err(RuntimeErrorKind::UnknownKeywordArg(kw_name.to_string()));
                    }
                }
                for i in 0..arity {
                    if resolved[i].is_none() {
                        if i >= default_offset {
                            resolved[i] = Some(defaults[i - default_offset]);
                        } else {
                            return Err(RuntimeErrorKind::MissingArgument(param_names[i].to_string()));
                        }
                    }
                }

                self.stack.truncate(callee_idx);
                self.stack.push(callee);
                for arg_opt in &resolved {
                    self.stack.push(arg_opt.unwrap());
                }
                self.call_value(callee, arity)
            }
        }
    }

    /// Return (param_names, arity, required_arity, defaults) for a callable.
    fn get_callable_info(&self, callee: ObjectHandle) -> RuntimeResult<(Vec<ShrString>, usize, usize, Vec<ObjectHandle>)> {
        let obj = self.obj_heap.get(callee);
        match obj {
            Object::Closure(closure) => {
                let func = self.obj_heap.get_function(closure.function).expect("must function");
                Ok((func.param_names.clone(), func.arity, func.required_arity, func.defaults.clone()))
            }
            Object::BoundMethod(bound) => {
                let handle = match bound.method {
                    Method::User(h) => h,
                    Method::Native(_) => {
                        // Native methods don't support keyword args yet.
                        return Err(RuntimeErrorKind::UnsupportedMethodCall("keyword", "native method"));
                    }
                };
                let closure = self.obj_heap.get_closure(handle).expect("must closure");
                let func = self.obj_heap.get_function(closure.function).expect("must function");
                Ok((func.param_names.clone(), func.arity, func.required_arity, func.defaults.clone()))
            }
            Object::Class(class) => {
                // For class construction, look up __init__.
                if let Some(Method::User(init_handle)) = class.methods.get("__init__").copied() {
                    let closure = self.obj_heap.get_closure(init_handle).expect("must closure");
                    let func = self.obj_heap.get_function(closure.function).expect("must function");
                    Ok((func.param_names.clone(), func.arity, func.required_arity, func.defaults.clone()))
                } else {
                    // No init — no params.
                    Ok((vec![], 0, 0, vec![]))
                }
            }
            _ => Err(RuntimeErrorKind::CanNotCall(self.obj_heap.type_of(callee))),
        }
    }

    // -----------------------------------------------------------------
    //  call_closure / call_native_fn  —  low-level frame entry
    // -----------------------------------------------------------------

    /// Insert the callee for `method` at `insert_pos` on the stack, then
    /// call it with `arg_count` arguments already sitting to the right of
    /// the insertion point.
    ///
    /// After insertion the stack layout is
    /// `[..., callee, arg_0, ..., arg_{n-1}]` — matching what
    /// [`call_closure`] / [`call_native_fn`] expect.
    pub(crate) fn insert_and_call_method(&mut self, method: &Method, insert_pos: usize, arg_count: usize) -> RuntimeResult<()> {
        match method {
            Method::User(closure_handle) => {
                let h = *closure_handle;
                self.stack.insert(insert_pos, h);
                self.call_closure(h, arg_count)
            }
            Method::Native(handle) => {
                let h = *handle;
                let native_fn = self.obj_heap.get_native_fn(h).expect("must fn").function;
                self.stack.insert(insert_pos, h);
                self.call_native_fn(native_fn, arg_count)
            }
        }
    }

    /// Like [`insert_and_call_method`], but for synchronous dispatch:
    /// runs User closures to completion via the step loop and returns
    /// the result.  Native methods return immediately.
    pub(crate) fn insert_and_call_method_sync(
        &mut self,
        method: &Method,
        insert_pos: usize,
        arg_count: usize,
    ) -> RuntimeResult<ObjectHandle> {
        match *method {
            Method::User(h) => {
                let saved_frame_count = self.frames.len();
                self.stack.insert(insert_pos, h);
                self.call_closure(h, arg_count)?;
                while self.frames.len() > saved_frame_count {
                    self.step()?;
                }
                self.pop_stack()
            }
            Method::Native(h) => {
                let native_fn = self.obj_heap.get_native_fn(h).expect("must fn").function;
                self.stack.insert(insert_pos, h);
                self.call_native_fn(native_fn, arg_count)?;
                self.pop_stack()
            }
        }
    }

    /// Push a call frame for a user-defined closure.
    ///
    /// The stack has the closure at slot 0 followed by `arg_count` arguments.
    /// `arg_count` does NOT include the closure itself.
    pub(crate) fn call_closure(&mut self, closure: ObjectHandle, arg_count: usize) -> RuntimeResult<()> {
        let closure_obj = self.obj_heap.get_closure(closure).expect("must closure");
        let function = self.obj_heap.get_function(closure_obj.function).expect("must function");
        // Allow arg_count in [required_arity, arity]; fill defaults for missing args.
        if arg_count < function.required_arity || arg_count > function.arity {
            Err(RuntimeErrorKind::ArgumentCountRange { min: function.required_arity, max: function.arity, got: arg_count })?;
        }
        let slots_start = self.stack.len() - arg_count - 1;

        // Fill in default values for missing arguments.
        let num_missing = function.arity - arg_count;
        if num_missing > 0 {
            let default_start = function.arity - function.defaults.len();
            // Push defaults for missing trailing parameters.
            for i in (function.arity - num_missing)..function.arity {
                let default_idx = i - default_start;
                let default_val = function.defaults[default_idx];
                self.stack.push(default_val);
            }
        }

        self.frames.push(crate::vm::CallFrame { closure, ip: 0, slots_start });
        Ok(())
    }

    /// Call a native function.
    ///
    /// The callee sits on the stack at slot 0; it is popped before the native
    /// function reads its arguments.  `arg_count` does NOT include the callee.
    pub(crate) fn call_native_fn(&mut self, native_func: NativeFunction, arg_count: usize) -> RuntimeResult<()> {
        let callee_idx = self.stack.len() - arg_count - 1;
        self.stack.remove(callee_idx);
        let actual_args = arg_count;
        let result = match native_func {
            NativeFunction::Arity0(f) => {
                self.get_0_args(actual_args)?;
                f(self)?
            }
            NativeFunction::Arity1(f) => {
                let a0 = self.get_1_args(actual_args)?;
                f(self, a0)?
            }
            NativeFunction::Arity2(f) => {
                let (a0, a1) = self.get_2_args(actual_args)?;
                f(self, a0, a1)?
            }
            NativeFunction::Arity3(f) => {
                let (a0, a1, a2) = self.get_3_args(actual_args)?;
                f(self, a0, a1, a2)?
            }
            NativeFunction::Arity4(f) => {
                let (a0, a1, a2, a3) = self.get_4_args(actual_args)?;
                f(self, a0, a1, a2, a3)?
            }
            NativeFunction::Arity5(f) => {
                let (a0, a1, a2, a3, a4) = self.get_5_args(actual_args)?;
                f(self, a0, a1, a2, a3, a4)?
            }
            NativeFunction::Variadic(f) => {
                let args = self.get_args(actual_args).to_vec();
                f(self, &args)?
            }
        };
        self.stack.truncate(self.stack.len() - actual_args);
        self.push_stack(result);
        Ok(())
    }

    /// Bind a closure as a method on a class (used by the `method` instruction).
    pub(crate) fn define_method(&mut self, name: ShrString) -> RuntimeResult<()> {
        let method_handle = self.peek_stack(0)?;
        let class_handle = self.peek_stack(1)?;
        let class = self.obj_heap.get_class_mut(class_handle).expect("must class");
        class.methods.insert(name, Method::User(method_handle));
        self.pop_stack()?;
        Ok(())
    }
}
