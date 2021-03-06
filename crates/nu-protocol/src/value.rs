pub mod column_path;
mod convert;
mod debug;
pub mod dict;
pub mod evaluate;
pub mod primitive;
pub mod range;
mod serde_bigdecimal;
mod serde_bigint;

use crate::type_name::{ShellTypeName, SpannedTypeName};
use crate::value::dict::Dictionary;
use crate::value::evaluate::Evaluate;
use crate::value::primitive::Primitive;
use crate::value::range::{Range, RangeInclusion};
use crate::{ColumnPath, PathMember};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use nu_errors::ShellError;
use nu_source::{AnchorLocation, HasSpan, Span, Spanned, Tag};
use num_bigint::BigInt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

/// The core structured values that flow through a pipeline
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Serialize, Deserialize)]
pub enum UntaggedValue {
    /// A primitive (or fundamental) type of values
    Primitive(Primitive),
    /// A table row
    Row(Dictionary),
    /// A full inner (or embedded) table
    Table(Vec<Value>),

    /// An error value that represents an error that occurred as the values in the pipeline were built
    Error(ShellError),

    /// A block of Nu code, eg `{ ls | get name }`
    Block(Evaluate),
}

impl UntaggedValue {
    /// Tags an UntaggedValue so that it can become a Value
    pub fn retag(self, tag: impl Into<Tag>) -> Value {
        Value {
            value: self,
            tag: tag.into(),
        }
    }

    /// Get the corresponding descriptors (column names) associated with this value
    pub fn data_descriptors(&self) -> Vec<String> {
        match self {
            UntaggedValue::Primitive(_) => vec![],
            UntaggedValue::Row(columns) => columns.entries.keys().map(|x| x.to_string()).collect(),
            UntaggedValue::Block(_) => vec![],
            UntaggedValue::Table(_) => vec![],
            UntaggedValue::Error(_) => vec![],
        }
    }

    /// Convert this UntaggedValue to a Value with the given Tag
    pub fn into_value(self, tag: impl Into<Tag>) -> Value {
        Value {
            value: self,
            tag: tag.into(),
        }
    }

    /// Convert this UntaggedValue into a Value with an empty Tag
    pub fn into_untagged_value(self) -> Value {
        Value {
            value: self,
            tag: Tag::unknown(),
        }
    }

    /// Returns true if this value represents boolean true
    pub fn is_true(&self) -> bool {
        match self {
            UntaggedValue::Primitive(Primitive::Boolean(true)) => true,
            _ => false,
        }
    }

    /// Returns true if the value represents something other than Nothing
    pub fn is_some(&self) -> bool {
        !self.is_none()
    }

    /// Returns true if the value represents Nothing
    pub fn is_none(&self) -> bool {
        match self {
            UntaggedValue::Primitive(Primitive::Nothing) => true,
            _ => false,
        }
    }

    /// Returns true if the value represents an error
    pub fn is_error(&self) -> bool {
        match self {
            UntaggedValue::Error(_err) => true,
            _ => false,
        }
    }

    /// Expect this value to be an error and return it
    pub fn expect_error(&self) -> ShellError {
        match self {
            UntaggedValue::Error(err) => err.clone(),
            _ => panic!("Don't call expect_error without first calling is_error"),
        }
    }

    /// Expect this value to be a string and return it
    pub fn expect_string(&self) -> &str {
        match self {
            UntaggedValue::Primitive(Primitive::String(string)) => &string[..],
            _ => panic!("expect_string assumes that the value must be a string"),
        }
    }

    /// Helper for creating row values
    pub fn row(entries: IndexMap<String, Value>) -> UntaggedValue {
        UntaggedValue::Row(entries.into())
    }

    /// Helper for creating table values
    pub fn table(list: &[Value]) -> UntaggedValue {
        UntaggedValue::Table(list.to_vec())
    }

    /// Helper for creating string values
    pub fn string(s: impl Into<String>) -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::String(s.into()))
    }

    /// Helper for creating line values
    pub fn line(s: impl Into<String>) -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::Line(s.into()))
    }

    /// Helper for creating column-path values
    pub fn column_path(s: Vec<impl Into<PathMember>>) -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::ColumnPath(ColumnPath::new(
            s.into_iter().map(|p| p.into()).collect(),
        )))
    }

    /// Helper for creating integer values
    pub fn int(i: impl Into<BigInt>) -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::Int(i.into()))
    }

    /// Helper for creating glob pattern values
    pub fn pattern(s: impl Into<String>) -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::String(s.into()))
    }

    /// Helper for creating filepath values
    pub fn path(s: impl Into<PathBuf>) -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::Path(s.into()))
    }

    /// Helper for creating bytesize values
    pub fn bytes(s: impl Into<u64>) -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::Bytes(s.into()))
    }

    /// Helper for creating decimal values
    pub fn decimal(s: impl Into<BigDecimal>) -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::Decimal(s.into()))
    }

    /// Helper for creating binary (non-text) buffer values
    pub fn binary(binary: Vec<u8>) -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::Binary(binary))
    }

    /// Helper for creating range values
    pub fn range(
        left: (Spanned<Primitive>, RangeInclusion),
        right: (Spanned<Primitive>, RangeInclusion),
    ) -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::Range(Box::new(Range::new(left, right))))
    }

    /// Helper for creating boolean values
    pub fn boolean(s: impl Into<bool>) -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::Boolean(s.into()))
    }

    /// Helper for creating date duration values
    pub fn duration(secs: u64) -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::Duration(secs))
    }

    /// Helper for creating datatime values
    pub fn system_date(s: SystemTime) -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::Date(s.into()))
    }

    pub fn date(d: impl Into<DateTime<Utc>>) -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::Date(d.into()))
    }

    /// Helper for creating the Nothing value
    pub fn nothing() -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::Nothing)
    }
}

/// The fundamental structured value that flows through the pipeline, with associated metadata
#[derive(Debug, Clone, PartialOrd, PartialEq, Ord, Eq, Hash, Serialize, Deserialize)]
pub struct Value {
    pub value: UntaggedValue,
    pub tag: Tag,
}

/// Overload deferencing to give back the UntaggedValue inside of a Value
impl std::ops::Deref for Value {
    type Target = UntaggedValue;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl Value {
    /// Get the corresponding anchor (originating location) for the Value
    pub fn anchor(&self) -> Option<AnchorLocation> {
        self.tag.anchor()
    }

    /// Get the name (url, filepath, etc) behind an anchor for the Value
    pub fn anchor_name(&self) -> Option<String> {
        self.tag.anchor_name()
    }

    /// Get the metadata for the Value
    pub fn tag(&self) -> Tag {
        self.tag.clone()
    }

    /// View the Value as a string, if possible
    pub fn as_string(&self) -> Result<String, ShellError> {
        match &self.value {
            UntaggedValue::Primitive(Primitive::String(string)) => Ok(string.clone()),
            UntaggedValue::Primitive(Primitive::Line(line)) => Ok(line.clone() + "\n"),
            _ => Err(ShellError::type_error("string", self.spanned_type_name())),
        }
    }

    /// View into the borrowed string contents of a Value, if possible
    pub fn as_forgiving_string(&self) -> Result<&str, ShellError> {
        match &self.value {
            UntaggedValue::Primitive(Primitive::String(string)) => Ok(&string[..]),
            _ => Err(ShellError::type_error("string", self.spanned_type_name())),
        }
    }

    /// View the Value as a path, if possible
    pub fn as_path(&self) -> Result<PathBuf, ShellError> {
        match &self.value {
            UntaggedValue::Primitive(Primitive::Path(path)) => Ok(path.clone()),
            UntaggedValue::Primitive(Primitive::String(path_str)) => Ok(PathBuf::from(&path_str)),
            _ => Err(ShellError::type_error("Path", self.spanned_type_name())),
        }
    }

    /// View the Value as a Primitive value, if possible
    pub fn as_primitive(&self) -> Result<Primitive, ShellError> {
        match &self.value {
            UntaggedValue::Primitive(primitive) => Ok(primitive.clone()),
            _ => Err(ShellError::type_error(
                "Primitive",
                self.spanned_type_name(),
            )),
        }
    }

    /// View the Value as unsigned 64-bit, if possible
    pub fn as_u64(&self) -> Result<u64, ShellError> {
        match &self.value {
            UntaggedValue::Primitive(primitive) => primitive.as_u64(self.tag.span),
            _ => Err(ShellError::type_error("integer", self.spanned_type_name())),
        }
    }

    /// View the Value as boolean, if possible
    pub fn as_bool(&self) -> Result<bool, ShellError> {
        match &self.value {
            UntaggedValue::Primitive(Primitive::Boolean(p)) => Ok(*p),
            _ => Err(ShellError::type_error("boolean", self.spanned_type_name())),
        }
    }
}

impl Into<Value> for String {
    fn into(self) -> Value {
        let end = self.len();
        Value {
            value: self.into(),
            tag: Tag {
                anchor: None,
                span: Span::new(0, end),
            },
        }
    }
}

impl Into<UntaggedValue> for &str {
    /// Convert a string slice into an UntaggedValue
    fn into(self) -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::String(self.to_string()))
    }
}

impl Into<UntaggedValue> for Value {
    /// Convert a Value into an UntaggedValue
    fn into(self) -> UntaggedValue {
        self.value
    }
}

/// Convert a borrowed Value into a borrowed UntaggedValue
impl<'a> Into<&'a UntaggedValue> for &'a Value {
    fn into(self) -> &'a UntaggedValue {
        &self.value
    }
}

impl HasSpan for Value {
    /// Return the corresponding Span for the Value
    fn span(&self) -> Span {
        self.tag.span
    }
}

impl ShellTypeName for Value {
    /// Get the type name for the Value
    fn type_name(&self) -> &'static str {
        ShellTypeName::type_name(&self.value)
    }
}

impl ShellTypeName for UntaggedValue {
    /// Get the type name for the UntaggedValue
    fn type_name(&self) -> &'static str {
        match &self {
            UntaggedValue::Primitive(p) => p.type_name(),
            UntaggedValue::Row(_) => "row",
            UntaggedValue::Table(_) => "table",
            UntaggedValue::Error(_) => "error",
            UntaggedValue::Block(_) => "block",
        }
    }
}

impl From<Primitive> for UntaggedValue {
    /// Convert a Primitive to an UntaggedValue
    fn from(input: Primitive) -> UntaggedValue {
        UntaggedValue::Primitive(input)
    }
}

impl From<String> for UntaggedValue {
    /// Convert a String to an UntaggedValue
    fn from(input: String) -> UntaggedValue {
        UntaggedValue::Primitive(Primitive::String(input))
    }
}

impl From<ShellError> for UntaggedValue {
    fn from(e: ShellError) -> Self {
        UntaggedValue::Error(e)
    }
}

pub fn merge_descriptors(values: &[Value]) -> Vec<String> {
    let mut ret: Vec<String> = vec![];
    let value_column = "<value>".to_string();
    for value in values {
        let descs = value.data_descriptors();

        if descs.is_empty() {
            if !ret.contains(&value_column) {
                ret.push("<value>".to_string());
            }
        } else {
            for desc in value.data_descriptors() {
                if !ret.contains(&desc) {
                    ret.push(desc);
                }
            }
        }
    }
    ret
}
