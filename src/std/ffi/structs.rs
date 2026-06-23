//! C struct definition, creation, and field access.
//!
//! Struct instances carry a dedicated [`Struct`] `ObjectInstanceData` that
//! stores the back-link to the [`StructDef`] and the named field values.
//! Property access (`.field`) is routed through `__getattr__` / `__setattr__`
//! magic methods registered on the `Struct` class — the VM dispatches to
//! these when the instance data is not `ObjectFields`.

use std::alloc::Layout;
use std::any::Any;
use std::collections::HashMap;

use crate::object::ObjectHeap;
use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{ObjectHandle, ObjectInstanceData, ShrString};

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

impl ObjectInstanceData for StructDef {
    fn mark_references(&self, _heap: &mut ObjectHeap) {}
    fn type_name(&self) -> &'static str {
        "StructDef"
    }
    fn as_any_ref(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl StructDef {
    pub(super) fn __new__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        let class = args[0];
        let field_descriptors = args[1];
        let descriptors = parse_struct_descriptors(vm, field_descriptors)?;
        let def = StructDef::from_descriptors(&descriptors)?;
        Ok(vm.obj_heap.alloc_instance(class, def))
    }

    pub(super) fn __init__(_vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        Ok(args[0])
    }

    pub(super) fn __call__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        if args.is_empty() {
            return Err(RuntimeErrorKind::FfiError("struct call: missing self".into()));
        }
    
        let self_handle = args[0];
        let field_values = &args[1..];
        let struct_def = vm.expect_type(vm.obj_heap.get_native::<StructDef>(self_handle), self_handle, "native")?;
    
        if field_values.len() != struct_def.field_types.len() {
            return Err(RuntimeErrorKind::FfiError(format!("struct expects {} value(s), got {}", struct_def.field_types.len(), field_values.len())));
        }

        let mut fields = HashMap::with_capacity(struct_def.field_names.len());
        for (i, name) in struct_def.field_names.iter().enumerate() {
            let value = field_values[i];
            fields.insert(ShrString::new_string(name.as_str()), value);
        }

        let instance_data = Struct { struct_def: self_handle, fields };

        let class = vm.lookup_module_export(self_handle, &ShrString::new_str("Struct")).expect("must exit");
        Ok(vm.obj_heap.alloc_instance(class, instance_data))
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

// ===========================================================================
// Struct — concrete struct instance (named fields + def back-link)
// ===========================================================================

pub(super) struct Struct {
    pub(super) struct_def: ObjectHandle,
    pub(super) fields: HashMap<ShrString, ObjectHandle>,
}

impl ObjectInstanceData for Struct {
    fn mark_references(&self, heap: &mut ObjectHeap) {
        heap.mark_object(self.struct_def);
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
        Err(RuntimeErrorKind::FfiError("Struct cannot be constructed directly; use ffi.struct_new()".into()))
    }

    pub(super) fn __getattr__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        // args: [self, field_name_string]
        if args.len() < 2 {
            return Err(RuntimeErrorKind::FfiError("__getattr__ requires 2 arguments (self, name)".into()));
        }
        let self_handle = args[0];
        let field_name = vm.expect_type(vm.obj_heap.get_string_instance(args[1]), args[1], "string")?.as_str().to_string();

        let data = vm
            .obj_heap
            .get_native::<Struct>(self_handle)
            .ok_or_else(|| RuntimeErrorKind::FfiError("__getattr__: not a struct instance".into()))?;

        let key = ShrString::new_string(field_name.as_str());
        data.fields.get(&key).copied().ok_or_else(|| RuntimeErrorKind::FfiError(format!("struct has no field '{field_name}'")))
    }

    pub(super) fn __setattr__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        // args: [self, field_name_string, value]
        if args.len() < 3 {
            return Err(RuntimeErrorKind::FfiError("__setattr__ requires 3 arguments (self, name, value)".into()));
        }
        let self_handle = args[0];
        let field_name = vm.expect_type(vm.obj_heap.get_string_instance(args[1]), args[1], "string")?.as_str().to_string();
        let value = args[2];

        let data = vm
            .obj_heap
            .get_native_mut::<Struct>(self_handle)
            .ok_or_else(|| RuntimeErrorKind::FfiError("__setattr__: not a struct instance".into()))?;

        data.fields.insert(ShrString::new_string(field_name.as_str()), value);
        Ok(ObjectHandle::NIL)
    }
}

// ===========================================================================
// struct_def — define a C struct layout
// ===========================================================================


/// Parse the `struct_def` argument into `(name, type)` pairs.
fn parse_struct_descriptors(vm: &VirtualMachine, handle: ObjectHandle) -> RuntimeResult<Vec<(String, String)>> {
    let items = vm
        .obj_heap.get_list_instance(handle)
        .ok_or_else(|| RuntimeErrorKind::FfiError("struct_def: expected a list of types or a list of [name, type] pairs".into()))?;

    if items.is_empty() {
        return Err(RuntimeErrorKind::FfiError("struct_def: field list must not be empty".into()));
    }

    // Detect format: if the first element is itself a list, treat as named
    // pairs; otherwise treat as positional type strings.
    let is_named = vm.obj_heap.get_list_instance(items[0]).is_some();

    let mut descriptors = Vec::with_capacity(items.len());

    if is_named {
        for (i, &item) in items.iter().enumerate() {
            let pair = vm
                .obj_heap.get_list_instance(item)
                .ok_or_else(|| RuntimeErrorKind::FfiError(format!("struct_def: expected [name, type] pair at position {i}")))?;
            if pair.len() != 2 {
                return Err(RuntimeErrorKind::FfiError(format!(
                    "struct_def: each pair must be [name, type], got {} elements at position {i}",
                    pair.len()
                )));
            }
            let name = vm
                .obj_heap.get_string_instance(pair[0])
                .ok_or_else(|| RuntimeErrorKind::FfiError(format!("struct_def: field name at position {i} must be a string")))?;
            let type_str = vm
                .obj_heap.get_string_instance(pair[1])
                .ok_or_else(|| RuntimeErrorKind::FfiError(format!("struct_def: field type at position {i} must be a string")))?;
            descriptors.push((name.as_str().to_string(), type_str.as_str().to_string()));
        }
    } else {
        for (i, &item) in items.iter().enumerate() {
            let type_str = vm
                .obj_heap.get_string_instance(item)
                .ok_or_else(|| RuntimeErrorKind::FfiError(format!("struct_def: expected type string at position {i}")))?;
            descriptors.push((i.to_string(), type_str.as_str().to_string()));
        }
    }

    Ok(descriptors)
}
