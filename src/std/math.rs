use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{NativeFunction, ObjectHandle, ShrString};
use std::collections::HashMap;

impl VirtualMachine {
    /// Create the `math` std module.
    pub(crate) fn create_math_module(&mut self) -> RuntimeResult<ObjectHandle> {
        // ---- function handles ----
        let sin = self.obj_heap.alloc_native_fn("sin", NativeFunction::a1(sin));
        let cos = self.obj_heap.alloc_native_fn("cos", NativeFunction::a1(cos));
        let tan = self.obj_heap.alloc_native_fn("tan", NativeFunction::a1(tan));
        let asin = self.obj_heap.alloc_native_fn("asin", NativeFunction::a1(asin));
        let acos = self.obj_heap.alloc_native_fn("acos", NativeFunction::a1(acos));
        let atan = self.obj_heap.alloc_native_fn("atan", NativeFunction::a1(atan));
        let atan2 = self.obj_heap.alloc_native_fn("atan2", NativeFunction::a2(atan2));
        let sqrt = self.obj_heap.alloc_native_fn("sqrt", NativeFunction::a1(sqrt));
        let pow = self.obj_heap.alloc_native_fn("pow", NativeFunction::a2(pow));
        let exp = self.obj_heap.alloc_native_fn("exp", NativeFunction::a1(exp));
        let ln = self.obj_heap.alloc_native_fn("ln", NativeFunction::a1(ln));
        let log2 = self.obj_heap.alloc_native_fn("log2", NativeFunction::a1(log2));
        let log10 = self.obj_heap.alloc_native_fn("log10", NativeFunction::a1(log10));
        let hypot = self.obj_heap.alloc_native_fn("hypot", NativeFunction::a2(hypot));
        let floor = self.obj_heap.alloc_native_fn("floor", NativeFunction::a1(floor));
        let ceil = self.obj_heap.alloc_native_fn("ceil", NativeFunction::a1(ceil));
        let round = self.obj_heap.alloc_native_fn("round", NativeFunction::a1(round));
        let degrees = self.obj_heap.alloc_native_fn("degrees", NativeFunction::a1(degrees));
        let radians = self.obj_heap.alloc_native_fn("radians", NativeFunction::a1(radians));

        // ---- constants ----
        let pi = self.obj_heap.alloc_float_instance(std::f64::consts::PI);
        let e = self.obj_heap.alloc_float_instance(std::f64::consts::E);
        let tau = self.obj_heap.alloc_float_instance(std::f64::consts::TAU);

        // ---- assemble module ----
        let mut exports: HashMap<ShrString, ObjectHandle> = HashMap::new();
        exports.insert(ShrString::new_str("PI"), pi);
        exports.insert(ShrString::new_str("E"), e);
        exports.insert(ShrString::new_str("TAU"), tau);
        exports.insert(ShrString::new_str("sin"), sin);
        exports.insert(ShrString::new_str("cos"), cos);
        exports.insert(ShrString::new_str("tan"), tan);
        exports.insert(ShrString::new_str("asin"), asin);
        exports.insert(ShrString::new_str("acos"), acos);
        exports.insert(ShrString::new_str("atan"), atan);
        exports.insert(ShrString::new_str("atan2"), atan2);
        exports.insert(ShrString::new_str("sqrt"), sqrt);
        exports.insert(ShrString::new_str("pow"), pow);
        exports.insert(ShrString::new_str("exp"), exp);
        exports.insert(ShrString::new_str("ln"), ln);
        exports.insert(ShrString::new_str("log2"), log2);
        exports.insert(ShrString::new_str("log10"), log10);
        exports.insert(ShrString::new_str("hypot"), hypot);
        exports.insert(ShrString::new_str("floor"), floor);
        exports.insert(ShrString::new_str("ceil"), ceil);
        exports.insert(ShrString::new_str("round"), round);
        exports.insert(ShrString::new_str("degrees"), degrees);
        exports.insert(ShrString::new_str("radians"), radians);

        let module = self.obj_heap.alloc_fields_instance(self.obj_heap.module_class, exports);
        Ok(module)
    }
}

// =====================================================================
//  Trig functions
// =====================================================================

fn sin(vm: &mut VirtualMachine, x: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let v = as_f64(vm, x, "sin")?;
    Ok(vm.obj_heap.alloc_float_instance(v.sin()))
}

fn cos(vm: &mut VirtualMachine, x: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let v = as_f64(vm, x, "cos")?;
    Ok(vm.obj_heap.alloc_float_instance(v.cos()))
}

fn tan(vm: &mut VirtualMachine, x: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let v = as_f64(vm, x, "tan")?;
    Ok(vm.obj_heap.alloc_float_instance(v.tan()))
}

fn asin(vm: &mut VirtualMachine, x: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let v = as_f64(vm, x, "asin")?;
    Ok(vm.obj_heap.alloc_float_instance(v.asin()))
}

fn acos(vm: &mut VirtualMachine, x: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let v = as_f64(vm, x, "acos")?;
    Ok(vm.obj_heap.alloc_float_instance(v.acos()))
}

fn atan(vm: &mut VirtualMachine, x: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let v = as_f64(vm, x, "atan")?;
    Ok(vm.obj_heap.alloc_float_instance(v.atan()))
}

fn atan2(vm: &mut VirtualMachine, y: ObjectHandle, x: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let yv = as_f64(vm, y, "atan2")?;
    let xv = as_f64(vm, x, "atan2")?;
    Ok(vm.obj_heap.alloc_float_instance(yv.atan2(xv)))
}

// =====================================================================
//  Power / exponential / log
// =====================================================================

fn sqrt(vm: &mut VirtualMachine, x: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let v = as_f64(vm, x, "sqrt")?;
    Ok(vm.obj_heap.alloc_float_instance(v.sqrt()))
}

fn pow(vm: &mut VirtualMachine, x: ObjectHandle, y: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let xv = as_f64(vm, x, "pow")?;
    let yv = as_f64(vm, y, "pow")?;
    Ok(vm.obj_heap.alloc_float_instance(xv.powf(yv)))
}

fn exp(vm: &mut VirtualMachine, x: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let v = as_f64(vm, x, "exp")?;
    Ok(vm.obj_heap.alloc_float_instance(v.exp()))
}

fn ln(vm: &mut VirtualMachine, x: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let v = as_f64(vm, x, "ln")?;
    Ok(vm.obj_heap.alloc_float_instance(v.ln()))
}

fn log2(vm: &mut VirtualMachine, x: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let v = as_f64(vm, x, "log2")?;
    Ok(vm.obj_heap.alloc_float_instance(v.log2()))
}

fn log10(vm: &mut VirtualMachine, x: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let v = as_f64(vm, x, "log10")?;
    Ok(vm.obj_heap.alloc_float_instance(v.log10()))
}

fn hypot(vm: &mut VirtualMachine, x: ObjectHandle, y: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let xv = as_f64(vm, x, "hypot")?;
    let yv = as_f64(vm, y, "hypot")?;
    Ok(vm.obj_heap.alloc_float_instance(xv.hypot(yv)))
}

// =====================================================================
//  Rounding
// =====================================================================

fn floor(vm: &mut VirtualMachine, x: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let v = as_f64(vm, x, "floor")?;
    Ok(vm.obj_heap.alloc_float_instance(v.floor()))
}

fn ceil(vm: &mut VirtualMachine, x: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let v = as_f64(vm, x, "ceil")?;
    Ok(vm.obj_heap.alloc_float_instance(v.ceil()))
}

fn round(vm: &mut VirtualMachine, x: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let v = as_f64(vm, x, "round")?;
    Ok(vm.obj_heap.alloc_float_instance(v.round()))
}

// =====================================================================
//  Conversion
// =====================================================================

fn degrees(vm: &mut VirtualMachine, rad: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let v = as_f64(vm, rad, "degrees")?;
    Ok(vm.obj_heap.alloc_float_instance(v.to_degrees()))
}

fn radians(vm: &mut VirtualMachine, deg: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let v = as_f64(vm, deg, "radians")?;
    Ok(vm.obj_heap.alloc_float_instance(v.to_radians()))
}

/// Extract an `f64` from a numeric handle (int or float).
fn as_f64(vm: &VirtualMachine, handle: ObjectHandle, fn_name: &'static str) -> RuntimeResult<f64> {
    if let Ok(v) = vm.get_integer_instance(handle) {
        Ok(*v as f64)
    } else if let Ok(v) = vm.get_float_instance(handle) {
        Ok(*v)
    } else {
        Err(RuntimeErrorKind::BinaryOpTypeMismatch(fn_name, "float", vm.value_type_name(handle)))
    }
}
