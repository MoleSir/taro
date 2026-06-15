use crate::ObjectHandle;
use crate::vm::{VirtualMachine, ExecuteError};
use super::ExecuteResult;

// ==================================================================== //
//               Get args
// ==================================================================== //
impl VirtualMachine {
    pub fn get_args(&self, actual_args: usize) -> &[ObjectHandle] {
        assert!(self.stack.len() >= actual_args);
        &self.stack[self.stack.len() - actual_args..]
    }

    pub fn get_n_args<const N: usize>(&self, actual_args: usize) -> ExecuteResult<[ObjectHandle; N]> {
        if actual_args != N { Err(ExecuteError::ArgumentCountMismatch { expected: N, got: actual_args })? }
        let args = self.get_args(actual_args);
        assert_eq!(N, actual_args);
        let arr: [ObjectHandle; N] = std::array::from_fn(|i| args[i].clone());
        Ok(arr)
    }

    #[inline]
    pub fn get_0_args(&self, actual_args: usize) -> ExecuteResult<()> {
        self.get_n_args::<0>(actual_args).map(|_| ())
    }

    #[inline]
    pub fn get_1_args(&self, actual_args: usize) -> ExecuteResult<ObjectHandle> {
        self.get_n_args::<1>(actual_args).map(|args| args[0])
    }

    #[inline]
    pub fn get_2_args(&self, actual_args: usize) -> ExecuteResult<(ObjectHandle, ObjectHandle)> {
        self.get_n_args::<2>(actual_args).map(|args| (args[0], args[1]))
    }

    #[inline]
    pub fn get_3_args(&self, actual_args: usize) -> ExecuteResult<(ObjectHandle, ObjectHandle, ObjectHandle)> {
        self.get_n_args::<3>(actual_args).map(|args| (args[0], args[1], args[2]))
    }

    #[inline]
    pub fn get_4_args(&self, actual_args: usize) -> ExecuteResult<(ObjectHandle, ObjectHandle, ObjectHandle, ObjectHandle)> {
        self.get_n_args::<4>(actual_args).map(|args| (args[0], args[1], args[2], args[3]))
    }

    #[inline]
    pub fn get_5_args(&self, actual_args: usize) -> ExecuteResult<(ObjectHandle, ObjectHandle, ObjectHandle, ObjectHandle, ObjectHandle)> {
        self.get_n_args::<5>(actual_args).map(|args| (args[0], args[1], args[2], args[3], args[4]))
    }
}
