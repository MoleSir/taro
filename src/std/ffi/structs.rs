//! C struct creation and field access.
//!
//! Struct instances carry a dedicated [`Struct`] `ObjectInstanceData` that
//! stores the back-link to the [`CType`] (which carries the [`StructLayout`])
//! and the named field values.  Property access (`.field`) is routed through
//! `__getattr__` / `__setattr__` magic methods registered on the `Struct`
//! class — the VM dispatches to these when the instance data is not
//! `ObjectFields`.

use std::collections::HashMap;

use crate::object::ObjectHeap;
use crate::vm::{RuntimeResult, VirtualMachine};
use crate::{ObjectHandle, ObjectInstanceData, ShrString};
use std::any::Any;

use super::error::FfiError;
use super::types::{CType, struct_layout_from_descriptors};

// ===========================================================================
// Struct — concrete struct instance (named fields + type back-link)
// ===========================================================================

pub(super) struct Struct {
    /// Back-link to the `CType` instance that describes this struct's layout.
    pub(super) ctype: ObjectHandle,
    /// Field values keyed by field name.
    pub(super) fields: HashMap<ShrString, ObjectHandle>,
}

impl ObjectInstanceData for Struct {
    fn mark_references(&self, heap: &mut ObjectHeap) {
        heap.mark_object(self.ctype);
        for (_, &value) in &self.fields {
            heap.mark_object(value);
        }
    }

    fn type_name(&self) -> &'static str {
        "StructInstance"
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Struct {
    pub(super) fn __new__(_vm: &mut VirtualMachine, _args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        Err(FfiError::StructDirectConstruction.into())
    }

    pub(super) fn __getattr__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        if args.len() < 2 {
            return Err(FfiError::GetAttrArgCount.into());
        }
        let self_handle = args[0];
        let field_name = vm.expect_type(vm.obj_heap.get_string_instance(args[1]), args[1], "string")?.as_str().to_string();

        let data = vm.obj_heap.get_native::<Struct>(self_handle).ok_or(FfiError::GetAttrNotStruct)?;

        let key = ShrString::new_string(field_name.as_str());
        data.fields.get(&key).copied().ok_or_else(|| FfiError::StructNoField(field_name).into())
    }

    pub(super) fn __setattr__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        if args.len() < 3 {
            return Err(FfiError::SetAttrArgCount.into());
        }
        let self_handle = args[0];
        let field_name = vm.expect_type(vm.obj_heap.get_string_instance(args[1]), args[1], "string")?.as_str().to_string();
        let value = args[2];

        let data = vm.obj_heap.get_native_mut::<Struct>(self_handle).ok_or(FfiError::SetAttrNotStruct)?;

        data.fields.insert(ShrString::new_string(field_name.as_str()), value);
        Ok(ObjectHandle::NIL)
    }
}

// ===========================================================================
// define_struct — define a C struct type, return a CType instance
// ===========================================================================

pub(super) fn define_struct(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    if args.is_empty() {
        return Err(FfiError::StructDefExpectedList.into());
    }
    let field_descriptors = args[0];
    let descriptors = parse_struct_descriptors(vm, field_descriptors)?;
    let layout = struct_layout_from_descriptors(&descriptors)?;
    let ctype_instance = CType::Struct(layout);

    let class = vm.lookup_loaded_module_export("std/ffi", &ShrString::new_str("CType")).ok_or(FfiError::CTypeClassNotFound)?;
    Ok(vm.obj_heap.alloc_instance(class, ctype_instance))
}

// ===========================================================================
// parse_struct_descriptors — parse field descriptor list
// ===========================================================================

fn parse_struct_descriptors(vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Vec<(String, CType)>> {
    let items = vm.obj_heap.get_list_instance(handle).ok_or(FfiError::StructDefInvalidFormat)?;

    if items.is_empty() {
        return Err(FfiError::StructDefEmptyList.into());
    }

    let is_named = vm.obj_heap.get_list_instance(items[0]).is_some();

    let mut descriptors = Vec::with_capacity(items.len());

    if is_named {
        for (i, &item) in items.iter().enumerate() {
            let pair = vm.obj_heap.get_list_instance(item).ok_or(FfiError::StructDefExpectedPair(i))?;
            if pair.len() != 2 {
                return Err(FfiError::StructDefPairLen(pair.len(), i).into());
            }
            let name = vm.obj_heap.get_string_instance(pair[0]).ok_or(FfiError::StructDefNameNotString(i))?;
            let ct = CType::from_handle(vm, pair[1]).map_err(|e| FfiError::StructDefInvalidType { pos: i, reason: e.to_string() })?;
            descriptors.push((name.as_str().to_string(), ct));
        }
    } else {
        for (i, &item) in items.iter().enumerate() {
            let ct = CType::from_handle(vm, item).map_err(|e| FfiError::StructDefInvalidType { pos: i, reason: e.to_string() })?;
            descriptors.push((i.to_string(), ct));
        }
    }

    Ok(descriptors)
}
