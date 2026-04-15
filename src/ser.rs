use regex::Regex;
use serde::ser::Serialize;
use serde_json::ser::{CharEscape, Formatter};
use std::io::Write;
use std::string::FromUtf8Error as Utf8Error;
use thiserror::Error;

// Compile-time assertion: serde_json must be compiled with the "arbitrary_precision" feature.
// Without it, serde_json::Number does not implement serde::Deserializer, and more critically,
// write_number_str is never called — numbers are coerced through f64, losing precision for
// large integers and non-representable decimals.
const _: () = {
    fn _requires<'de, T: serde::Deserializer<'de>>() {}
    fn _check_arbitrary_precision() {
        _requires::<serde_json::Number>();
    }
};

/// Canonicalize an arbitrary-precision number string to canonical JSON form.
///
/// Rules:
/// - Integers (no fractional part): output as plain decimal integer, no sign for zero.
/// - Non-integers: `d.dddE±n` — one non-zero digit before the decimal, at least one digit
///   after, no trailing zeros after the last significant digit, capital E, no leading zeros
///   or plus sign in the exponent.
fn canonicalize_number(s: &str) -> String {
    // Strip sign
    let (negative, s) = if let Some(rest) = s.strip_prefix('-') {
        (true, rest)
    } else {
        (false, s)
    };

    // Split off exponent
    let (mantissa, explicit_exp): (&str, i64) = if let Some(pos) = s.find(|c| c == 'e' || c == 'E')
    {
        let exp: i64 = s[pos + 1..].parse().unwrap_or(0);
        (&s[..pos], exp)
    } else {
        (s, 0)
    };

    // Split mantissa into integer and fractional parts
    let (int_str, frac_str) = if let Some(pos) = mantissa.find('.') {
        (&mantissa[..pos], &mantissa[pos + 1..])
    } else {
        (mantissa, "")
    };

    // All significant digits concatenated
    let all_digits = format!("{}{}", int_str, frac_str);
    let frac_len = frac_str.len() as i64;

    // base10_exp: value = all_digits_as_integer * 10^base10_exp
    let base10_exp = explicit_exp - frac_len;

    // Strip leading zeros to get significant digits
    let significant = all_digits.trim_start_matches('0');

    if significant.is_empty() {
        return "0".to_string();
    }

    let n = significant.len() as i64;
    let sign = if negative { "-" } else { "" };

    // Determine if the value is an integer
    let is_integer = if base10_exp >= 0 {
        true
    } else {
        let frac_digits = (-base10_exp) as usize;
        if frac_digits > significant.len() {
            false
        } else {
            significant[significant.len() - frac_digits..]
                .bytes()
                .all(|b| b == b'0')
        }
    };

    if is_integer {
        if base10_exp >= 0 {
            let zeros = "0".repeat(base10_exp as usize);
            format!("{}{}{}", sign, significant, zeros)
        } else {
            let keep = significant.len() - (-base10_exp) as usize;
            let int_part = &significant[..keep];
            if int_part.is_empty() {
                "0".to_string()
            } else {
                format!("{}{}", sign, int_part)
            }
        }
    } else {
        // Canonical exponent: place decimal after first digit
        let canon_exp = base10_exp + n - 1;

        // Trim trailing zeros from significand
        let sig_trimmed = significant.trim_end_matches('0');

        // Build significand: always at least one digit after the decimal point
        let significand = if sig_trimmed.len() == 1 {
            format!("{}.0", sig_trimmed)
        } else {
            format!("{}.{}", &sig_trimmed[..1], &sig_trimmed[1..])
        };

        format!("{}{}E{}", sign, significand, canon_exp)
    }
}

/// Implements the [serde_json::ser::Formatter] trait for serializing [serde_json::Value] objects into their
/// canonical string representation.
///
/// # Example
///
/// ```
/// use serde::Serialize;
/// use serde_json::json;
/// use canonical_json::JsonFormatter;
///
/// let input = json!(vec!["one", "two", "three"]);
/// let mut bytes = vec![];
/// let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, JsonFormatter);
/// input.serialize(&mut serializer).unwrap();
///
/// assert_eq!(String::from_utf8(bytes).unwrap(), r#"["one","two","three"]"#);
/// ```
pub struct JsonFormatter;

#[derive(Debug, Error)]
pub enum CanonicalJSONError {
    #[error("UTF-8 related error: {0}")]
    Utf8Error(#[from] Utf8Error),
    #[error("JSON related error: {0}")]
    JSONError(#[from] serde_json::error::Error),
}

impl Formatter for JsonFormatter {
    fn write_f64<W: ?Sized>(&mut self, writer: &mut W, value: f64) -> Result<(), std::io::Error>
    where
        W: Write,
    {
        format_number(writer, value)?;
        Ok(())
    }

    fn write_number_str<W: ?Sized>(
        &mut self,
        writer: &mut W,
        value: &str,
    ) -> Result<(), std::io::Error>
    where
        W: Write,
    {
        writer.write_all(canonicalize_number(value).as_bytes())
    }

    fn write_char_escape<W: ?Sized>(
        &mut self,
        writer: &mut W,
        char_escape: CharEscape,
    ) -> Result<(), std::io::Error>
    where
        W: Write,
    {
        match char_escape {
            CharEscape::Quote => {
                writer.write_all(b"\\\"")?;
            }
            CharEscape::ReverseSolidus => {
                writer.write_all(b"\\\\")?;
            }
            CharEscape::LineFeed => {
                writer.write_all(b"\\n")?;
            }
            CharEscape::Tab => {
                writer.write_all(b"\\t")?;
            }
            CharEscape::CarriageReturn => {
                writer.write_all(b"\\r")?;
            }
            CharEscape::Solidus => {
                writer.write_all(b"\\/")?;
            }
            CharEscape::Backspace => {
                writer.write_all(b"\\b")?;
            }
            CharEscape::FormFeed => {
                writer.write_all(b"\\f")?;
            }
            CharEscape::AsciiControl(number) => {
                static HEX_DIGITS: [u8; 16] = *b"0123456789ABCDEF";
                let bytes = &[
                    b'\\',
                    b'u',
                    b'0',
                    b'0',
                    HEX_DIGITS[(number >> 4) as usize],
                    HEX_DIGITS[(number & 0xF) as usize],
                ];
                return writer.write_all(bytes);
            }
        }

        Ok(())
    }

    fn write_string_fragment<W: ?Sized>(
        &mut self,
        writer: &mut W,
        fragment: &str,
    ) -> Result<(), std::io::Error>
    where
        W: Write,
    {
        writer.write_all(fragment.as_bytes())
    }
}

fn format_number<W: ?Sized>(writer: &mut W, number: f64) -> Result<(), std::io::Error>
where
    W: Write,
{
    let formatted = format!("{:e}", number);
    let normalized = normalize_number(formatted);
    writer.write_all(&normalized.into_bytes())?;
    Ok(())
}

// force capital-E exponent, remove + signs and leading zeroes
fn normalize_number(input: String) -> String {
    // https://github.com/gibson042/canonicaljson-go/blob/b9eb21a76/encode.go#L506-L514
    let re = Regex::new("(?:E(?:[+]0*|(-|)0+)|e(?:[+]|(-|))0*)([0-9])").unwrap();
    re.replace_all(&input, "E$1$2$3").to_string()
}

/// Serialize a JSON value to String
///
/// # Examples
/// ```rust
/// # use canonical_json::ser::to_string;
/// # use serde_json::json;
/// # fn main() {
///     to_string(&json!(null)); // returns "null"
///
///     to_string(&json!("test")); // returns "test"
///
///     to_string(&json!(10.0_f64.powf(21.0))); // returns "1e+21"
///
///     to_string(&json!({
///         "a": "a",
///         "id": "1",
///         "b": "b"
///     })); // returns "{"a":"a","b":"b","id":"1"}"; (orders object keys)
///
///     to_string(&json!(vec!["one", "two", "three"])); // returns "["one","two","three"]"
/// # }
///
/// ```
pub fn to_string(input: &serde_json::Value) -> Result<String, CanonicalJSONError> {
    let string = vec![];
    let mut serializer = serde_json::Serializer::with_formatter(string, JsonFormatter);
    input.serialize(&mut serializer)?;
    let serialized_string = String::from_utf8(serializer.into_inner())?;
    Ok(serialized_string)
}

#[cfg(test)]
mod tests {
    use super::to_string;
    use serde_json::json;

    macro_rules! test_canonical_json {
        ($v:tt, $e:expr) => {
            match to_string(&json!($v)) {
                Ok(serialized_string) => {
                    println!("serialized is {}", serialized_string);
                    assert_eq!(serialized_string, $e)
                },
                Err(error) => { panic!("error serializing input : {:?}", error) }
            };
        };
    }

    #[test]
    fn test_to_string() {
        test_canonical_json!(null, "null");
        test_canonical_json!((std::f64::NAN), "null");
        test_canonical_json!((std::f64::INFINITY), "null");
        test_canonical_json!((std::f64::NEG_INFINITY), "null");
        test_canonical_json!(true, "true");
        test_canonical_json!(false, "false");
        test_canonical_json!(0, "0");
        test_canonical_json!(123, "123");
        test_canonical_json!((-123), "-123");
        test_canonical_json!(23.1, "2.31E1");
        test_canonical_json!(23, "23");
        test_canonical_json!(1_f64, "1");
        test_canonical_json!(0_f64, "0");
        test_canonical_json!(23.0, "23");
        test_canonical_json!((-23.0), "-23");
        test_canonical_json!(2300, "2300");
        test_canonical_json!(0.00099, "9.9E-4");
        test_canonical_json!(0.000011, "1.1E-5");
        test_canonical_json!(0.0000011, "1.1E-6");
        test_canonical_json!(0.000001, "1.0E-6");
        test_canonical_json!(5.6, "5.6E0");
        test_canonical_json!(0.00000099, "9.9E-7");
        test_canonical_json!(0.0000001, "1.0E-7");
        test_canonical_json!(0.000000930258908, "9.30258908E-7");
        test_canonical_json!(0.00000000000068272, "6.8272E-13");
        test_canonical_json!((10.000_f64.powf(21.0)), "1000000000000000000000");
        test_canonical_json!((10.0_f64.powi(20)), "100000000000000000000");
        test_canonical_json!((10.0_f64.powi(15) + 0.1), "1.0000000000000001E15");
        test_canonical_json!((10.0_f64.powi(16) * 1.1), "11000000000000000");

        // serialize string
        test_canonical_json!("", r#""""#);
        //escape quotes
        test_canonical_json!(
            " Preserve single quotes'in string",
            r#"" Preserve single quotes'in string""#
        );
        test_canonical_json!(" Escapes quotes \" ", r#"" Escapes quotes \" ""#);
        test_canonical_json!("test", r#""test""#);
        // escapes backslashes
        test_canonical_json!("This\\and this", r#""This\\and this""#);
        // non-ASCII characters above U+001F are not escaped
        test_canonical_json!("I ❤ testing", r#""I ❤ testing""#);

        // serialize does not alter certain strings (newline, tab, carriagereturn, forwardslashes)
        test_canonical_json!("This is a sentence.\n", r#""This is a sentence.\n""#);
        test_canonical_json!("This is a \t tab.", r#""This is a \t tab.""#);
        test_canonical_json!(
            "This is a \r carriage return char.",
            r#""This is a \r carriage return char.""#
        );
        test_canonical_json!("image/jpeg", r#""image/jpeg""#);
        test_canonical_json!("image//jpeg", r#""image//jpeg""#);
        // serialize preserves scientific notation number within string
        test_canonical_json!("frequency at 10.0e+04", r#""frequency at 10.0e+04""#);
        // serialize preserves invalid unicode escape sequence
        test_canonical_json!("I \\u{} testing", r#""I \\u{} testing""#);
        // serialize preserves opening curly brackets when invalid unicode escape sequence
        test_canonical_json!("I \\u{1234 testing", r#""I \\u{1234 testing""#);
        test_canonical_json!("I \\u{{12345}} testing", r#""I \\u{{12345}} testing""#);

        // characters above U+FFFF are output as literal UTF-8, not re-encoded as surrogate pairs
        test_canonical_json!("𝄞", r#""𝄞""#);
        test_canonical_json!("𝗠𝗼𝘇", r#""𝗠𝗼𝘇""#);
        test_canonical_json!("\u{10000} \u{10FFFF}", "\"\u{10000} \u{10FFFF}\"");

        // serialize object
        test_canonical_json!(
            {
                "a": {},
                "b": "b"
            },
            r#"{"a":{},"b":"b"}"#
        );

        // serialize object with keys ordered
        test_canonical_json!(
            {
                "a": "a",
                "id": "1",
                "b": "b"
            },
            r#"{"a":"a","b":"b","id":"1"}"#
        );

        // serialize deeply nested objects
        test_canonical_json!(
            {
                "a": json!({
                    "b": "b",
                    "a": "a",
                    "c": json!({
                        "b": "b",
                        "a": "a",
                        "c": ["b", "a", "c"],
                        "d": json!({ "b": "b", "a": "a" }),
                        "id": "1",
                        "e": 1,
                        "f": [2, 3, 1],
                        "g": json!({
                            "2": 2,
                            "3": 3,
                            "1": json!({
                                "b": "b",
                                "a": "a",
                                "c": "c",
                            })
                        })
                    })
                }),
                "id": "1"
            },
            concat!(
                r#"{"a":{"a":"a","b":"b","c":{"a":"a","b":"b","c":["b","a","c"],"#,
                r#""d":{"a":"a","b":"b"},"e":1,"f":[2,3,1],"#,
                r#""g":{"1":{"a":"a","b":"b","c":"c"},"2":2,"3":3},"id":"1"}},"id":"1"}"#
            )
        );

        test_canonical_json!(
            {
                "b": vec!["two", "three"],
                "a": vec!["zero", "one"]
            },
            r#"{"a":["zero","one"],"b":["two","three"]}"#
        );

        test_canonical_json!(
            {
                "b": { "d": "d", "c": "c" },
                "a": { "b": "b", "a": "a" },
            },
            r#"{"a":{"a":"a","b":"b"},"b":{"c":"c","d":"d"}}"#
        );

        // non-ASCII characters in object keys are not escaped
        test_canonical_json!({"é": "check"}, r#"{"é":"check"}"#);

        test_canonical_json!(
            {
                "def": "bar",
                "abc": json!(0.000000930258908),
                "ghi": json!(1000000000000000000000.0_f64),
                "rust": "❤",
                "zoo": [
                    "zorilla",
                    "anteater"
                ]
            },
            r#"{"abc":9.30258908E-7,"def":"bar","ghi":1000000000000000000000,"rust":"❤","zoo":["zorilla","anteater"]}"#
        );

        // serialize empty array
        test_canonical_json!([], "[]");

        // serialize array should preserve array order
        test_canonical_json!((vec!["one", "two", "three"]), r#"["one","two","three"]"#);

        test_canonical_json!((vec![json!({ "key": "✓" })]), r#"[{"key":"✓"}]"#);

        test_canonical_json!((vec![json!({ "key": "ę" })]), r#"[{"key":"ę"}]"#);

        test_canonical_json!((vec![json!({ "key": "é" })]), r#"[{"key":"é"}]"#);

        // serialize array preserves data
        test_canonical_json!(
            (vec![
                json!({ "foo": "bar", "last_modified": "12345", "id": "1" }),
                json!({ "bar": "baz", "last_modified": "45678", "id": "2" }),
            ]),
            r#"[{"foo":"bar","id":"1","last_modified":"12345"},{"bar":"baz","id":"2","last_modified":"45678"}]"#
        );

        // serialize does not add space separators
        test_canonical_json!(
            (vec![
                json!({ "foo": "bar", "last_modified": "12345", "id": "1" }),
                json!({ "bar": "baz", "last_modified": "45678", "id": "2" }),
            ]),
            r#"[{"foo":"bar","id":"1","last_modified":"12345"},{"bar":"baz","id":"2","last_modified":"45678"}]"#
        );
    }
}
