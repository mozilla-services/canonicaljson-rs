use canonical_json::ser::to_string;
use serde_json;
use serde_json::Value;

use std::env;
use std::fs::File;
use std::io::BufReader;

fn main() {
    let args: Vec<String> = env::args().collect();

    // Open the file in read-only mode with buffer.
    let file = File::open(&args[1]).unwrap();
    let reader = BufReader::new(file);

    // Read the JSON contents of the file as an instance of `User`.
    let v: Value = serde_json::from_reader(reader).unwrap();

    print!("{}", to_string(&v).unwrap());
}
