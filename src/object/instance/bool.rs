use super::ObjectHeap;
use crate::{
    NativeFunction, ObjectHandle, impl_object_instance_data,
    vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine},
};

// ========================================================================== //
//  ObjectBool
// ========================================================================== //

/// Represents the `Bool` built-in type.  Magic methods are implemented as
/// associated functions matching [`NativeFunction`] signatures so they can be
/// registered directly on the class during [`ObjectHeap::new`].
pub struct ObjectBool {
    pub value: bool,
}

impl_object_instance_data!(ObjectBool, "bool");

impl ObjectBool {
    pub fn new(value: bool) -> Self {
        Self { value }
    }

    /// Treat bool as 0 or 1 for arithmetic.
    fn as_int(vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<i64> {
        let val = *vm.obj_heap.get_bool_instance(handle).expect("must be bool instance");
        Ok(if val { 1 } else { 0 })
    }

    // ---- unary ----

    pub fn __neg__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.get_bool_instance(receiver).expect("must be bool instance");
        Ok(vm.obj_heap.alloc_integer_instance(if val { -1 } else { 0 }))
    }

    pub fn __not__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.get_bool_instance(receiver).expect("must be bool instance");
        Ok(vm.obj_heap.alloc_bool_instance(!val))
    }

    // ---- arithmetic ----

    pub fn __add__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_val = Self::as_int(vm, lhs)?;
        if let Some(rhs) = vm.obj_heap.get_integer_instance(rhs) {
            return Ok(vm.obj_heap.alloc_integer_instance(lhs_val.wrapping_add(*rhs)));
        }
        if let Some(rhs) = vm.obj_heap.get_float_instance(rhs) {
            return Ok(vm.obj_heap.alloc_float_instance(lhs_val as f64 + *rhs));
        }
        if vm.obj_heap.get_bool_instance(rhs).is_some() {
            let rhs_val = Self::as_int(vm, rhs)?;
            return Ok(vm.obj_heap.alloc_integer_instance(lhs_val.wrapping_add(rhs_val)));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("add", "bool", vm.obj_heap.type_of(rhs)))
    }

    pub fn __sub__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_val = Self::as_int(vm, lhs)?;
        if let Some(rhs) = vm.obj_heap.get_integer_instance(rhs) {
            return Ok(vm.obj_heap.alloc_integer_instance(lhs_val.wrapping_sub(*rhs)));
        }
        if let Some(rhs) = vm.obj_heap.get_float_instance(rhs) {
            return Ok(vm.obj_heap.alloc_float_instance(lhs_val as f64 - *rhs));
        }
        if vm.obj_heap.get_bool_instance(rhs).is_some() {
            let rhs_val = Self::as_int(vm, rhs)?;
            return Ok(vm.obj_heap.alloc_integer_instance(lhs_val.wrapping_sub(rhs_val)));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("sub", "bool", vm.obj_heap.type_of(rhs)))
    }

    pub fn __mul__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_val = Self::as_int(vm, lhs)?;
        if let Some(rhs) = vm.obj_heap.get_integer_instance(rhs) {
            return Ok(vm.obj_heap.alloc_integer_instance(lhs_val.wrapping_mul(*rhs)));
        }
        if let Some(rhs) = vm.obj_heap.get_float_instance(rhs) {
            return Ok(vm.obj_heap.alloc_float_instance(lhs_val as f64 * *rhs));
        }
        if vm.obj_heap.get_bool_instance(rhs).is_some() {
            let rhs_val = Self::as_int(vm, rhs)?;
            return Ok(vm.obj_heap.alloc_integer_instance(lhs_val.wrapping_mul(rhs_val)));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("mul", "bool", vm.obj_heap.type_of(rhs)))
    }

    pub fn __div__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_val = Self::as_int(vm, lhs)? as f64;
        if let Some(rhs) = vm.obj_heap.get_integer_instance(rhs) {
            if *rhs == 0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_float_instance(lhs_val / *rhs as f64));
        }
        if let Some(rhs) = vm.obj_heap.get_float_instance(rhs) {
            if *rhs == 0.0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_float_instance(lhs_val / *rhs));
        }
        if vm.obj_heap.get_bool_instance(rhs).is_some() {
            let rhs_val = Self::as_int(vm, rhs)?;
            if rhs_val == 0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_float_instance(lhs_val / rhs_val as f64));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("div", "bool", vm.obj_heap.type_of(rhs)))
    }

    pub fn __floordiv__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_val = Self::as_int(vm, lhs)?;
        if let Some(rhs) = vm.obj_heap.get_integer_instance(rhs) {
            if *rhs == 0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_integer_instance(i64::wrapping_div_euclid(lhs_val, *rhs)));
        }
        if let Some(rhs) = vm.obj_heap.get_float_instance(rhs) {
            if *rhs == 0.0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_float_instance((lhs_val as f64 / *rhs).floor()));
        }
        if vm.obj_heap.get_bool_instance(rhs).is_some() {
            let rhs_val = Self::as_int(vm, rhs)?;
            if rhs_val == 0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_integer_instance(i64::wrapping_div_euclid(lhs_val, rhs_val)));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("floordiv", "bool", vm.obj_heap.type_of(rhs)))
    }

    pub fn __mod__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_val = Self::as_int(vm, lhs)?;
        if let Some(rhs) = vm.obj_heap.get_integer_instance(rhs) {
            if *rhs == 0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_integer_instance(i64::wrapping_rem_euclid(lhs_val, *rhs)));
        }
        if let Some(rhs) = vm.obj_heap.get_float_instance(rhs) {
            if *rhs == 0.0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_float_instance((lhs_val as f64).rem_euclid(*rhs)));
        }
        if vm.obj_heap.get_bool_instance(rhs).is_some() {
            let rhs_val = Self::as_int(vm, rhs)?;
            if rhs_val == 0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_integer_instance(i64::wrapping_rem_euclid(lhs_val, rhs_val)));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("mod", "bool", vm.obj_heap.type_of(rhs)))
    }

    // ---- comparison ----

    pub fn __eq__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_val = *vm.obj_heap.expect_bool(lhs)?;
        if let Some(rhs) = vm.obj_heap.get_bool_instance(rhs) {
            return Ok(vm.obj_heap.alloc_bool_instance(lhs_val == *rhs));
        }
        let lhs_int = if lhs_val { 1i64 } else { 0 };
        if let Some(rhs) = vm.obj_heap.get_integer_instance(rhs) {
            return Ok(vm.obj_heap.alloc_bool_instance(lhs_int == *rhs));
        }
        if let Some(rhs) = vm.obj_heap.get_float_instance(rhs) {
            return Ok(vm.obj_heap.alloc_bool_instance(lhs_int as f64 == *rhs));
        }
        Ok(vm.obj_heap.alloc_bool_instance(false))
    }

    pub fn __ne__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let eq = Self::__eq__(vm, lhs, rhs)?;
        let b = vm.obj_heap.get_bool_instance_mut(eq).expect("must return bool");
        *b = !*b;
        Ok(eq)
    }

    pub fn __gt__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_int = Self::as_int(vm, lhs)?;
        if let Some(rhs) = vm.obj_heap.get_bool_instance(rhs) {
            return Ok(vm.obj_heap.alloc_bool_instance(lhs_int > (if *rhs { 1 } else { 0 })));
        }
        if let Some(rhs) = vm.obj_heap.get_integer_instance(rhs) {
            return Ok(vm.obj_heap.alloc_bool_instance(lhs_int > *rhs));
        }
        if let Some(rhs) = vm.obj_heap.get_float_instance(rhs) {
            return Ok(vm.obj_heap.alloc_bool_instance(lhs_int as f64 > *rhs));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("gt", "bool", vm.obj_heap.type_of(rhs)))
    }

    pub fn __ge__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_val = *vm.obj_heap.expect_bool(lhs)?;
        let lhs_int = if lhs_val { 1i64 } else { 0 };
        if let Some(rhs_val) = vm.obj_heap.get_bool_instance(rhs) {
            return Ok(vm.obj_heap.alloc_bool_instance(lhs_int >= (if *rhs_val { 1 } else { 0 })));
        }
        if let Some(rhs_val) = vm.obj_heap.get_integer_instance(rhs) {
            return Ok(vm.obj_heap.alloc_bool_instance(lhs_int >= *rhs_val));
        }
        if let Some(rhs_val) = vm.obj_heap.get_float_instance(rhs) {
            return Ok(vm.obj_heap.alloc_bool_instance(lhs_int as f64 >= *rhs_val));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("ge", "bool", vm.obj_heap.type_of(rhs)))
    }

    pub fn __lt__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_val = *vm.obj_heap.expect_bool(lhs)?;
        let lhs_int = if lhs_val { 1i64 } else { 0 };
        if let Some(rhs_val) = vm.obj_heap.get_bool_instance(rhs) {
            return Ok(vm.obj_heap.alloc_bool_instance(lhs_int < (if *rhs_val { 1 } else { 0 })));
        }
        if let Some(rhs_val) = vm.obj_heap.get_integer_instance(rhs) {
            return Ok(vm.obj_heap.alloc_bool_instance(lhs_int < *rhs_val));
        }
        if let Some(rhs_val) = vm.obj_heap.get_float_instance(rhs) {
            return Ok(vm.obj_heap.alloc_bool_instance((lhs_int as f64) < *rhs_val));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("lt", "bool", vm.obj_heap.type_of(rhs)))
    }

    pub fn __le__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_val = *vm.obj_heap.expect_bool(lhs)?;
        let lhs_int = if lhs_val { 1i64 } else { 0 };
        if let Some(rhs_val) = vm.obj_heap.get_bool_instance(rhs) {
            return Ok(vm.obj_heap.alloc_bool_instance(lhs_int <= (if *rhs_val { 1 } else { 0 })));
        }
        if let Some(rhs_val) = vm.obj_heap.get_integer_instance(rhs) {
            return Ok(vm.obj_heap.alloc_bool_instance(lhs_int <= *rhs_val));
        }
        if let Some(rhs_val) = vm.obj_heap.get_float_instance(rhs) {
            return Ok(vm.obj_heap.alloc_bool_instance(lhs_int as f64 <= *rhs_val));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("le", "bool", vm.obj_heap.type_of(rhs)))
    }

    // ---- conversion ----

    pub fn __str__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.expect_bool(receiver)?;
        Ok(vm.obj_heap.alloc_string_instance(if val { crate::ShrString::from("true") } else { crate::ShrString::from("false") }))
    }

    pub fn __bool__(_vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        Ok(receiver)
    }

    pub fn __hash__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.expect_bool(receiver)?;
        Ok(vm.obj_heap.alloc_integer_instance(if val { 1 } else { 0 }))
    }

    pub fn __int__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.expect_bool(receiver)?;
        Ok(vm.obj_heap.alloc_integer_instance(if val { 1 } else { 0 }))
    }

    pub fn __float__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.expect_bool(receiver)?;
        Ok(vm.obj_heap.alloc_float_instance(if val { 1.0 } else { 0.0 }))
    }
}

// ========================================================================== //
//  Registration
// ========================================================================== //

/// Register all `Bool` magic methods directly on the class during heap init.
pub fn register_bool_builtins(heap: &mut ObjectHeap) {
    let bc = heap.bool_class;
    heap.register_native_method(bc, "__neg__", NativeFunction::a1(ObjectBool::__neg__));
    heap.register_native_method(bc, "__not__", NativeFunction::a1(ObjectBool::__not__));
    heap.register_native_method(bc, "__add__", NativeFunction::a2(ObjectBool::__add__));
    heap.register_native_method(bc, "__sub__", NativeFunction::a2(ObjectBool::__sub__));
    heap.register_native_method(bc, "__mul__", NativeFunction::a2(ObjectBool::__mul__));
    heap.register_native_method(bc, "__div__", NativeFunction::a2(ObjectBool::__div__));
    heap.register_native_method(bc, "__floordiv__", NativeFunction::a2(ObjectBool::__floordiv__));
    heap.register_native_method(bc, "__mod__", NativeFunction::a2(ObjectBool::__mod__));
    heap.register_native_method(bc, "__eq__", NativeFunction::a2(ObjectBool::__eq__));
    heap.register_native_method(bc, "__ne__", NativeFunction::a2(ObjectBool::__ne__));
    heap.register_native_method(bc, "__gt__", NativeFunction::a2(ObjectBool::__gt__));
    heap.register_native_method(bc, "__ge__", NativeFunction::a2(ObjectBool::__ge__));
    heap.register_native_method(bc, "__lt__", NativeFunction::a2(ObjectBool::__lt__));
    heap.register_native_method(bc, "__le__", NativeFunction::a2(ObjectBool::__le__));
    heap.register_native_method(bc, "__str__", NativeFunction::a1(ObjectBool::__str__));
    heap.register_native_method(bc, "__bool__", NativeFunction::a1(ObjectBool::__bool__));
    heap.register_native_method(bc, "__hash__", NativeFunction::a1(ObjectBool::__hash__));
    heap.register_native_method(bc, "__int__", NativeFunction::a1(ObjectBool::__int__));
    heap.register_native_method(bc, "__float__", NativeFunction::a1(ObjectBool::__float__));
}
