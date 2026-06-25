use crate::{impl_object_instance_data, vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine}, ObjectHandle};

pub(super) struct ObjectRangeIter {
    pub current: i64,
    pub stop: i64,
    pub step: i64,
}

impl_object_instance_data!(ObjectRangeIter, "RangeIter");

impl ObjectRangeIter {
    pub(super) fn new(start: i64, stop: i64, step: i64) -> Self {
        Self { current: start, stop, step }
    }

    pub(super) fn len(&self) -> i64 {
        if self.step > 0 {
            if self.current >= self.stop { 0 } else { (self.stop - self.current - 1) / self.step + 1 }
        } else {
            if self.current <= self.stop { 0 } else { (self.current - self.stop - 1) / (-self.step) + 1 }
        }
    }

    pub(super) fn __iter__(_vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        Ok(receiver)
    }

    pub(super) fn __next__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let range = vm.obj_heap.get_instance_data::<ObjectRangeIter>(receiver).expect("must be range iter");
        let exhausted = if range.step > 0 { range.current >= range.stop } else { range.current <= range.stop };
        if exhausted {
            return Ok(ObjectHandle::ITER_END);
        }
        let value = vm.obj_heap.alloc_integer_instance(range.current);
       
        let range = vm.obj_heap.get_instance_data_mut::<ObjectRangeIter>(receiver).expect("must be range iter");
        range.current += range.step;

        Ok(value)
    }

    pub(super) fn __len__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let range = vm.obj_heap.get_instance_data::<ObjectRangeIter>(receiver).expect("must be range iter");
        Ok(vm.obj_heap.alloc_integer_instance(range.len()))
    }

    pub(super) fn __str__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let range = vm.obj_heap.get_instance_data::<ObjectRangeIter>(receiver).expect("must be range iter");
        let s = crate::format_shr!("range({}, {}, {})", range.current, range.stop, range.step);
        Ok(vm.obj_heap.alloc_string_instance(s))
    }
}

pub(super) fn range(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    let (start, stop, step) = match args.len() {
        1 => (0, *vm.obj_heap.expect_integer(args[0])?, 1),
        2 => (*vm.obj_heap.expect_integer(args[0])?, *vm.obj_heap.expect_integer(args[1])?, 1),
        3 => {
            let start = *vm.obj_heap.expect_integer(args[0])?;
            let stop = *vm.obj_heap.expect_integer(args[1])?;
            let step = *vm.obj_heap.expect_integer(args[2])?;
            if step == 0 {
                return Err(RuntimeErrorKind::RangeStepZero);
            }
            (start, stop, step)
        }
        _ => Err(RuntimeErrorKind::ArgumentCountRange { min: 1, max: 3, got: args.len() })?
    };

    let class = *vm.builtins.get("RangeIter").expect("must has RangeIter");
    let range_iter = ObjectRangeIter::new(start, stop, step);
    Ok(vm.obj_heap.alloc_instance(class, range_iter))
}