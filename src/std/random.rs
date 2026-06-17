use std::collections::HashMap;
use crate::{NativeFunction, ObjectHandle, ObjectInstanceData, ShrString};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

impl VirtualMachine {
    /// Create the `random` std module.
    pub(crate) fn create_random_module(&mut self) -> ExecuteResult<ObjectHandle> {
        // ---- function handles ----
        let random_fn   = self.obj_heap.alloc_native_fn("random",  NativeFunction::a0(random));
        let randint_fn  = self.obj_heap.alloc_native_fn("randint", NativeFunction::a2(randint));
        let uniform_fn  = self.obj_heap.alloc_native_fn("uniform", NativeFunction::a2(uniform));
        let choice_fn   = self.obj_heap.alloc_native_fn("choice",  NativeFunction::a1(choice));
        let shuffle_fn  = self.obj_heap.alloc_native_fn("shuffle", NativeFunction::a1(shuffle));

        // ---- assemble module ----
        let mut exports: HashMap<ShrString, ObjectHandle> = HashMap::new();
        exports.insert(ShrString::new_str("random"),  random_fn);
        exports.insert(ShrString::new_str("randint"), randint_fn);
        exports.insert(ShrString::new_str("uniform"), uniform_fn);
        exports.insert(ShrString::new_str("choice"),  choice_fn);
        exports.insert(ShrString::new_str("shuffle"), shuffle_fn);

        let module = self.obj_heap.alloc_fields_instance(self.obj_heap.module_class);
        if let Some(inst) = self.obj_heap.get_instance_mut(module) {
            if let ObjectInstanceData::Fields(fields) = &mut inst.data {
                *fields = exports;
            }
        }

        Ok(module)
    }
}

fn random(vm: &mut VirtualMachine) -> ExecuteResult<ObjectHandle> {
    let v = rand::random();
    Ok(vm.obj_heap.alloc_float_instance(v))
}

fn randint(vm: &mut VirtualMachine, min: ObjectHandle, max: ObjectHandle) -> ExecuteResult<ObjectHandle> {
    let minv = as_i64(vm, min, "randint")?;
    let maxv = as_i64(vm, max, "randint")?;
    if minv > maxv {
        return Err(ExecuteError::RandomError(
            format!("randint: min ({minv}) must be <= max ({maxv})")
        ));
    }
    let v = rand::random_range(minv..=maxv);
    Ok(vm.obj_heap.alloc_integer_instance(v))
}

fn uniform(vm: &mut VirtualMachine, min: ObjectHandle, max: ObjectHandle) -> ExecuteResult<ObjectHandle> {
    let minv = as_f64(vm, min, "uniform")?;
    let maxv = as_f64(vm, max, "uniform")?;
    if minv > maxv {
        return Err(ExecuteError::RandomError(
            format!("uniform: min ({minv}) must be <= max ({maxv})")
        ));
    }
    let v = rand::random_range(minv..maxv);
    Ok(vm.obj_heap.alloc_float_instance(v))
}

fn choice(vm: &mut VirtualMachine, seq: ObjectHandle) -> ExecuteResult<ObjectHandle> {
    // Extract length first to avoid holding the immutable borrow across
    // the mutable borrow on `vm.rng`.
    let len = {
        let list = vm.get_list_instance(seq).map_err(|_| {
            ExecuteError::BinaryOpTypeMismatch("choice", "list", vm.value_type_name(seq))
        })?;
        if list.is_empty() {
            return Err(ExecuteError::RandomError("choice: list is empty".into()));
        }
        list.len()
    };
    let idx = rand::random_range(0..len);
    Ok(vm.get_list_instance(seq).unwrap()[idx])
}

fn shuffle(vm: &mut VirtualMachine, seq: ObjectHandle) -> ExecuteResult<ObjectHandle> {
    // Pre-generate swap indices to avoid overlapping borrows.
    let n = {
        let list = vm.get_list_instance(seq).map_err(|_| {
            ExecuteError::BinaryOpTypeMismatch("shuffle", "list", vm.value_type_name(seq))
        })?;
        list.len()
    };
    // Fisher-Yates shuffle: for each i, pick j in [i, n).
    let swaps: Vec<(usize, usize)> = (0..n)
        .map(|i| (i, rand::random_range(i..n)))
        .collect();
    let list = vm.get_list_instance_mut(seq).expect("just checked");
    for (i, j) in swaps {
        list.swap(i, j);
    }
    Ok(seq)
}

/// Extract an `f64` from a numeric handle (int or float).
fn as_f64(vm: &VirtualMachine, handle: ObjectHandle, fn_name: &'static str) -> ExecuteResult<f64> {
    if let Ok(v) = vm.get_integer_instance(handle) {
        Ok(*v as f64)
    } else if let Ok(v) = vm.get_float_instance(handle) {
        Ok(*v)
    } else {
        Err(ExecuteError::BinaryOpTypeMismatch(fn_name, "float", vm.value_type_name(handle)))
    }
}

/// Extract an `f64` from a numeric handle (int or float).
fn as_i64(vm: &VirtualMachine, handle: ObjectHandle, fn_name: &'static str) -> ExecuteResult<i64> {
    if let Ok(v) = vm.get_integer_instance(handle) {
        Ok(*v)
    } else {
        Err(ExecuteError::BinaryOpTypeMismatch(fn_name, "i64", vm.value_type_name(handle)))
    }
}