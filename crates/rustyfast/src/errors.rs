use std::io;
use thiserror::Error;

/// Any error that is detected solely by examining a template definition, thus
/// even before receiving any data stream. Counterparties MUST signal static
/// errors and the template where the original error occurred must be discarded.
#[derive(Copy, Clone, Debug, Error)]
#[non_exhaustive]
pub enum StaticError {
    /// It is a static error if templates encoded in the concrete XML syntax are
    /// in fact not well-formed, do not follow the rules of XML namespaces or are
    /// invalid with respect to the schema in Appendix 1.
    #[error("The template is not encoded correctly according to the XML spec.")]
    S1,
    /// It is a static error if an operator is specified for a field of a type to
    /// which the operator is not applicable.
    #[error(
        "An operator is specified for a field of a type to which the operator is not applicable."
    )]
    S2,
    /// It is a static error if an initial value specified by the value attribute
    /// in the concrete syntax cannot be converted to a value of the type of the
    /// field.
    #[error("The initial value specified for a field is not convertible to its type.")]
    S3,
    /// It is a static error if no initial value is specified for a constant
    /// operator.
    #[error("No initial value is specified for a constant operator.")]
    S4,
    /// It is a static error if no initial value is specified for a default
    /// operator on a mandatory field.
    #[error("No initial value is specified for a default operator on a mandatory field.")]
    S5,
}

/// Any error detected when encoding or decoding a FAST stream. Counterparties
/// MUST signal dynamic errors.
#[derive(Clone, Debug, Error)]
#[non_exhaustive]
pub enum DynamicError {
    /// It is a dynamic error if type of a field in a template cannot be
    /// converted to or from the type of the corresponding application field.
    #[error(
        "A field in a template cannot be converted to or from the type of the corresponding application field"
    )]
    D1,
    /// It is a dynamic error if an integer in the stream does not fall within
    /// the bounds of the specific integer type specified on the corresponding
    /// field.
    #[error(
        "An integer in the stream does not fall within the bounds of the specific integer type specified on the corresponding field."
    )]
    D2,
    /// Enhanced version of D2 with specific value details for better debugging.
    #[error(
        "Integer value {value} in the stream does not fall within bounds [{min_bound}..{max_bound}] of the specified integer type on field."
    )]
    D2WithValue {
        value: i64,
        min_bound: i64,
        max_bound: i64,
    },
    /// It is a dynamic error if a decimal value cannot be encoded due to
    /// limitations introduced by using individual operators on exponent and
    /// mantissa.
    #[error(
        "A decimal value cannot be encoded due to limitations introduced by using individual operators on exponent and mantissa."
    )]
    D3,
    /// Enhanced version of D3 with specific decimal details.
    #[error(
        "Decimal value (exponent: {exponent}, mantissa: {mantissa}) cannot be encoded due to limitations introduced by using individual operators on exponent and mantissa."
    )]
    D3WithValue { exponent: i32, mantissa: i64 },
    /// It is a dynamic error if the type of the previous value is not the same
    /// as the type of the field of the current operator.
    #[error(
        "The type of the previous value is not the same as the type of the field of the current operator."
    )]
    D4,
    /// It is a dynamic error if a mandatory field is not present in the stream,
    /// has an undefined previous value and there is no initial value in the
    /// instruction context.
    #[error(
        "A mandatory field is not present in the stream, has an undefined previous value and there is no initial value in the instruction context."
    )]
    D5,
    /// It is a dynamic error if a mandatory field is not present in the stream
    /// and has an empty previous value.
    #[error("A mandatory field is not present in the stream and has an empty previous value.")]
    D6,
    /// It is a dynamic error if the subtraction length exceeds the length of the
    /// base value or if it does not fall in the value rang of an int32.
    #[error(
        "The subtraction length exceeds the length of the base value or if it does not fall in the value rang of an int32."
    )]
    D7,
    /// It is a dynamic error if the name specified on a static template
    /// reference does not point to a template known by the encoder or decoder.
    #[error(
        "The name specified on a static template reference does not point to a template known by the encoder or decoder."
    )]
    D8,
    /// It is a dynamic error if a decoder cannot find a template associated with
    /// a template identifier appearing in the stream.
    #[error(
        "A decoder cannot find a template associated with a template identifier appearing in the stream."
    )]
    D9,
    /// It is a dynamic error if the syntax of a string does not follow the rules
    /// for the type converted to.
    #[error("The syntax of a string does not follow the rules for the type converted to.")]
    D10,
    /// It is a dynamic error if the syntax of a string does not follow the rules
    /// for the type converted to
    #[error("The syntax of a string does not follow the rules for the type converted to.")]
    D11,
    /// It is a dynamic error if a block length preamble is zero.
    #[error("A block length preamble is zero.")]
    D12,
}

/// Any error detected when encoding or decoding a FAST stream. Contrary to
/// dynamic errors, counterparties are not obligated to signal dynamic errors an
/// may choose not to do so, e.g. to improve performance.
#[derive(Clone, Debug, Error)]
#[non_exhaustive]
pub enum ReportableError {
    /// It is a reportable error if a decimal cannot be represented by an
    /// exponent in the range [-63 … 63] or if the mantissa does not fit in an
    /// int64.
    #[error(
        "A decimal cannot be represented by an exponent in the range [-63 … 63] or if the mantissa does not fit in an int64."
    )]
    R1,
    /// Enhanced version of R1 with specific decimal details for debugging.
    #[error(
        "Decimal cannot be represented: exponent {exponent} is outside range [-63..63] or mantissa {mantissa} does not fit in int64."
    )]
    R1WithValue { exponent: i32, mantissa: i64 },
    /// It is a reportable error if the combined value after applying a tail or
    /// delta operator to a Unicode string is not a valid UTF-8 sequence.
    #[error(
        "The combined value after applying a tail or delta operator to a Unicode string is not a valid UTF-8 sequence."
    )]
    R2,
    /// It is a reportable error if a Unicode string that is being converted to
    /// an ASCII string contains characters that are outside the ASCII character
    /// set.
    #[error(
        "A Unicode string that is being converted to an ASCII string contains characters that are outside the ASCII character set."
    )]
    R3,
    /// It is a reportable error if a value of an integer type cannot be
    /// represented in the target integer type in a conversion.
    #[error(
        "A value of an integer type cannot be represented in the target integer type in a conversion."
    )]
    R4,
    /// Enhanced version of R4 with specific conversion details.
    #[error(
        "Integer value {value} cannot be represented in target type '{target_type}' (range: {min_target}..{max_target})."
    )]
    R4WithValue {
        value: i64,
        target_type: &'static str,
        min_target: i64,
        max_target: i64,
    },
    /// It is a reportable error if a decimal being converted to an integer has a
    /// negative exponent or if the resulting integer does not fit the target
    /// integer type.
    #[error(
        "A decimal being converted to an integer has a negative exponent or if the resulting integer does not fit the target integer type."
    )]
    R5,
    /// Enhanced version of R5 with specific decimal conversion details.
    #[error(
        "Decimal (exponent: {exponent}, mantissa: {mantissa}) being converted to integer: {reason}."
    )]
    R5WithValue {
        exponent: i32,
        mantissa: i64,
        reason: &'static str, // "negative exponent" or "resulting integer doesn't fit target type"
    },
    /// It is a reportable error if an integer appears in an overlong encoding.
    #[error("An integer appears in an overlong encoding.")]
    R6,
    /// It is a reportable error if a presence map is overlong.
    #[error("A presence map is overlong.")]
    R7,
    /// It is a reportable error if a presence map contains more bits than required.
    #[error("A presence map contains more bits than required.")]
    R8,
    /// It is a reportable error if a string appears in an overlong encoding.
    #[error("A string appears in an overlong encoding.")]
    R9,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Static(#[from] StaticError),
    #[error(transparent)]
    Dynamic(#[from] DynamicError),
    #[error(transparent)]
    Reportable(#[from] ReportableError),
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}
