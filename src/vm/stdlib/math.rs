use std::collections::HashMap;

use crate::{NativeFunction, ObjectHandle, ObjectInstanceData, ShrString};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

impl VirtualMachine {
    /// Extract an `f64` from a numeric handle (int or float).
    fn math_as_f64(&self, handle: ObjectHandle, fn_name: &'static str) -> ExecuteResult<f64> {
        if let Ok(v) = self.get_integer_instance(handle) {
            Ok(*v as f64)
        } else if let Ok(v) = self.get_float_instance(handle) {
            Ok(*v)
        } else {
            Err(ExecuteError::BinaryOpTypeMismatch(fn_name, "float", self.value_type_name(handle)))
        }
    }

    /// Create the `math` std module.
    pub(super) fn create_math_module(&mut self) -> ExecuteResult<ObjectHandle> {
        // ---- function handles ----
        let sin     = self.obj_heap.alloc_native_fn("sin",     NativeFunction::a1(VirtualMachine::math_sin));
        let cos     = self.obj_heap.alloc_native_fn("cos",     NativeFunction::a1(VirtualMachine::math_cos));
        let tan     = self.obj_heap.alloc_native_fn("tan",     NativeFunction::a1(VirtualMachine::math_tan));
        let asin    = self.obj_heap.alloc_native_fn("asin",    NativeFunction::a1(VirtualMachine::math_asin));
        let acos    = self.obj_heap.alloc_native_fn("acos",    NativeFunction::a1(VirtualMachine::math_acos));
        let atan    = self.obj_heap.alloc_native_fn("atan",    NativeFunction::a1(VirtualMachine::math_atan));
        let atan2   = self.obj_heap.alloc_native_fn("atan2",   NativeFunction::a2(VirtualMachine::math_atan2));
        let sqrt    = self.obj_heap.alloc_native_fn("sqrt",    NativeFunction::a1(VirtualMachine::math_sqrt));
        let pow     = self.obj_heap.alloc_native_fn("pow",     NativeFunction::a2(VirtualMachine::math_pow));
        let exp     = self.obj_heap.alloc_native_fn("exp",     NativeFunction::a1(VirtualMachine::math_exp));
        let ln      = self.obj_heap.alloc_native_fn("ln",      NativeFunction::a1(VirtualMachine::math_ln));
        let log2    = self.obj_heap.alloc_native_fn("log2",    NativeFunction::a1(VirtualMachine::math_log2));
        let log10   = self.obj_heap.alloc_native_fn("log10",   NativeFunction::a1(VirtualMachine::math_log10));
        let hypot   = self.obj_heap.alloc_native_fn("hypot",   NativeFunction::a2(VirtualMachine::math_hypot));
        let floor   = self.obj_heap.alloc_native_fn("floor",   NativeFunction::a1(VirtualMachine::math_floor));
        let ceil    = self.obj_heap.alloc_native_fn("ceil",    NativeFunction::a1(VirtualMachine::math_ceil));
        let round   = self.obj_heap.alloc_native_fn("round",   NativeFunction::a1(VirtualMachine::math_round));
        let degrees = self.obj_heap.alloc_native_fn("degrees", NativeFunction::a1(VirtualMachine::math_degrees));
        let radians = self.obj_heap.alloc_native_fn("radians", NativeFunction::a1(VirtualMachine::math_radians));

        // ---- constants ----
        let pi  = self.obj_heap.alloc_float_instance(std::f64::consts::PI);
        let e   = self.obj_heap.alloc_float_instance(std::f64::consts::E);
        let tau = self.obj_heap.alloc_float_instance(std::f64::consts::TAU);

        // ---- assemble module ----
        let mut exports: HashMap<ShrString, ObjectHandle> = HashMap::new();
        exports.insert(ShrString::new_str("PI"),  pi);
        exports.insert(ShrString::new_str("E"),   e);
        exports.insert(ShrString::new_str("TAU"), tau);
        exports.insert(ShrString::new_str("sin"),     sin);
        exports.insert(ShrString::new_str("cos"),     cos);
        exports.insert(ShrString::new_str("tan"),     tan);
        exports.insert(ShrString::new_str("asin"),    asin);
        exports.insert(ShrString::new_str("acos"),    acos);
        exports.insert(ShrString::new_str("atan"),    atan);
        exports.insert(ShrString::new_str("atan2"),   atan2);
        exports.insert(ShrString::new_str("sqrt"),    sqrt);
        exports.insert(ShrString::new_str("pow"),     pow);
        exports.insert(ShrString::new_str("exp"),     exp);
        exports.insert(ShrString::new_str("ln"),      ln);
        exports.insert(ShrString::new_str("log2"),    log2);
        exports.insert(ShrString::new_str("log10"),   log10);
        exports.insert(ShrString::new_str("hypot"),   hypot);
        exports.insert(ShrString::new_str("floor"),   floor);
        exports.insert(ShrString::new_str("ceil"),    ceil);
        exports.insert(ShrString::new_str("round"),   round);
        exports.insert(ShrString::new_str("degrees"), degrees);
        exports.insert(ShrString::new_str("radians"), radians);

        let module = self.obj_heap.alloc_fields_instance(self.obj_heap.module_class);
        if let Some(inst) = self.obj_heap.get_instance_mut(module) {
            if let ObjectInstanceData::Fields(fields) = &mut inst.data {
                *fields = exports;
            }
        }

        Ok(module)
    }

    // =====================================================================
    //  Trig functions
    // =====================================================================

    fn math_sin(&mut self, x: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let v = self.math_as_f64(x, "sin")?;
        Ok(self.obj_heap.alloc_float_instance(v.sin()))
    }

    fn math_cos(&mut self, x: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let v = self.math_as_f64(x, "cos")?;
        Ok(self.obj_heap.alloc_float_instance(v.cos()))
    }

    fn math_tan(&mut self, x: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let v = self.math_as_f64(x, "tan")?;
        Ok(self.obj_heap.alloc_float_instance(v.tan()))
    }

    fn math_asin(&mut self, x: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let v = self.math_as_f64(x, "asin")?;
        Ok(self.obj_heap.alloc_float_instance(v.asin()))
    }

    fn math_acos(&mut self, x: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let v = self.math_as_f64(x, "acos")?;
        Ok(self.obj_heap.alloc_float_instance(v.acos()))
    }

    fn math_atan(&mut self, x: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let v = self.math_as_f64(x, "atan")?;
        Ok(self.obj_heap.alloc_float_instance(v.atan()))
    }

    fn math_atan2(&mut self, y: ObjectHandle, x: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let yv = self.math_as_f64(y, "atan2")?;
        let xv = self.math_as_f64(x, "atan2")?;
        Ok(self.obj_heap.alloc_float_instance(yv.atan2(xv)))
    }

    // =====================================================================
    //  Power / exponential / log
    // =====================================================================

    fn math_sqrt(&mut self, x: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let v = self.math_as_f64(x, "sqrt")?;
        Ok(self.obj_heap.alloc_float_instance(v.sqrt()))
    }

    fn math_pow(&mut self, x: ObjectHandle, y: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let xv = self.math_as_f64(x, "pow")?;
        let yv = self.math_as_f64(y, "pow")?;
        Ok(self.obj_heap.alloc_float_instance(xv.powf(yv)))
    }

    fn math_exp(&mut self, x: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let v = self.math_as_f64(x, "exp")?;
        Ok(self.obj_heap.alloc_float_instance(v.exp()))
    }

    fn math_ln(&mut self, x: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let v = self.math_as_f64(x, "ln")?;
        Ok(self.obj_heap.alloc_float_instance(v.ln()))
    }

    fn math_log2(&mut self, x: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let v = self.math_as_f64(x, "log2")?;
        Ok(self.obj_heap.alloc_float_instance(v.log2()))
    }

    fn math_log10(&mut self, x: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let v = self.math_as_f64(x, "log10")?;
        Ok(self.obj_heap.alloc_float_instance(v.log10()))
    }

    fn math_hypot(&mut self, x: ObjectHandle, y: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let xv = self.math_as_f64(x, "hypot")?;
        let yv = self.math_as_f64(y, "hypot")?;
        Ok(self.obj_heap.alloc_float_instance(xv.hypot(yv)))
    }

    // =====================================================================
    //  Rounding
    // =====================================================================

    fn math_floor(&mut self, x: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let v = self.math_as_f64(x, "floor")?;
        Ok(self.obj_heap.alloc_float_instance(v.floor()))
    }

    fn math_ceil(&mut self, x: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let v = self.math_as_f64(x, "ceil")?;
        Ok(self.obj_heap.alloc_float_instance(v.ceil()))
    }

    fn math_round(&mut self, x: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let v = self.math_as_f64(x, "round")?;
        Ok(self.obj_heap.alloc_float_instance(v.round()))
    }

    // =====================================================================
    //  Conversion
    // =====================================================================

    fn math_degrees(&mut self, rad: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let v = self.math_as_f64(rad, "degrees")?;
        Ok(self.obj_heap.alloc_float_instance(v.to_degrees()))
    }

    fn math_radians(&mut self, deg: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let v = self.math_as_f64(deg, "radians")?;
        Ok(self.obj_heap.alloc_float_instance(v.to_radians()))
    }
}
