use std::collections::HashMap;
use crate::{NativeFunction, ObjectHandle, ObjectInstanceData, ShrString};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

impl VirtualMachine {
    /// Extract an `i64` from a numeric handle, rejecting floats.
    fn random_as_i64(&self, handle: ObjectHandle, fn_name: &'static str) -> ExecuteResult<i64> {
        self.get_integer_instance(handle)
            .copied()
            .map_err(|_| ExecuteError::BinaryOpTypeMismatch(fn_name, "int", self.value_type_name(handle)))
    }

    /// Extract an `f64` from a numeric handle (int or float).
    fn random_as_f64(&self, handle: ObjectHandle, fn_name: &'static str) -> ExecuteResult<f64> {
        if let Ok(v) = self.get_integer_instance(handle) {
            Ok(*v as f64)
        } else if let Ok(v) = self.get_float_instance(handle) {
            Ok(*v)
        } else {
            Err(ExecuteError::BinaryOpTypeMismatch(fn_name, "float", self.value_type_name(handle)))
        }
    }

    /// Create the `random` std module.
    pub(super) fn create_random_module(&mut self) -> ExecuteResult<ObjectHandle> {
        // ---- function handles ----
        let random_fn   = self.obj_heap.alloc_native_fn("random",  NativeFunction::a0(VirtualMachine::random_random));
        let randint_fn  = self.obj_heap.alloc_native_fn("randint", NativeFunction::a2(VirtualMachine::random_randint));
        let uniform_fn  = self.obj_heap.alloc_native_fn("uniform", NativeFunction::a2(VirtualMachine::random_uniform));
        let choice_fn   = self.obj_heap.alloc_native_fn("choice",  NativeFunction::a1(VirtualMachine::random_choice));
        let shuffle_fn  = self.obj_heap.alloc_native_fn("shuffle", NativeFunction::a1(VirtualMachine::random_shuffle));

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

    // =====================================================================
    //  random() — random float in [0, 1)
    // =====================================================================

    fn random_random(&mut self) -> ExecuteResult<ObjectHandle> {
        let v = rand::random();
        Ok(self.obj_heap.alloc_float_instance(v))
    }

    // =====================================================================
    //  randint(min, max) — random integer in [min, max] inclusive
    // =====================================================================

    fn random_randint(&mut self, min: ObjectHandle, max: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let minv = self.random_as_i64(min, "randint")?;
        let maxv = self.random_as_i64(max, "randint")?;
        if minv > maxv {
            return Err(ExecuteError::ImportError(
                format!("randint: min ({minv}) must be <= max ({maxv})")
            ));
        }
        let v = rand::random_range(minv..=maxv);
        Ok(self.obj_heap.alloc_integer_instance(v))
    }

    // =====================================================================
    //  uniform(min, max) — random float in [min, max)
    // =====================================================================

    fn random_uniform(&mut self, min: ObjectHandle, max: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let minv = self.random_as_f64(min, "uniform")?;
        let maxv = self.random_as_f64(max, "uniform")?;
        if minv > maxv {
            return Err(ExecuteError::ImportError(
                format!("uniform: min ({minv}) must be <= max ({maxv})")
            ));
        }
        let v = rand::random_range(minv..maxv);
        Ok(self.obj_heap.alloc_float_instance(v))
    }

    // =====================================================================
    //  choice(seq) — random element from a list
    // =====================================================================

    fn random_choice(&mut self, seq: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        // Extract length first to avoid holding the immutable borrow across
        // the mutable borrow on `self.rng`.
        let len = {
            let list = self.get_list_instance(seq).map_err(|_| {
                ExecuteError::BinaryOpTypeMismatch("choice", "list", self.value_type_name(seq))
            })?;
            if list.is_empty() {
                return Err(ExecuteError::ImportError("choice: list is empty".into()));
            }
            list.len()
        };
        let idx = rand::random_range(0..len);
        Ok(self.get_list_instance(seq).unwrap()[idx])
    }

    // =====================================================================
    //  shuffle(list) — shuffle a list in place, returns the list
    // =====================================================================

    fn random_shuffle(&mut self, seq: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        // Pre-generate swap indices to avoid overlapping borrows.
        let n = {
            let list = self.get_list_instance(seq).map_err(|_| {
                ExecuteError::BinaryOpTypeMismatch("shuffle", "list", self.value_type_name(seq))
            })?;
            list.len()
        };
        // Fisher-Yates shuffle: for each i, pick j in [i, n).
        let swaps: Vec<(usize, usize)> = (0..n)
            .map(|i| (i, rand::random_range(i..n)))
            .collect();
        let list = self.get_list_instance_mut(seq).expect("just checked");
        for (i, j) in swaps {
            list.swap(i, j);
        }
        Ok(seq)
    }
}
