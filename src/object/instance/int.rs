use super::ObjectHeap;
use crate::{
    NativeFunction, ObjectHandle, impl_object_instance_data,
    vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine},
};

// ========================================================================== //
//  ObjectInt
// ========================================================================== //

/// Represents the `Int` built-in type.  Magic methods are implemented as
/// associated functions matching [`NativeFunction`] signatures so they can be
/// registered directly on the class during [`ObjectHeap::new`].
pub struct ObjectInt {
    pub value: i64,
}

impl_object_instance_data!(ObjectInt, "int");

macro_rules! int_binary_arith {
    ($name:ident, $int_op:expr, $float_op:expr, $op_name:literal) => {
        pub fn $name(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
            let lhs_val = *vm.obj_heap.expect_integer(lhs)?;
            if let Some(rhs) = vm.obj_heap.get_integer_instance(rhs) {
                return Ok(vm.obj_heap.alloc_integer_instance($int_op(lhs_val, *rhs)));
            }
            if let Some(rhs) = vm.obj_heap.get_float_instance(rhs) {
                return Ok(vm.obj_heap.alloc_float_instance($float_op(lhs_val as f64, *rhs)));
            }
            Err(RuntimeErrorKind::BinaryOpTypeMismatch($op_name, "int", vm.obj_heap.type_of(rhs)))
        }
    };
}

macro_rules! int_cmp_op {
    ($name:ident, $int_cmp:expr, $float_cmp:expr, $op_name:literal) => {
        pub fn $name(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
            let lhs_val = *vm.obj_heap.expect_integer(lhs)?;
            let result = if let Some(rhs) = vm.obj_heap.get_integer_instance(rhs) {
                $int_cmp(lhs_val, *rhs)
            } else if let Some(rhs) = vm.obj_heap.get_float_instance(rhs) {
                $float_cmp(lhs_val as f64, *rhs)
            } else {
                return Err(RuntimeErrorKind::BinaryOpTypeMismatch($op_name, "int", vm.obj_heap.type_of(rhs)));
            };
            Ok(vm.obj_heap.alloc_bool_instance(result))
        }
    };
}

impl ObjectInt {
    pub fn new(value: i64) -> Self {
        Self { value }
    }

    int_binary_arith!(__add__, |a, b| i64::wrapping_add(a, b), |a, b| a + b, "add");
    int_binary_arith!(__sub__, |a, b| i64::wrapping_sub(a, b), |a, b| a - b, "sub");
    int_binary_arith!(__mul__, |a, b| i64::wrapping_mul(a, b), |a, b| a * b, "mul");

    int_cmp_op!(__eq__, |a, b| a == b, |a, b| a == b, "eq");
    int_cmp_op!(__ne__, |a, b| a != b, |a, b| a != b, "ne");
    int_cmp_op!(__gt__, |a, b| a > b, |a, b| a > b, "gt");
    int_cmp_op!(__ge__, |a, b| a >= b, |a, b| a >= b, "ge");
    int_cmp_op!(__lt__, |a, b| a < b, |a, b| a < b, "lt");
    int_cmp_op!(__le__, |a, b| a <= b, |a, b| a <= b, "le");

    pub fn __div__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_val = *vm.obj_heap.expect_integer(lhs)?;
        if let Some(rhs) = vm.obj_heap.get_integer_instance(rhs) {
            if *rhs == 0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_float_instance(lhs_val as f64 / *rhs as f64));
        }
        if let Some(rhs) = vm.obj_heap.get_float_instance(rhs) {
            if *rhs == 0.0 {
                return Err(RuntimeErrorKind::DivideByZero);
            }
            return Ok(vm.obj_heap.alloc_float_instance(lhs_val as f64 / *rhs));
        }
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("div", "int", vm.obj_heap.type_of(rhs)))
    }

    pub fn __floordiv__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_val = *vm.obj_heap.expect_integer(lhs)?;
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
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("floordiv", "int", vm.obj_heap.type_of(rhs)))
    }

    pub fn __mod__(vm: &mut VirtualMachine, lhs: ObjectHandle, rhs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let lhs_val = *vm.obj_heap.expect_integer(lhs)?;
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
        Err(RuntimeErrorKind::BinaryOpTypeMismatch("mod", "int", vm.obj_heap.type_of(rhs)))
    }

    pub fn __neg__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.expect_integer(receiver)?;
        Ok(vm.obj_heap.alloc_integer_instance(val.wrapping_neg()))
    }

    pub fn __not__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.expect_integer(receiver)?;
        Ok(vm.obj_heap.alloc_bool_instance(val == 0))
    }

    pub fn __str__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.expect_integer(receiver)?;
        Ok(vm.obj_heap.alloc_string_instance(crate::format_shr!("{}", val)))
    }

    pub fn __bool__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.expect_integer(receiver)?;
        Ok(vm.obj_heap.alloc_bool_instance(val != 0))
    }

    pub fn __hash__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.expect_integer(receiver)?;
        Ok(vm.obj_heap.alloc_integer_instance(val))
    }

    pub fn __int__(_vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        Ok(receiver)
    }

    pub fn __float__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let val = *vm.obj_heap.expect_integer(receiver)?;
        Ok(vm.obj_heap.alloc_float_instance(val as f64))
    }
}

// ========================================================================== //
//  Registration
// ========================================================================== //

/// Register all `Int` magic methods directly on the class during heap init.
pub fn register_int_builtins(heap: &mut ObjectHeap) {
    let ic = heap.int_class;
    heap.register_native_method(ic, "__neg__", NativeFunction::a1(ObjectInt::__neg__));
    heap.register_native_method(ic, "__not__", NativeFunction::a1(ObjectInt::__not__));
    heap.register_native_method(ic, "__add__", NativeFunction::a2(ObjectInt::__add__));
    heap.register_native_method(ic, "__sub__", NativeFunction::a2(ObjectInt::__sub__));
    heap.register_native_method(ic, "__mul__", NativeFunction::a2(ObjectInt::__mul__));
    heap.register_native_method(ic, "__div__", NativeFunction::a2(ObjectInt::__div__));
    heap.register_native_method(ic, "__floordiv__", NativeFunction::a2(ObjectInt::__floordiv__));
    heap.register_native_method(ic, "__mod__", NativeFunction::a2(ObjectInt::__mod__));
    heap.register_native_method(ic, "__eq__", NativeFunction::a2(ObjectInt::__eq__));
    heap.register_native_method(ic, "__ne__", NativeFunction::a2(ObjectInt::__ne__));
    heap.register_native_method(ic, "__gt__", NativeFunction::a2(ObjectInt::__gt__));
    heap.register_native_method(ic, "__ge__", NativeFunction::a2(ObjectInt::__ge__));
    heap.register_native_method(ic, "__lt__", NativeFunction::a2(ObjectInt::__lt__));
    heap.register_native_method(ic, "__le__", NativeFunction::a2(ObjectInt::__le__));
    heap.register_native_method(ic, "__str__", NativeFunction::a1(ObjectInt::__str__));
    heap.register_native_method(ic, "__bool__", NativeFunction::a1(ObjectInt::__bool__));
    heap.register_native_method(ic, "__hash__", NativeFunction::a1(ObjectInt::__hash__));
    heap.register_native_method(ic, "__int__", NativeFunction::a1(ObjectInt::__int__));
    heap.register_native_method(ic, "__float__", NativeFunction::a1(ObjectInt::__float__));
}
