use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{NativeFunction, ObjectHandle, ShrString};
use std::collections::HashMap;

impl VirtualMachine {
    /// Create the `random` std module.
    pub(crate) fn create_random_module(&mut self) -> RuntimeResult<ObjectHandle> {
        // ---- function handles ----
        let random_fn = self.obj_heap.alloc_native_fn("random", NativeFunction::a0(random));
        let randint_fn = self.obj_heap.alloc_native_fn("randint", NativeFunction::a2(randint));
        let uniform_fn = self.obj_heap.alloc_native_fn("uniform", NativeFunction::a2(uniform));
        let choice_fn = self.obj_heap.alloc_native_fn("choice", NativeFunction::a1(choice));
        let shuffle_fn = self.obj_heap.alloc_native_fn("shuffle", NativeFunction::a1(shuffle));

        // ---- assemble module ----
        let mut exports: HashMap<ShrString, ObjectHandle> = HashMap::new();
        exports.insert(ShrString::new_str("random"), random_fn);
        exports.insert(ShrString::new_str("randint"), randint_fn);
        exports.insert(ShrString::new_str("uniform"), uniform_fn);
        exports.insert(ShrString::new_str("choice"), choice_fn);
        exports.insert(ShrString::new_str("shuffle"), shuffle_fn);

        let module = self.obj_heap.alloc_fields_instance(self.obj_heap.module_class, exports);
        Ok(module)
    }
}

fn random(vm: &mut VirtualMachine) -> RuntimeResult<ObjectHandle> {
    let v = rand::random();
    Ok(vm.obj_heap.alloc_float_instance(v))
}

fn randint(vm: &mut VirtualMachine, min: ObjectHandle, max: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let minv = as_i64(vm, min, "randint")?;
    let maxv = as_i64(vm, max, "randint")?;
    if minv > maxv {
        return Err(RuntimeErrorKind::RandomError(format!("randint: min ({minv}) must be <= max ({maxv})")));
    }
    let v = rand::random_range(minv..=maxv);
    Ok(vm.obj_heap.alloc_integer_instance(v))
}

fn uniform(vm: &mut VirtualMachine, min: ObjectHandle, max: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let minv = as_f64(vm, min, "uniform")?;
    let maxv = as_f64(vm, max, "uniform")?;
    if minv > maxv {
        return Err(RuntimeErrorKind::RandomError(format!("uniform: min ({minv}) must be <= max ({maxv})")));
    }
    let v = rand::random_range(minv..maxv);
    Ok(vm.obj_heap.alloc_float_instance(v))
}

fn choice(vm: &mut VirtualMachine, seq: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    // Extract length first to avoid holding the immutable borrow across
    // the mutable borrow on `vm.rng`.
    let len = {
        let list = vm
            .obj_heap
            .get_list_instance(seq)
            .ok_or_else(|| RuntimeErrorKind::BinaryOpTypeMismatch("choice", "list", vm.obj_heap.type_of(seq)))?;
        if list.is_empty() {
            return Err(RuntimeErrorKind::RandomError("choice: list is empty".into()));
        }
        list.len()
    };
    let idx = rand::random_range(0..len);
    Ok(vm.obj_heap.get_list_instance(seq).unwrap()[idx])
}

fn shuffle(vm: &mut VirtualMachine, seq: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    // Pre-generate swap indices to avoid overlapping borrows.
    let n = {
        let list = vm
            .obj_heap
            .get_list_instance(seq)
            .ok_or_else(|| RuntimeErrorKind::BinaryOpTypeMismatch("shuffle", "list", vm.obj_heap.type_of(seq)))?;
        list.len()
    };
    // Fisher-Yates shuffle: for each i, pick j in [i, n).
    let swaps: Vec<(usize, usize)> = (0..n).map(|i| (i, rand::random_range(i..n))).collect();
    let list = vm.obj_heap.get_list_instance_mut(seq).expect("just checked");
    for (i, j) in swaps {
        list.swap(i, j);
    }
    Ok(seq)
}

/// Extract an `f64` from a numeric handle (int or float).
fn as_f64(vm: &VirtualMachine, handle: ObjectHandle, fn_name: &'static str) -> RuntimeResult<f64> {
    if let Some(v) = vm.obj_heap.get_integer_instance(handle) {
        Ok(*v as f64)
    } else if let Some(v) = vm.obj_heap.get_float_instance(handle) {
        Ok(*v)
    } else {
        Err(RuntimeErrorKind::BinaryOpTypeMismatch(fn_name, "float", vm.obj_heap.type_of(handle)))
    }
}

/// Extract an `f64` from a numeric handle (int or float).
fn as_i64(vm: &VirtualMachine, handle: ObjectHandle, fn_name: &'static str) -> RuntimeResult<i64> {
    if let Some(v) = vm.obj_heap.get_integer_instance(handle) {
        Ok(*v)
    } else {
        Err(RuntimeErrorKind::BinaryOpTypeMismatch(fn_name, "i64", vm.obj_heap.type_of(handle)))
    }
}
