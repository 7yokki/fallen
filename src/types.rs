use crate::sema::ClassInfo;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Void,
    Char,
    SChar,
    UChar,
    Short,
    UShort,
    Int,
    UInt,
    Long,
    ULong,
    Ptr(Box<Type>),
    Array(Box<Type>, usize),
    Class(String),
}

impl Type {
    pub fn ptr(inner: Type) -> Type {
        Type::Ptr(Box::new(inner))
    }

    pub fn is_pointer(&self) -> bool {
        matches!(self, Type::Ptr(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Type::Array(_, _))
    }

    pub fn is_aggregate(&self) -> bool {
        matches!(self, Type::Array(_, _) | Type::Class(_))
    }

    pub fn pointee(&self) -> Option<&Type> {
        match self {
            Type::Ptr(t) => Some(t),
            Type::Array(t, _) => Some(t),
            _ => None,
        }
    }

    pub fn decay(&self) -> Type {
        match self {
            Type::Array(t, _) => Type::Ptr(t.clone()),
            other => other.clone(),
        }
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Type::Char
                | Type::SChar
                | Type::UChar
                | Type::Short
                | Type::UShort
                | Type::Int
                | Type::UInt
                | Type::Long
                | Type::ULong
        )
    }

    pub fn is_signed(&self) -> bool {
        matches!(
            self,
            Type::Char | Type::SChar | Type::Short | Type::Int | Type::Long
        )
    }

    pub fn align(&self, classes: &HashMap<String, ClassInfo>) -> usize {
        match self {
            Type::Void => 1,
            Type::Char | Type::SChar | Type::UChar => 1,
            Type::Short | Type::UShort => 2,
            Type::Int | Type::UInt => 4,
            Type::Long | Type::ULong | Type::Ptr(_) => 8,
            Type::Array(elem, _) => elem.align(classes),
            Type::Class(name) => classes
                .get(name)
                .map(|c| c.align)
                .unwrap_or(1),
        }
    }

    pub fn size(&self, classes: &HashMap<String, ClassInfo>) -> usize {
        match self {
            Type::Void => 1,
            Type::Char | Type::SChar | Type::UChar => 1,
            Type::Short | Type::UShort => 2,
            Type::Int | Type::UInt => 4,
            Type::Long | Type::ULong | Type::Ptr(_) => 8,
            Type::Array(elem, n) => elem.size(classes) * n,
            Type::Class(name) => classes.get(name).map(|c| c.size).unwrap_or(1),
        }
    }
}

pub fn align_up(v: usize, a: usize) -> usize {
    if a <= 1 {
        v
    } else {
        (v + a - 1) / a * a
    }
}
