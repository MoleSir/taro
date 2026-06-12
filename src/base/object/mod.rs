mod variants;
mod heap;
pub use variants::*;
pub use heap::*;

pub enum Object {
    Function(ObjectFunction),
    BuiltinFn(ObjectBuiltinFn),
    Closure(ObjectClosure),
    Upvalue(ObjectUpvalue),
    Class(ObjectClass),
    Instance(ObjectInstance),
    BoundMethod(ObjectBoundMethod),
    List(ObjectList),
}

macro_rules! impl_object_conversions {
    (
        $(
            $variant:ident => {
                ty: $ty:ty,
                method: $method:ident,
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

            paste::paste! {
                $(
                    pub fn [<as_ $method>](&self) -> Result<&$ty, ObjectError> {
                        match self {
                            Object::$variant(value) => Ok(value),
                            _ => Err(ObjectError::TypeMismatch {
                                expected: $name,
                                found: self.type_name(),
                            }),
                        }
                    }
                )*

                $(
                    pub fn [<as_ $method _mut>](&mut self) -> Result<&mut $ty, ObjectError> {
                        match self {
                            Object::$variant(value) => Ok(value),
                            _ => Err(ObjectError::TypeMismatch {
                                expected: $name,
                                found: self.type_name(),
                            }),
                        }
                    }
                )*

                $(
                    pub fn [<is_ $method>](&self) -> bool {
                        match self {
                            Object::$variant(_) => true,
                            _ => false,
                        }
                    }
                )*
            }
        }
    };
}

impl_object_conversions! {
    Function => { ty: ObjectFunction, method: function, name: "Function" },
    BuiltinFn => { ty: ObjectBuiltinFn, method: builtin_fn, name: "BuiltinFn" },
    Closure => { ty: ObjectClosure, method: closure, name: "Closure" },
    Upvalue => { ty: ObjectUpvalue, method: upvalue, name: "Upvalue" },
    Class => { ty: ObjectClass, method: class, name: "Class" },
    Instance => { ty: ObjectInstance, method: instance, name: "Instance" },
    BoundMethod => { ty: ObjectBoundMethod, method: bound_method, name: "BoundMethod" },
    List => { ty: ObjectList, method: list, name: "List" },
}

#[derive(Debug, thiserror::Error)]
pub enum ObjectError {
    #[error("expected {expected}, got {found}")]
    TypeMismatch { expected: &'static str, found: &'static str },
}