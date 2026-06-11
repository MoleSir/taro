mod variants;
mod heap;
pub use variants::*;
pub use heap::*;
use super::Value;

pub enum Object {
    Function(ObjectFunction),
    BuiltinFn(ObjectBuiltinFn),
    Closure(ObjectClosure),
    Upvalue(ObjectUpvalue),
    Class(ObjectClass),
    Instance(ObjectInstance),
    BoundMethod(ObjectBoundMethod),
}

impl Object {
    pub fn extract_children(&self, out_children: &mut Vec<ObjectHandle>) {
        match self {
            Object::Closure(closure) => {
                out_children.push(closure.function);
                out_children.extend(&closure.upvalues);
            }
            Object::Instance(instance) => {
                out_children.push(instance.klass);
            }
            Object::BoundMethod(bound) => {
                out_children.push(bound.method);
                if let Value::Object(h) = bound.receiver {
                    out_children.push(h);
                }
            }
            _ => {}
        }
    }
}

macro_rules! impl_object_conversions {
    (
        $(
            $variant:ident => {
                ty: $ty:ty,
                as: $as:ident,
                name: $name:literal
            }
        ),* $(,)?
    ) => {
        $(
            impl From<$ty> for Object {
                fn from(value: $ty) -> Self {
                    Object::$variant(value)
                }
            }
        )*

        impl Object {
            pub fn type_name(&self) -> &'static str {
                match self {
                    $(
                        Object::$variant(_) => $name,
                    )*
                }
            }

            $(
                pub fn $as(&self) -> Result<&$ty, ObjectError> {
                    match self {
                        Object::$variant(value) => Ok(value),
                        _ => Err(ObjectError::TypeMismatch {
                            expected: $name,
                            found: self.type_name(),
                        }),
                    }
                }
            )*

            paste::paste! {
                $(
                    pub fn [<$as _mut>](&mut self) -> Result<&mut $ty, ObjectError> {
                        match self {
                            Object::$variant(value) => Ok(value),
                            _ => Err(ObjectError::TypeMismatch {
                                expected: $name,
                                found: self.type_name(),
                            }),
                        }
                    }
                )*
            }
        }
    };
}

impl_object_conversions! {
    Function => { ty: ObjectFunction, as: as_function, name: "Function" },
    BuiltinFn => { ty: ObjectBuiltinFn, as: as_builtin_fn, name: "BuiltinFn" },
    Closure => { ty: ObjectClosure, as: as_closure, name: "Closure" },
    Upvalue => { ty: ObjectUpvalue, as: as_upvalue, name: "Upvalue" },
    Class => { ty: ObjectClass, as: as_class, name: "Class" },
    Instance => { ty: ObjectInstance, as: as_instance, name: "Instance" },
    BoundMethod => { ty: ObjectBoundMethod, as: as_bound_method, name: "BoundMethod" },
}

#[derive(Debug, thiserror::Error)]
pub enum ObjectError {
    #[error("expected {expected}, got {found}")]
    TypeMismatch { expected: &'static str, found: &'static str },
}