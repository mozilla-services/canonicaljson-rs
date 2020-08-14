/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */
 
use log::debug;
use serde::ser::Serialize;
use serde_json::ser::{CharEscape, Formatter};
use std::io::Write;
use std::string::FromUtf8Error as Utf8Error;

struct JSONFormatter {}

#[derive(Debug)]
pub enum CanonicalJSONError {}

impl From<Utf8Error> for CanonicalJSONError {
    fn from(err: Utf8Error) -> Self {
        err.into()
    }
}

impl From<serde_json::error::Error> for CanonicalJSONError {
    fn from(err: serde_json::error::Error) -> Self {
        err.into()
    }
}

impl Formatter for JSONFormatter {
    fn write_f64<W: ?Sized>(&mut self, writer: &mut W, value: f64) -> Result<(), std::io::Error>
    where
        W: Write,
    {
        format_number(writer, value, 10.0_f64.powf(21.0), 10.0_f64.powf(-6.0))?;
        Ok(())
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
                writer.write(b"\\\"")?;
            },
            CharEscape::ReverseSolidus => {
                writer.write(b"\\\\")?;
            },
            CharEscape::LineFeed => {
                writer.write(b"\\n")?;
            },
            CharEscape::Tab => {
                writer.write(b"\\t")?;
            },
            CharEscape::CarriageReturn => {
                writer.write(b"\\r")?;
            },
            CharEscape::Solidus => {
                writer.write(b"\\/")?;
            },
            CharEscape::Backspace => {
                writer.write(b"\\b")?;
            },
            CharEscape::FormFeed => {
                writer.write(b"\\f")?;
            },
            CharEscape::AsciiControl(number) => {
                static HEX_DIGITS: [u8; 16] = *b"0123456789abcdef";
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
        let formatted_string = format!("{}", fragment)
            .escape_default()
            .to_string()
            .replace(r#"\'"#, "'");

        return format_unicode_in_string(writer, formatted_string).and(Ok(()));
    }
}

fn format_number<W: ?Sized>(
    writer: &mut W,
    number: f64,
    lower_bound: f64,
    upper_bound: f64,
) -> Result<(), std::io::Error>
where
    W: Write,
{
    if (0.0 < number && number < upper_bound) || (number >= lower_bound) {
        debug!("converting number {} to scientific notation", number);
        let number_string = format!("{:e}", number);
        debug!("formatted_number {}", number_string);
        let mut prev_char = '\0';
        for curr_char in number_string.chars() {
            if prev_char == 'e' && curr_char != '-' {
                // its a positive exponent
                writer.write("+".as_bytes())?;
            }

            writer.write(curr_char.to_string().as_bytes())?;
            prev_char = curr_char;
        }

        return Ok(());
    }

    debug!("returning number {} without scientifc notation", number);
    writer.write(&format!("{}", number).into_bytes())?;
    Ok(())
}

/// looking for \u{X} \u{XX}, \u{XXX}, \u{XXXX} to remove the curly braces
fn format_unicode_in_string<W: ?Sized>(
    writer: &mut W,
    serialized_string: String,
) -> Result<(), std::io::Error>
where
    W: Write,
{
    let mut string_iter = serialized_string.chars().peekable();

    while let Some(curr_char) = string_iter.next() {
        if curr_char == '\\' && string_iter.peek() == Some(&'u') {
            writer.write(&"\\u".as_bytes())?;
            string_iter.next();

            if string_iter.peek() == Some(&'{') {
                // consume at most 4 characters till '}' is found
                let mut characters = String::new();
                string_iter.next(); // skip the '{' for now
                let mut index = 0;

                while index < 4 && string_iter.peek() != Some(&'}') && string_iter.peek() != None {
                    match string_iter.peek() {
                        Some(character) => characters.push(*character),
                        None => break,
                    };

                    string_iter.next();
                    index += 1;
                }

                if string_iter.peek() == None {
                    // could not find '}' bracket so must include '{' and following characters
                    writer.write(&"{".as_bytes())?;
                    writer.write(&characters.into_bytes())?;
                } else if string_iter.peek() == Some(&'}') {
                    // found '}' - remove '{' and '}' but must pad zeros
                    if characters.len() == 0 {
                        writer.write(&"{}".as_bytes())?;
                    } else {
                        writer.write(
                            &std::iter::repeat("0")
                                .take(4 - characters.len())
                                .collect::<String>()
                                .into_bytes(),
                        )?;
                        writer.write(&characters.into_bytes())?;
                        string_iter.next(); // skip '}'
                    }
                }
            }

            continue;
        }

        writer.write(curr_char.to_string().as_bytes())?;
    }

    Ok(())
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
    let mut serializer = serde_json::Serializer::with_formatter(string, JSONFormatter {});
    input.serialize(&mut serializer)?;
    let serialized_string = String::from_utf8(serializer.into_inner())?;
    Ok(serialized_string)
}

#[cfg(test)]
mod tests {
    use super::to_string;
    use env_logger;
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

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn test_to_string() {
        init();

        test_canonical_json!(null, "null");
        // serialize nan to null
        test_canonical_json!((std::f64::NAN), "null");
        // serialize inf to null
        test_canonical_json!((std::f64::INFINITY), "null");
        // serialize negative inf to null
        test_canonical_json!((std::f64::NEG_INFINITY), "null");

        // serialize bool
        test_canonical_json!((true), "true");
        test_canonical_json!((false), "false");

        // serialize number
        test_canonical_json!((0), "0");
        // positive number
        test_canonical_json!((123), "123");
        // negative number
        test_canonical_json!((-123), "-123");
        // non-zero decimals
        test_canonical_json!((23.1), "23.1");
        // trim trailing decimal zeros
        test_canonical_json!((23.0), "23");
        test_canonical_json!((2300), "2300");
        // preserve numbers > 10^-6 and < 10^21
        test_canonical_json!((0.00099), "0.00099");
        test_canonical_json!((0.000011), "0.000011");
        test_canonical_json!((0.0000011), "0.0000011");
        test_canonical_json!((0.000001), "0.000001");
        // convert to scientific notation if number <= 10^-6 and number >= 10^21
        test_canonical_json!((0.00000099), "9.9e-7");
        test_canonical_json!((0.0000001), "1e-7");
        test_canonical_json!((0.000000930258908), "9.30258908e-7");
        test_canonical_json!((0.00000000000068272), "6.8272e-13");
        // very large number >= 10^21
        test_canonical_json!((10.000_f64.powf(21.0)), "1e+21");
        test_canonical_json!((10.0_f64.powi(20)), "100000000000000000000");
        test_canonical_json!((10.0_f64.powi(15) + 0.1), "1000000000000000.1");
        test_canonical_json!((10.0_f64.powi(16) * 1.1), "11000000000000000");

        // serialize string
        test_canonical_json!((""), r#""""#);
        //escape quotes
        test_canonical_json!(" Preserve single quotes'in string", r#"" Preserve single quotes'in string""#);
        test_canonical_json!(" Escapes quotes \" ", r#"" Escapes quotes \" ""#);
        test_canonical_json!(("test"), r#""test""#);
        // escapes backslashes
        test_canonical_json!(("This\\and this"), r#""This\\and this""#);
        // convert unicode characters to unicode escape sequences
        test_canonical_json!(("I ❤ testing"), r#""I \u2764 testing""#);

        // serialize does not alter certain strings (newline, tab, carriagereturn, forwardslashes)
        test_canonical_json!(("This is a sentence.\n"), r#""This is a sentence.\n""#);
        test_canonical_json!(("This is a \t tab."), r#""This is a \t tab.""#);
        test_canonical_json!(
            "This is a \r carriage return char.",
            r#""This is a \r carriage return char.""#
        );
        test_canonical_json!(("image/jpeg"), r#""image/jpeg""#);
        test_canonical_json!(("image//jpeg"), r#""image//jpeg""#);
        // serialize preserves scientific notation number within string
        test_canonical_json!(("frequency at 10.0e+04"), r#""frequency at 10.0e+04""#);
        // serialize preserves invalid unicode escape sequence
        test_canonical_json!(("I \\u{} testing"), r#""I \\u{} testing""#);
        // serialize preserves opening curly brackets when invalid unicode escape sequence
        test_canonical_json!(("I \\u{1234 testing"), r#""I \\u{1234 testing""#);
        test_canonical_json!(
            "I \\u{{12345}} testing",
            r#""I \\u{{12345}} testing""#
        );

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

        // escapes unicode characters in object keys
        test_canonical_json!({"é": "check"}, r#"{"\u00e9":"check"}"#);

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
            r#"{"abc":9.30258908e-7,"def":"bar","ghi":1e+21,"rust":"\u2764","zoo":["zorilla","anteater"]}"#
        );

        // serialize empty array
        test_canonical_json!([], "[]");

        // serialize array should preserve array order
        test_canonical_json!(
            (vec!["one", "two", "three"]),
            r#"["one","two","three"]"#
        );

        test_canonical_json!((vec![json!({ "key": "✓" })]), r#"[{"key":"\u2713"}]"#);

        // escapes unicode values with 1 preceding zeros
        test_canonical_json!((vec![json!({ "key": "ę" })]), r#"[{"key":"\u0119"}]"#);

        // escapes unicode values with 2 preceding zeros
        test_canonical_json!((vec![json!({ "key": "é" })]), r#"[{"key":"\u00e9"}]"#);

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
