use canonical_json::ser::to_string;
use serde_json::Value;
use std::env;
use std::fs;
use std::io::Read;

/// Replace lone surrogate \uDXXX escape sequences in JSON strings with Private Use Area
/// stand-ins (U+E000–U+E7FF) so that serde_json can parse the file without errors.
/// Valid surrogate pairs (\uD8xx\uDCxx) are left intact for serde_json to decode normally.
fn preprocess_surrogates(json: &str) -> String {
    let chars: Vec<char> = json.chars().collect();
    let mut result = String::with_capacity(json.len());
    let mut i = 0;
    let mut in_string = false;

    while i < chars.len() {
        if !in_string {
            if chars[i] == '"' {
                in_string = true;
                result.push('"');
                i += 1;
            } else {
                result.push(chars[i]);
                i += 1;
            }
        } else {
            // Inside a JSON string
            if chars[i] == '\\' && i + 1 < chars.len() {
                match chars[i + 1] {
                    '"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't' => {
                        result.push(chars[i]);
                        result.push(chars[i + 1]);
                        i += 2;
                    }
                    'u' | 'U' if i + 5 < chars.len() => {
                        let hex: String = chars[i + 2..i + 6].iter().collect();
                        if let Ok(cp) = u16::from_str_radix(&hex, 16) {
                            if (0xD800..=0xDBFF).contains(&cp) {
                                // High surrogate — check for following low surrogate
                                let has_low = i + 11 < chars.len()
                                    && (chars[i + 6] == '\\')
                                    && (chars[i + 7] == 'u' || chars[i + 7] == 'U')
                                    && {
                                        let hex2: String =
                                            chars[i + 8..i + 12].iter().collect();
                                        u16::from_str_radix(&hex2, 16)
                                            .map(|cp2| (0xDC00..=0xDFFF).contains(&cp2))
                                            .unwrap_or(false)
                                    };
                                if has_low {
                                    // Valid surrogate pair — leave for serde_json
                                    for c in &chars[i..i + 12] {
                                        result.push(*c);
                                    }
                                    i += 12;
                                } else {
                                    // Lone high surrogate → PUA
                                    result.push_str(&format!("\\u{:04X}", cp as u32 + 0x800));
                                    i += 6;
                                }
                            } else if (0xDC00..=0xDFFF).contains(&cp) {
                                // Lone low surrogate → PUA
                                result.push_str(&format!("\\u{:04X}", cp as u32 + 0x800));
                                i += 6;
                            } else {
                                // Normal \uXXXX — pass through
                                result.push(chars[i]);
                                result.push(chars[i + 1]);
                                for c in &chars[i + 2..i + 6] {
                                    result.push(*c);
                                }
                                i += 6;
                            }
                        } else {
                            result.push(chars[i]);
                            result.push(chars[i + 1]);
                            i += 2;
                        }
                    }
                    _ => {
                        result.push(chars[i]);
                        result.push(chars[i + 1]);
                        i += 2;
                    }
                }
            } else if chars[i] == '"' {
                in_string = false;
                result.push('"');
                i += 1;
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }
    }
    result
}

/// Convert PUA characters U+E000–U+E7FF in the serialized output back to \uDXXX escapes.
/// These were inserted by preprocess_surrogates for lone surrogates.
fn postprocess_surrogates(output: &str) -> String {
    let bytes = output.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        // U+E000–U+E7FF in UTF-8: EE [80–9F] [80–BF]
        if i + 2 < bytes.len()
            && bytes[i] == 0xEE
            && (0x80..=0x9F).contains(&bytes[i + 1])
            && (0x80..=0xBF).contains(&bytes[i + 2])
        {
            let code = 0xE000u32
                + (((bytes[i + 1] & 0x3F) as u32) << 6)
                + ((bytes[i + 2] & 0x3F) as u32);
            let surrogate = code - 0x800; // maps E000–E7FF → D800–DFFF
            result.extend_from_slice(format!("\\u{:04X}", surrogate).as_bytes());
            i += 3;
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(result).unwrap()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut file = fs::File::open(&args[1]).unwrap();
    let mut json_text = String::new();
    file.read_to_string(&mut json_text).unwrap();

    let preprocessed = preprocess_surrogates(&json_text);
    let v: Value = serde_json::from_str(&preprocessed).unwrap();
    let canonical = to_string(&v).unwrap();
    let result = postprocess_surrogates(&canonical);
    print!("{}", result);
}
