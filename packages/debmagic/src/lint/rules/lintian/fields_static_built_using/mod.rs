//! Rules for lintian's `fields/static-built-using` Check. Each Rule owns one
//! Tag. The clause check is shared with `fields_built_using`.

mod ln0834_invalid_value_in_static_built_using_field;

pub(crate) use ln0834_invalid_value_in_static_built_using_field::InvalidValueInStaticBuiltUsingField;
