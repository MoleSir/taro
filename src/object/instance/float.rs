use super::ObjectHeap;
use crate::{
    NativeFunction, ObjectHandle, impl_object_instance_data,
    vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine},
};

// ========================================================================== //
//  ObjectFloat
// ========================================================================== //

/// Represents the `Float` built-in type.  Magic methods are implemented as
/// associated functions matching [`NativeFunction`] signatures so they can be
/// registered directly on the class during [`ObjectHeap::new`].
pub struct ObjectFloat {
    pub value: f64,
}

impl_object_instance_data!(ObjectFloat, "float");

macro_rules! float_binary_arith {
    ($name:ident, $float_op:expr, $op_name:literal) => {
        pub fn $name(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
            let lhs_val = *vm.obj_heap.expect_float(lhs)?;
            if let Some(rhs) = vm.obj_heap.get_float_instance(rhs) {
                return Ok(vm.obj_heap.alloc_float_instance($float_op(lhs_val, *rhs)));
            }
            if let Some(rhs) = vm.obj_heap.get_integer_instance(rhs) {
                return Ok(vm.obj_heap.alloc_float_instance($float_op(lhs_val, *rhs as f64)));
            }
            Err(RuntimeErrorKind::BinaryOpTypeMismatch($op_name, "float", vm.obj_heap.type_of(rhs)))
        }
    };
}

macro_rules! float_cmp_op {
    ($name:ident, $float_cmp:expr, $op_name:literal) => {
        pub fn $name(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
            let lhs_val = *vm.obj_heap.expect_float(lhs)?;
            let result = if let Some(rhs) = vm.obj_heap.get_float_instance(rhs) {
                $float_cmp(lhs_val, *rhs)
            } else if let Some(rhs) = vm.obj_heap.get_integer_instance(rhs) {
                $float_cmp(lhs_val, *rhs as f64)
            } else {
                return Err(RuntimeErrorKind::BinaryOpTypeMismatch($op_name, "float", vm.obj_heap.type_of(rhs)));
            };
            Ok(vm.obj_heap.alloc_bool_instance(result))
        }
    };
}

impl ObjectFloat {
    float_binary_arith!(__add__, |a, b| a + b, "add");
    float_binary_arith!(__sub__, |a, b| a - b, "sub");
    float_binary_arith!(__mul__, |a, b| a * b, "mul");

    float_cmp_op!(__eq__, |a, b| a == b, "eq");
    float_cmp_op!(__ne__, |a, b| a != b, "ne");
    float_cmp_op!(__gt__, |a, b| a > b, "gt");
    float_cmp_op!(__ge__, |a, b| a >= b, "ge");
    float_cmp_op!(__lt__, |a, b| a < b, "lt");
    float_cmp_op!(__le__, |a, b| a <= b, "le");

    pub fn __div__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_val = *vm.obj_heap.expect_float(lhs)?;
        if let Some(rhs) = vm.obj_heap.get_float_instance(rhs) {
            if *rhs == 0.0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_float_instance(lhs_val / *rhs));
        }
        if let Some(rhs) = vm.obj_heap.get_integer_instance(rhs) {
            if *rhs == 0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_float_instance(lhs_val / *rhs as f64));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("div", "float", vm.obj_heap.type_of(rhs)))
    }

    pub fn __floordiv__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_val = *vm.obj_heap.expect_float(lhs)?;
        if let Some(rhs) = vm.obj_heap.get_float_instance(rhs) {
            if *rhs == 0.0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_float_instance((lhs_val / *rhs).floor()));
        }
        if let Some(rhs) = vm.obj_heap.get_integer_instance(rhs) {
            if *rhs == 0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_float_instance((lhs_val / *rhs as f64).floor()));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("floordiv", "float", vm.obj_heap.type_of(rhs)))
    }

    pub fn __mod__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_val = *vm.obj_heap.expect_float(lhs)?;
        if let Some(rhs) = vm.obj_heap.get_float_instance(rhs) {
            if *rhs == 0.0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_float_instance(lhs_val.rem_euclid(*rhs)));
        }
        if let Some(rhs) = vm.obj_heap.get_integer_instance(rhs) {
            if *rhs == 0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_float_instance(lhs_val.rem_euclid(*rhs as f64)));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("mod", "float", vm.obj_heap.type_of(rhs)))
    }

    pub fn __neg__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.expect_float(receiver)?;
        Ok(vm.obj_heap.alloc_float_instance(-val))
    }

    pub fn __not__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.expect_float(receiver)?;
        Ok(vm.obj_heap.alloc_bool_instance(val == 0.0))
    }

    pub fn __str__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.expect_float(receiver)?;
        Ok(vm.obj_heap.alloc_string_instance(crate::format_shr!("{}", val)))
    }

    pub fn __bool__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.expect_float(receiver)?;
        Ok(vm.obj_heap.alloc_bool_instance(val != 0.0))
    }

    pub fn __hash__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.expect_float(receiver)?;
        let hash = val.to_bits() as i64;
        Ok(vm.obj_heap.alloc_integer_instance(hash))
    }

    pub fn __int__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.expect_float(receiver)?;
        Ok(vm.obj_heap.alloc_integer_instance(val as i64))
    }

    pub fn __float__(_vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        Ok(receiver)
    }
}

// ========================================================================== //
//  Registration
// ========================================================================== //

/// Register all `Float` magic methods directly on the class during heap init.
pub fn register_float_builtins(heap: &mut ObjectHeap) {
    let fc = heap.float_class;
    heap.register_native_method(fc, "__neg__", NativeFunction::a1(ObjectFloat::__neg__));
    heap.register_native_method(fc, "__not__", NativeFunction::a1(ObjectFloat::__not__));
    heap.register_native_method(fc, "__add__", NativeFunction::a2(ObjectFloat::__add__));
    heap.register_native_method(fc, "__sub__", NativeFunction::a2(ObjectFloat::__sub__));
    heap.register_native_method(fc, "__mul__", NativeFunction::a2(ObjectFloat::__mul__));
    heap.register_native_method(fc, "__div__", NativeFunction::a2(ObjectFloat::__div__));
    heap.register_native_method(fc, "__floordiv__", NativeFunction::a2(ObjectFloat::__floordiv__));
    heap.register_native_method(fc, "__mod__", NativeFunction::a2(ObjectFloat::__mod__));
    heap.register_native_method(fc, "__eq__", NativeFunction::a2(ObjectFloat::__eq__));
    heap.register_native_method(fc, "__ne__", NativeFunction::a2(ObjectFloat::__ne__));
    heap.register_native_method(fc, "__gt__", NativeFunction::a2(ObjectFloat::__gt__));
    heap.register_native_method(fc, "__ge__", NativeFunction::a2(ObjectFloat::__ge__));
    heap.register_native_method(fc, "__lt__", NativeFunction::a2(ObjectFloat::__lt__));
    heap.register_native_method(fc, "__le__", NativeFunction::a2(ObjectFloat::__le__));
    heap.register_native_method(fc, "__str__", NativeFunction::a1(ObjectFloat::__str__));
    heap.register_native_method(fc, "__bool__", NativeFunction::a1(ObjectFloat::__bool__));
    heap.register_native_method(fc, "__hash__", NativeFunction::a1(ObjectFloat::__hash__));
    heap.register_native_method(fc, "__int__", NativeFunction::a1(ObjectFloat::__int__));
    heap.register_native_method(fc, "__float__", NativeFunction::a1(ObjectFloat::__float__));
}
