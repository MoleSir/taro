//! C struct definition, creation, and marshalling.
//!
//! Struct instances use `ObjectFields` for storage, which enables native
//! `.field` property access via the VM's existing GetProperty path — no VM
//! changes needed.  A hidden `__struct_def__` field stores a back-link to the
//! [`StructDef`] so that FFI marshalling can recover type metadata and rebuild
//! the raw byte buffer on demand.

use std::alloc::Layout;
use std::any::Any;
use std::collections::HashMap;

use crate::object::ObjectFields;
use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{impl_object_instance_data, ObjectHandle, ObjectInstanceData, ShrString};

use super::types::CType;

// ===========================================================================
// StructDef — C struct layout descriptor
// ===========================================================================

pub(super) struct StructDef {
    /// Field types in layout order.
    pub(super) field_types: Vec<CType>,
    /// Field names in layout order.
    pub(super) field_names: Vec<String>,
    /// Byte offset of each field from the start of the struct.
    pub(super) offsets: Vec<usize>,
    /// Total size of the struct (including tail padding).
    pub(super) size: usize,
    #[allow(dead_code)]
    pub(super) alignment: usize,
}

impl StructDef {
    pub(super) fn __new__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        let class = args[0];
        let field_descriptors = args[1];
        let descriptors = parse_struct_descriptors(vm, field_descriptors)?;
        let def = StructDef::from_descriptors(&descriptors)?;
        Ok(vm.obj_heap.alloc_instance(class, def))
    }

    /// Build a struct layout from field descriptors.
    ///
    /// Each descriptor is either a plain type string (unnamed field, name is
    /// the index) or a `(name, type)` pair.
    fn from_descriptors(descriptors: &[(String, String)]) -> RuntimeResult<Self> {
        let mut field_types = Vec::with_capacity(descriptors.len());
        let mut field_names = Vec::with_capacity(descriptors.len());
        let mut offsets = Vec::with_capacity(descriptors.len());

        let mut layout = Layout::from_size_align(0, 1).map_err(|_| RuntimeErrorKind::FfiError("invalid initial layout".into()))?;

        for (name, type_str) in descriptors {
            let ct = CType::from_str(type_str)?;
            let (size, align) = ct.size_align()?;
            let field_layout = Layout::from_size_align(size, align).map_err(|_| {
                RuntimeErrorKind::FfiError(format!("invalid layout for field '{name}' (type={type_str}, size={size}, align={align})"))
            })?;
            let (new_layout, offset) =
                layout.extend(field_layout).map_err(|_| RuntimeErrorKind::FfiError("struct exceeds maximum supported size".into()))?;
            layout = new_layout;
            offsets.push(offset);
            field_types.push(ct);
            field_names.push(name.clone());
        }

        let total_layout = layout.pad_to_align();
        Ok(Self { field_types, field_names, offsets, size: total_layout.size(), alignment: total_layout.align() })
    }
}

impl_object_instance_data!(StructDef, "StructDef");

// ===========================================================================
// struct_def — define a C struct layout
// ===========================================================================
//
// Accepts two formats:
//
//   1. Positional list:        ffi.struct_def(["uint8", "uint8", "uint8"])
//      → fields named "0", "1", "2"
//
//   2. Named-pair list:        ffi.struct_def([["r","uint8"], ["g","uint8"]])
//      → fields named "r", "g" (order preserved)

pub(super) fn struct_def(vm: &mut VirtualMachine, field_descriptors: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let descriptors = parse_struct_descriptors(vm, field_descriptors)?;
    let def = StructDef::from_descriptors(&descriptors)?;

    let class = vm
        .lookup_loaded_module_export("std/ffi", &ShrString::new_str("__StructDef__"))
        .ok_or_else(|| RuntimeErrorKind::FfiError("StructDef class not found in ffi module".into()))?;
    Ok(vm.obj_heap.alloc_instance(class, def))
}

/// Parse the `struct_def` argument into `(name, type)` pairs.
fn parse_struct_descriptors(vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Vec<(String, String)>> {
    let items = vm
        .get_list_instance(handle)
        .map_err(|_| RuntimeErrorKind::FfiError("struct_def: expected a list of types or a list of [name, type] pairs".into()))?;

    if items.is_empty() {
        return Err(RuntimeErrorKind::FfiError("struct_def: field list must not be empty".into()));
    }

    // Detect format: if the first element is itself a list, treat as named
    // pairs; otherwise treat as positional type strings.
    let is_named = vm.get_list_instance(items[0]).is_ok();

    let mut descriptors = Vec::with_capacity(items.len());

    if is_named {
        for (i, &item) in items.iter().enumerate() {
            let pair = vm
                .get_list_instance(item)
                .map_err(|_| RuntimeErrorKind::FfiError(format!("struct_def: expected [name, type] pair at position {i}")))?;
            if pair.len() != 2 {
                return Err(RuntimeErrorKind::FfiError(format!(
                    "struct_def: each pair must be [name, type], got {} elements at position {i}",
                    pair.len()
                )));
            }
            let name = vm
                .get_string_instance(pair[0])
                .map_err(|_| RuntimeErrorKind::FfiError(format!("struct_def: field name at position {i} must be a string")))?;
            let type_str = vm
                .get_string_instance(pair[1])
                .map_err(|_| RuntimeErrorKind::FfiError(format!("struct_def: field type at position {i} must be a string")))?;
            descriptors.push((name.as_str().to_string(), type_str.as_str().to_string()));
        }
    } else {
        for (i, &item) in items.iter().enumerate() {
            let type_str = vm
                .get_string_instance(item)
                .map_err(|_| RuntimeErrorKind::FfiError(format!("struct_def: expected type string at position {i}")))?;
            descriptors.push((i.to_string(), type_str.as_str().to_string()));
        }
    }

    Ok(descriptors)
}

// ===========================================================================
// struct_new — create a struct instance (positional values)
// ===========================================================================

pub(super) fn struct_new(vm: &mut VirtualMachine, def_handle: ObjectHandle, values_list: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let (field_types, field_names) = {
        let def = vm
            .obj_heap
            .get_native::<StructDef>(def_handle)
            .ok_or_else(|| RuntimeErrorKind::FfiError("struct_new: first argument must be a struct def".into()))?;
        (def.field_types.clone(), def.field_names.clone())
    };

    let value_handles: Vec<ObjectHandle> = vm
        .get_list_instance(values_list)
        .map_err(|_| RuntimeErrorKind::FfiError("struct_new: second argument must be a list of values".into()))?
        .clone();

    if value_handles.len() != field_types.len() {
        return Err(RuntimeErrorKind::FfiError(format!("struct_new: expected {} values, got {}", field_types.len(), value_handles.len())));
    }

    build_struct_instance(vm, def_handle, &field_names, &field_types, &value_handles)
}

// ===========================================================================
// StructDef.__call__ — create struct via Color(r, g, b, a) syntax
// ===========================================================================

pub(super) fn struct_def_call(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
    if args.is_empty() {
        return Err(RuntimeErrorKind::FfiError("struct call: missing self".into()));
    }

    let self_handle = args[0];
    let field_values = &args[1..];

    let (field_types, field_names) = {
        let def = vm
            .obj_heap
            .get_native::<StructDef>(self_handle)
            .ok_or_else(|| RuntimeErrorKind::FfiError("struct call: self is not a StructDef".into()))?;
        (def.field_types.clone(), def.field_names.clone())
    };

    if field_values.len() != field_types.len() {
        return Err(RuntimeErrorKind::FfiError(format!("struct expects {} value(s), got {}", field_types.len(), field_values.len())));
    }

    build_struct_instance(vm, self_handle, &field_names, &field_types, field_values)
}

// ===========================================================================
// Shared: build an ObjectFields-based struct instance
// ===========================================================================

pub(super) struct Struct {
    pub(super) struct_def: ObjectHandle,
    pub(super) fields: HashMap<ShrString, ObjectHandle>,
}

impl ObjectInstanceData for Struct {
    fn type_name(&self) -> &'static str {
        "StructInstance"
    }

    fn mark_references(&self, heap: &mut crate::ObjectHeap) {
        heap.mark_object(self.struct_def);
    }

        fn as_any_ref(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn build_struct_instance(
    vm: &mut VirtualMachine,
    def_handle: ObjectHandle,
    field_names: &[String],
    field_types: &[CType],
    values: &[ObjectHandle],
) -> RuntimeResult<ObjectHandle> {
    let mut fields: HashMap<ShrString, ObjectHandle> = HashMap::with_capacity(field_names.len() + 1);

    for (i, name) in field_names.iter().enumerate() {
        let value = values[i];
        let _ = (field_types, value);
        fields.insert(ShrString::new_string(name.as_str()), value);
    }

    // Back-link to the StructDef so FFI marshalling can recover type info.
    fields.insert(ShrString::new_str("__struct_def__"), def_handle);

    let instance_data = ObjectFields { fields };
    let class = vm
        .lookup_module_export(def_handle, &ShrString::new_str("__Struct__"))
        .ok_or_else(|| RuntimeErrorKind::FfiError("Struct class not found in ffi module".into()))?;
    Ok(vm.obj_heap.alloc_instance(class, instance_data))
}
