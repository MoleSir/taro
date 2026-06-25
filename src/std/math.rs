use super::ModuleBuilder;
use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{NativeFunction, ObjectHandle};

impl VirtualMachine {
    /// Create the `math` std module.
    pub(crate) fn create_math_module(&mut self) -> RuntimeResult<ObjectHandle> {
        // Pre-allocate constants before handing &mut heap to the builder.
        let pi = self.obj_heap.alloc_float_instance(std::f64::consts::PI);
        let e_val = self.obj_heap.alloc_float_instance(std::f64::consts::E);
        let tau = self.obj_heap.alloc_float_instance(std::f64::consts::TAU);

        let mut m = ModuleBuilder::new(&mut self.obj_heap, "math");

        // ---- constants ----
        m.define_value("PI", pi);
        m.define_value("E", e_val);
        m.define_value("TAU", tau);

        // ---- trig ----
        m.define_fn("sin", NativeFunction::a1(sin));
        m.define_fn("cos", NativeFunction::a1(cos));
        m.define_fn("tan", NativeFunction::a1(tan));
        m.define_fn("asin", NativeFunction::a1(asin));
        m.define_fn("acos", NativeFunction::a1(acos));
        m.define_fn("atan", NativeFunction::a1(atan));
        m.define_fn("atan2", NativeFunction::a2(atan2));

        // ---- power / log ----
        m.define_fn("sqrt", NativeFunction::a1(sqrt));
        m.define_fn("pow", NativeFunction::a2(pow));
        m.define_fn("exp", NativeFunction::a1(exp));
        m.define_fn("ln", NativeFunction::a1(ln));
        m.define_fn("log2", NativeFunction::a1(log2));
        m.define_fn("log10", NativeFunction::a1(log10));
        m.define_fn("hypot", NativeFunction::a2(hypot));

        // ---- comparison ----
        m.define_fn("min", NativeFunction::a2(min));
        m.define_fn("max", NativeFunction::a2(max));

        // ---- rounding ----
        m.define_fn("floor", NativeFunction::a1(floor));
        m.define_fn("ceil", NativeFunction::a1(ceil));
        m.define_fn("round", NativeFunction::a1(round));

        // ---- conversion ----
        m.define_fn("degrees", NativeFunction::a1(degrees));
        m.define_fn("radians", NativeFunction::a1(radians));

        Ok(m.build())
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
//  Comparison
// =====================================================================

fn min(vm: &mut VirtualMachine, a: ObjectHandle, b: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let lhs = as_f64(vm, a, "min")?;
    let rhs = as_f64(vm, b, "min")?;
    Ok(vm.obj_heap.alloc_float_instance(lhs.min(rhs)))
}

fn max(vm: &mut VirtualMachine, a: ObjectHandle, b: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let lhs = as_f64(vm, a, "max")?;
    let rhs = as_f64(vm, b, "max")?;
    Ok(vm.obj_heap.alloc_float_instance(lhs.max(rhs)))
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
    if let Some(v) = vm.obj_heap.get_integer_instance(handle) {
        Ok(*v as f64)
    } else if let Some(v) = vm.obj_heap.get_float_instance(handle) {
        Ok(*v)
    } else {
        Err(RuntimeErrorKind::BinaryOpTypeMismatch(fn_name, "float", vm.obj_heap.type_of(handle)))
    }
}
