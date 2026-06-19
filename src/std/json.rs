use std::collections::HashMap;
use crate::{NativeFunction, ObjectHandle, ObjectInstanceData, ShrString};
use crate::object::ObjectDict;
use crate::vm::{RuntimeResult, RuntimeErrorKind, VirtualMachine};

impl VirtualMachine {
    /// Create the `json` std module.
    ///
    /// # Exports
    ///
    /// | function         | description                              |
    /// |------------------|------------------------------------------|
    /// | `encode(value)`  | serialize a Taro value to a JSON string  |
    /// | `decode(string)` | parse a JSON string into a Taro value    |
    pub(crate) fn create_json_module(&mut self) -> RuntimeResult<ObjectHandle> {
        let encode_fn = self.obj_heap.alloc_native_fn("encode", NativeFunction::a1(encode));
        let decode_fn = self.obj_heap.alloc_native_fn("decode", NativeFunction::a1(decode));

        let mut exports: HashMap<ShrString, ObjectHandle> = HashMap::new();
        exports.insert(ShrString::new_str("encode"), encode_fn);
        exports.insert(ShrString::new_str("decode"), decode_fn);

        let module = self.obj_heap.alloc_fields_instance(self.obj_heap.module_class, exports);
        Ok(module)
    }
}

// =====================================================================
//  encode — Taro value → JSON string
// =====================================================================

/// `json.encode(value)` — serialize a Taro value to a JSON string.
///
/// # Supported types
///
/// | Taro type | JSON output        |
/// |-----------|--------------------|
/// | nil       | `null`             |
/// | Bool      | `true` / `false`   |
/// | Int       | number             |
/// | Float     | number             |
/// | String    | string             |
/// | List      | `[...]`            |
/// | Dict      | `{...}`            |
/// | Set       | `[...]` (as array) |
///
/// # Errors
///
/// Returns an error when the value (or any nested value) is not a
/// plain-data type (e.g. a class instance, function, or native object).
fn encode(vm: &mut VirtualMachine, value: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let mut out = String::new();
    encode_value(vm, value, &mut out)?;
    Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(&out)))
}

fn encode_value(vm: &VirtualMachine, handle: ObjectHandle, out: &mut String) -> RuntimeResult<()> {
    if handle.is_nil() {
        out.push_str("null");
        return Ok(());
    }

    let inst = match vm.obj_heap.get_instance(handle) {
        Some(inst) => inst,
        None => {
            return Err(encode_error(vm.value_type_name(handle)));
        }
    };

    match &inst.data {
        ObjectInstanceData::Nil => out.push_str("null"),

        ObjectInstanceData::Bool(b) => {
            out.push_str(if *b { "true" } else { "false" });
        }

        ObjectInstanceData::Integer(i) => {
            use std::fmt::Write;
            write!(out, "{}", i).unwrap();
        }

        ObjectInstanceData::Float(f) => {
            if f.is_nan() || f.is_infinite() {
                // JSON doesn't support NaN / Infinity — encode as null.
                out.push_str("null");
            } else {
                use std::fmt::Write;
                write!(out, "{}", f).unwrap();
            }
        }

        ObjectInstanceData::String(s) => {
            encode_json_string(s.as_str(), out);
        }

        ObjectInstanceData::List(items) => {
            out.push('[');
            for (i, &item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                encode_value(vm, item, out)?;
            }
            out.push(']');
        }

        ObjectInstanceData::Dict(entries) => {
            out.push('{');
            let mut first = true;
            for bucket in entries.values() {
                for &(k, v) in bucket {
                    // Only string keys are valid in JSON.
                    let key_str = match vm.obj_heap.get_instance(k) {
                        Some(ki) => match &ki.data {
                            ObjectInstanceData::String(s) => s.clone(),
                            _ => return Err(RuntimeErrorKind::JosnError(
                                "json.encode: dict keys must be strings".into()
                            )),
                        },
                        None => return Err(RuntimeErrorKind::JosnError(
                            "json.encode: dict keys must be strings".into()
                        )),
                    };

                    if !first { out.push(','); }
                    first = false;

                    encode_json_string(key_str.as_str(), out);
                    out.push(':');
                    encode_value(vm, v, out)?;
                }
            }
            out.push('}');
        }

        ObjectInstanceData::Set(entries) => {
            // Encode set as a JSON array (order not guaranteed).
            out.push('[');
            let mut first = true;
            for bucket in entries.values() {
                for &item in bucket {
                    if !first { out.push(','); }
                    first = false;
                    encode_value(vm, item, out)?;
                }
            }
            out.push(']');
        }

        // Fields / Native / IterEnd — not serializable.
        _ => return Err(encode_error(vm.value_type_name(handle))),
    }

    Ok(())
}

fn encode_error(type_name: &str) -> RuntimeErrorKind {
    RuntimeErrorKind::JosnError(format!(
        "json.encode: cannot serialize type '{type_name}'"
    ))
}

/// Write a Rust string slice as a JSON-escaped string (including the
/// surrounding double-quotes).
fn encode_json_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"'  => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                use std::fmt::Write;
                write!(out, "\\u{:04x}", c as u32).unwrap();
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

// =====================================================================
//  decode — JSON string → Taro value
// =====================================================================

/// `json.decode(string)` — parse a JSON string into a Taro value.
///
/// JSON types map to Taro as follows:
///   null   → nil
///   bool   → Bool
///   number → Int (if integral) or Float
///   string → String
///   array  → List
///   object → Dict (string keys)
fn decode(vm: &mut VirtualMachine, text: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let s = vm.get_string_instance(text)?;

    let value: serde_json::Value = serde_json::from_str(s.as_str())
        .map_err(|e| RuntimeErrorKind::JosnError(format!("json.decode: {e}")))?;

    json_value_to_taro(vm, &value)
}

fn json_value_to_taro(vm: &mut VirtualMachine, v: &serde_json::Value) -> RuntimeResult<ObjectHandle> {
    match v {
        serde_json::Value::Null => Ok(ObjectHandle::NIL),

        serde_json::Value::Bool(b) => Ok(vm.obj_heap.alloc_bool_instance(*b)),

        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(vm.obj_heap.alloc_integer_instance(i))
            } else if let Some(f) = n.as_f64() {
                Ok(vm.obj_heap.alloc_float_instance(f))
            } else {
                Err(RuntimeErrorKind::JosnError(format!(
                    "json.decode: unsupported number '{n}'"
                )))
            }
        }

        serde_json::Value::String(s) => {
            Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(s)))
        }

        serde_json::Value::Array(arr) => {
            let items: Vec<ObjectHandle> = arr
                .iter()
                .map(|v| json_value_to_taro(vm, v))
                .collect::<RuntimeResult<_>>()?;
            Ok(vm.obj_heap.alloc_list_instance(items))
        }

        serde_json::Value::Object(map) => {
            let dict = vm.obj_heap.alloc_dict_instance(HashMap::new());
            for (k, v) in map {
                let key = vm.obj_heap.alloc_string_instance(ShrString::new_string(k));
                let val = json_value_to_taro(vm, v)?;
                ObjectDict::__setitem__(vm, dict, key, val)?;
            }
            Ok(dict)
        }
    }
}
