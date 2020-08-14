# Canonical JSON library (Remote-Settings)

  Canonical JSON is a variant of JSON in which each value has a single,
  unambiguous serialized form. This provides meaningful and repeatable hashes
  of encoded data.

Compared to serde_json::to_string
- String contents are not guaranteed be parsable as UTF-8. Be aware that encoded data may contain escaped unicode characters.
- Minus Zero is disallowed
- Object keys must appear in lexiographical order and must not be repeated.

[Link to the Canonical Spec](spec.txt)

Comparsion to other Canonical JSON implementation - https://github.com/zmanian/canonical_json

- Canonical JSON library (Remote-Settings) supports floating-point numbers, exponents, and can convert Unicode characters into Unicode escape sequences
- Preserves character escapes for {Tab, CarriageReturn, Newline, LineFeed, Quote}
- Escapes ReverseSolidus
- Canonical JSON can be parsed by regular JSON parsers

## Examples

```rust,no_run
   use canonical_json::ser::to_string;
   use serde_json::json;
   fn main() {
     to_string(&json!(null)); // returns "null"
 
     to_string(&json!("test")); // returns "test"
 
     to_string(&json!(10.0_f64.powf(21.0))); // returns "1e+21"
 
     to_string(&json!({
         "a": "a",
         "id": "1",
         "b": "b"
     })); // returns "{"a":"a","b":"b","id":"1"}"; (orders object keys)
 
     to_string(&json!(vec!["one", "two", "three"])); // returns "["one","two","three"]"
   } 
```

## License

Licensed under Mozilla Public License, Version 2.0 (https://www.mozilla.org/en-US/MPL/2.0/)
