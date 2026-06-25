use super::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{Instruction, Method, Object, ObjectFields, ObjectHandle};
use std::collections::HashMap;

macro_rules! binary_op {
    ($vm:ident, $f:ident) => {
        paste::paste! {{
            let rhs = $vm.pop_stack()?;
            let lhs = $vm.pop_stack()?;
            let res = $vm.[<__ $f __>](lhs, rhs)?;
            $vm.push_stack(res);
        }}
    };
}

macro_rules! unary_op {
    ($vm:ident, $f:ident) => {
        paste::paste! {{
            let v = $vm.pop_stack()?;
            let res = $vm.[<__ $f __>](v)?;
            $vm.push_stack(res);
        }}
    };
}

impl VirtualMachine {
    /// Advance the VM by one instruction.
    pub(crate) fn step(&mut self) -> RuntimeResult<()> {
        let mut ip = self.frame()?.ip;

        let closure = self.obj_heap.get_closure(self.frame()?.closure).expect("must closure");
        let module = self.current_module();

        let inst = {
            let function = self.obj_heap.get_function(closure.function).expect("must function");
            function.chunk.read_instruction(&mut ip, &self.obj_heap)?
        };

        match inst {
            Instruction::Constant(handle) => self.push_stack(handle),
            Instruction::DefineGlobal(name) => {
                let value = self.pop_stack()?;
                self.obj_heap.get_module_mut(module).expect("module").fields.insert(name, value);
            }
            Instruction::GetGlobal(name) => {
                let value = self
                    .obj_heap
                    .get_module(module)
                    .and_then(|m| m.fields.get(&name).copied())
                    .or_else(|| self.builtins.get(&name).copied())
                    .ok_or_else(|| RuntimeErrorKind::VariableNotFound(name.as_str().to_string()))?;
                self.push_stack(value);
            }
            Instruction::SetGlobal(name) => {
                let value = self.stack.last().copied().ok_or(RuntimeErrorKind::StackEmpty)?;
                self.obj_heap.get_module_mut(module).expect("module").fields.insert(name, value);
            }
            Instruction::GetLocal(slot) => {
                let base = self.frame()?.slots_start;
                let index = base + slot;
                let value = self.stack.get(index).copied().ok_or_else(|| RuntimeErrorKind::StackIndexOutOfRange(index))?;
                self.push_stack(value);
            }
            Instruction::SetLocal(slot) => {
                let base = self.frame()?.slots_start;
                let index = base + slot;
                let value = self.stack.last().copied().ok_or(RuntimeErrorKind::StackEmpty)?;
                if index >= self.stack.len() {
                    return Err(RuntimeErrorKind::StackIndexOutOfRange(index));
                }
                self.stack[index] = value;
            }
            Instruction::Return => {
                let frame = self.frames.pop().expect("not empty frame");
                let result = self.pop_stack()?;
                self.close_upvalues(frame.slots_start)?;
                if self.frames.is_empty() {
                    // Last frame — keep only the result on the stack.
                    self.stack.truncate(0);
                    self.push_stack(result);
                    return Ok(());
                }
                self.stack.truncate(frame.slots_start);
                self.push_stack(result);
                return Ok(());
            }
            Instruction::Nil => self.push_stack(ObjectHandle::NIL),
            Instruction::True => {
                let h = self.obj_heap.alloc_bool_instance(true);
                self.push_stack(h);
            }
            Instruction::False => {
                let h = self.obj_heap.alloc_bool_instance(false);
                self.push_stack(h);
            }
            Instruction::Negate => unary_op!(self, neg),
            Instruction::Not => unary_op!(self, not),
            Instruction::Add => binary_op!(self, add),
            Instruction::Sub => binary_op!(self, sub),
            Instruction::Mul => binary_op!(self, mul),
            Instruction::Div => binary_op!(self, div),
            Instruction::Mod => binary_op!(self, mod),
            Instruction::FloorDiv => binary_op!(self, floordiv),
            Instruction::Equal => binary_op!(self, eq),
            Instruction::NotEqual => binary_op!(self, ne),
            Instruction::Greater => binary_op!(self, gt),
            Instruction::GreaterEqual => binary_op!(self, ge),
            Instruction::Less => binary_op!(self, lt),
            Instruction::LessEqual => binary_op!(self, le),
            Instruction::Pop => {
                self.pop_stack()?;
            }
            Instruction::JumpIfFalse(offset) => {
                let value = self.peek_stack(0)?;
                if !self.__bool__(value)? {
                    ip += offset;
                }
            }
            Instruction::Jump(offset) => {
                ip += offset;
            }
            Instruction::Loop(offset) => {
                ip -= offset;
            }

            Instruction::Call(arg_count) => {
                self.frame_mut()?.ip = ip;
                let callee = self.peek_stack(arg_count)?;
                self.call_value(callee, arg_count)?;
                return Ok(());
            }

            Instruction::CallKw { pos_count, kw_count, ref kw_names } => {
                self.frame_mut()?.ip = ip;
                let total_on_stack = pos_count + kw_count;
                let callee = self.peek_stack(total_on_stack)?;
                self.call_value_kw(callee, pos_count, kw_count, kw_names)?;
                return Ok(());
            }

            Instruction::Closure { function, upvalues } => {
                let closure_handle = self.obj_heap.alloc_closure(function, module);
                for uv_desc in upvalues {
                    let upvalue = if uv_desc.is_local {
                        let slot = self.frame()?.slots_start + uv_desc.index;
                        self.capture_upvalue(slot)?
                    } else {
                        let enclosing_closure = self.obj_heap.get_closure(self.frame()?.closure).expect("must closure");
                        enclosing_closure.upvalues[uv_desc.index]
                    };
                    self.obj_heap.get_closure_mut(closure_handle).expect("must closure").upvalues.push(upvalue);
                }
                self.push_stack(closure_handle);
            }

            Instruction::GetUpvalue(slot) => {
                let closure_handle = self.frame()?.closure;
                let closure = self.obj_heap.get_closure(closure_handle).expect("must closure");
                let upvalue_handle = closure.upvalues[slot];
                let upvalue = self.obj_heap.get_upvalue(upvalue_handle).expect("must upvalue");
                let value = match upvalue.location {
                    Some(stack_slot) => self.stack[stack_slot],
                    None => upvalue.closed,
                };
                self.push_stack(value);
            }

            Instruction::SetUpvalue(slot) => {
                let closure_handle = self.frame()?.closure;
                let closure = self.obj_heap.get_closure(closure_handle).expect("must closure");
                let upvalue_handle = closure.upvalues[slot];
                let upvalue = self.obj_heap.get_upvalue(upvalue_handle).expect("must upvalue");
                let value = self.peek_stack(0)?;
                match upvalue.location {
                    Some(stack_slot) => self.stack[stack_slot] = value,
                    None => {
                        let uv = self.obj_heap.get_upvalue_mut(upvalue_handle).expect("must upvalue");
                        uv.closed = value;
                    }
                }
            }

            Instruction::CloseUpvalue => {
                let top_slot = self.stack.len() - 1;
                self.close_upvalues(top_slot)?;
                self.pop_stack()?;
            }

            Instruction::Class(class_name) => {
                let class = self.obj_heap.alloc_class(class_name, module);
                self.push_stack(class);
            }

            // ---- GetProperty — unified dispatch ----
            Instruction::GetProperty(field_name) => {
                let receiver = self.peek_stack(0)?;

                let obj = self.obj_heap.get(receiver);
                match obj {
                    Object::Instance(instance) => {
                        if let Some(fields_data) = instance.data.as_any_ref().downcast_ref::<ObjectFields>()
                            && let Some(value) = fields_data.fields.get(&field_name).cloned()
                        {
                            self.pop_stack()?;
                            self.push_stack(value);
                        } else {
                            // Try a regular method first.
                            let class = self.obj_heap.get_class(instance.class).expect("must class");
                            let method = class.methods.get(&field_name).copied();
                            if let Some(m) = method {
                                let receiver = self.pop_stack()?;
                                let bound = self.obj_heap.alloc_bound_method(receiver, m);
                                self.push_stack(bound);
                            } else if let Some(_magic) = class.methods.get("__getattr__").copied() {
                                // Fall back to __getattr__(self, field_name).
                                let receiver = self.pop_stack()?;
                                let result = self.__getattr__(receiver, &field_name)?;
                                self.push_stack(result);
                            } else {
                                return Err(RuntimeErrorKind::UndefinedProperty(field_name.to_string()));
                            }
                        }
                    }
                    Object::Module(module) => {
                        // Module field access — fields take priority over
                        // any implicit class-like behaviour.
                        if let Some(&value) = module.fields.get(&field_name) {
                            self.pop_stack()?; // discard the module handle
                            self.push_stack(value);
                        } else {
                            return Err(RuntimeErrorKind::UndefinedProperty(field_name.to_string()));
                        }
                    }
                    Object::Class(class) => {
                        if let Some(method) = class.methods.get(&field_name).copied() {
                            self.pop_stack()?; // discard the class
                            match method {
                                Method::User(closure_handle) => {
                                    self.push_stack(closure_handle);
                                }
                                Method::Native(handle) => {
                                    // Unbound: push the NativeFn object directly.
                                    self.push_stack(handle);
                                }
                            }
                            self.frame_mut()?.ip = ip;
                            return Ok(());
                        }
                        return Err(RuntimeErrorKind::UndefinedProperty(field_name.to_string()));
                    }
                    _ => Err(RuntimeErrorKind::UndefinedProperty(field_name.to_string()))?,
                }
            }

            Instruction::SetProperty(field_name) => {
                let value = self.peek_stack(0)?;
                let instance_handle = self.peek_stack(1)?;

                // Module field assignment.
                if let Some(module) = self.obj_heap.get_module_mut(instance_handle) {
                    module.fields.insert(field_name, value);
                    let value = self.pop_stack()?;
                    self.pop_stack()?;
                    self.push_stack(value);
                } else if let Some(instance) = self.obj_heap.get_instance_mut(instance_handle) {
                    if let Some(fields) = instance.get_data_mut::<ObjectFields>() {
                        fields.fields.insert(field_name, value);
                        let value = self.pop_stack()?;
                        self.pop_stack()?;
                        self.push_stack(value);
                    } else {
                        // Not ObjectFields — try __setattr__ magic method.
                        let _ = instance; // release borrow before __setattr__
                        self.__setattr__(instance_handle, &field_name, value)?;
                        let value = self.pop_stack()?;
                        self.pop_stack()?;
                        self.push_stack(value);
                    }
                }
            }

            Instruction::Inherit => {
                let superclass = self.peek_stack(0)?;
                let subclass = self.peek_stack(1)?;
                let super_methods = {
                    let sc = self.obj_heap.get_class(superclass).expect("must class");
                    sc.methods.clone()
                };
                let sub = self.obj_heap.get_class_mut(subclass).expect("must class");
                sub.superclass = Some(superclass);
                for (name, method) in super_methods {
                    sub.methods.entry(name).or_insert(method);
                }
                self.pop_stack()?;
            }

            Instruction::Method(method_name) => {
                self.define_method(method_name)?;
            }

            // ---- Invoke — unified dispatch ----
            Instruction::Invoke(method_name, arg_count) => {
                let receiver = self.peek_stack(arg_count)?;

                // First, check if the receiver is an Instance/Module with a
                // field named `method_name`.  If so, try to call it directly.
                let field_value = match self.obj_heap.get(receiver) {
                    Object::Instance(inst) => {
                        inst.data.as_any_ref().downcast_ref::<ObjectFields>().and_then(|f| f.fields.get(&method_name).copied())
                    }
                    Object::Module(module) => module.fields.get(&method_name).copied(),
                    _ => None,
                };

                if let Some(handle) = field_value {
                    // Replace the receiver slot with the field value and call it.
                    let index = self.callee_slot(arg_count);
                    self.stack[index] = handle;
                    self.frame_mut()?.ip = ip;
                    self.call_value(handle, arg_count)?;
                    return Ok(());
                }

                // Not a field — fall back to class method lookup.
                let class_handle = self.obj_heap.get_instance(receiver).expect("must instance").class;

                let method = {
                    let class_ = self.obj_heap.get_class(class_handle).expect("must class");
                    class_
                        .methods
                        .get(&method_name)
                        .copied()
                        .ok_or_else(|| RuntimeErrorKind::UndefinedProperty(method_name.as_str().to_string()))?
                };

                match method {
                    Method::User(closure_handle) => {
                        self.frame_mut()?.ip = ip;
                        self.call_closure(closure_handle, arg_count + 1, false)?;
                        return Ok(());
                    }
                    Method::Native(handle) => {
                        let native_fn = self.obj_heap.get_native_fn(handle).expect("must fn").function;
                        self.call_native_fn(native_fn, arg_count + 1, false)?;
                    }
                }
            }

            Instruction::SuperInvoke(method_name, arg_count) => {
                let method = {
                    let receiver = self.peek_stack(arg_count)?;
                    let instance = self.obj_heap.get_instance(receiver).expect("must instance");
                    let class = self.obj_heap.get_class(instance.class).expect("must class");
                    let superclass_handle = class.superclass.ok_or(RuntimeErrorKind::NoSuperclass)?;
                    let superclass = self.obj_heap.get_class(superclass_handle).expect("must class");
                    superclass
                        .methods
                        .get(&method_name)
                        .copied()
                        .ok_or_else(|| RuntimeErrorKind::UndefinedProperty(method_name.as_str().to_string()))?
                };

                match method {
                    Method::User(closure_handle) => {
                        self.frame_mut()?.ip = ip;
                        self.call_closure(closure_handle, arg_count + 1, false)?;
                        return Ok(());
                    }
                    Method::Native(handle) => {
                        let native_fn = self.obj_heap.get_native_fn(handle).expect("must fn").function;
                        self.call_native_fn(native_fn, arg_count + 1, false)?;
                    }
                }
            }

            Instruction::BuildList(count) => {
                let mut items = vec![];
                for _ in 0..count {
                    items.push(self.pop_stack()?);
                }
                items.reverse();
                let list = self.obj_heap.alloc_list_instance(items);
                self.push_stack(list);
            }
            Instruction::BuildDict(count) => {
                let mut map: HashMap<u64, Vec<(ObjectHandle, ObjectHandle)>> = HashMap::new();
                for _ in 0..count {
                    let val = self.pop_stack()?;
                    let key = self.pop_stack()?;
                    let hash = self.__hash__(key)?;
                    map.entry(hash).or_default().push((key, val));
                }
                let dict = self.obj_heap.alloc_dict_instance(map);
                self.push_stack(dict);
            }
            Instruction::BuildSet(count) => {
                let mut map: HashMap<u64, Vec<ObjectHandle>> = HashMap::new();
                for _ in 0..count {
                    let item = self.pop_stack()?;
                    let hash = self.__hash__(item)?;
                    map.entry(hash).or_default().push(item);
                }
                let set = self.obj_heap.alloc_set_instance(map);
                self.push_stack(set);
            }
            Instruction::IndexGet => {
                let index = self.pop_stack()?;
                let collection = self.pop_stack()?;
                let result = self.__getitem__(collection, index)?;
                self.push_stack(result);
            }
            Instruction::IndexSet => {
                let value = self.pop_stack()?;
                let index = self.pop_stack()?;
                let collection = self.pop_stack()?;
                let result = self.__setitem__(collection, index, value)?;
                self.push_stack(result);
            }

            Instruction::Import(file_path) => {
                self.frame_mut()?.ip = ip;
                let module = self.import_module(file_path.as_str())?;
                self.push_stack(module);
                return Ok(());
            }

            // ---- iteration ----
            Instruction::IterEnd => {
                self.push_stack(ObjectHandle::ITER_END);
            }

            Instruction::ForInIter => {
                let iterable = self.pop_stack()?;
                let iterator = self.__iter__(iterable)?;
                self.push_stack(iterator);
            }

            Instruction::ForInNext(offset) => {
                let iterator = self.peek_stack(0)?;
                let result = self.__next__(iterator)?;
                // dispatch_magic internally pushes/pops receiver; the iterator
                // on the stack (pushed by GetLocal) is untouched.
                if result.is_iter_end() {
                    ip += offset as usize;
                } else {
                    self.push_stack(result);
                }
            }
        }

        self.frame_mut()?.ip = ip;
        Ok(())
    }
}
