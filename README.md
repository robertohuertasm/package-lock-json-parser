# package-lock-json-parser

[![ActionsStatus](https://github.com/robertohuertasm/package-lock-json-parser/workflows/Build/badge.svg)](https://github.com/robertohuertasm/package-lock-json-parser/actions) [![Crates.io](https://img.shields.io/crates/v/package-lock-json-parser.svg)](https://crates.io/crates/package-lock-json-parser)

Easily parse `package-lock.json` dependencies.

Supports lock file versions 1, 2, and 3.

## Example

```rust
use std::{error::Error, fs};
use yarn_lock_parser::{parse_str, Entry};

fn main() -> Result<(), Box<dyn Error>> {
    let package_lock_json_text = fs::read_to_string("package-lock.json")?;
    let entries: Vec<Entry> = parse_str(&package_lock_json_text)?;

    println!("{:?}", entries);

    Ok(())
}
```

## Documentation

Visit [https://docs.rs/package-lock-json-parser/](https://docs.rs/package-lock-json-parser/).

## Build

You will need [cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html), the Rust package manager.

```bash
cargo build
```

## Test

```bash
cargo test
```
