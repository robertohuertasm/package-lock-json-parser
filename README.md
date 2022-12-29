# package-lock-json-parser

[![ActionsStatus](https://github.com/robertohuertasm/package-lock-json-parser/workflows/Build/badge.svg)](https://github.com/robertohuertasm/package-lock-json-parser/actions) [![Crates.io](https://img.shields.io/crates/v/package-lock-json-parser.svg)](https://crates.io/crates/package-lock-json-parser)

Easily parse `package-lock.json` dependencies.

It supports lock file versions 1, 2, and 3.

## Example

```rust
// Getting a full package lock json file.
// You'll get information about the lock file version and a list of v1 or v2 dependencies.
// v1 lock files will only have v1 dependencies while v3 lock files will only have v2 dependencies. 
// v2 lock files will get both v1 and v2 dependencies.
// Check this URL (https://docs.npmjs.com/cli/v9/configuring-npm/package-lock-json?v=true) if you want to learn more about package-lock.json fields.
use std::{error::Error, fs};
use package_lock_json::{parse, PackageLockJson};

fn main() -> Result<(), Box<dyn Error>> {
    let package_lock_json_text = fs::read_to_string("package-lock.json")?;
    let lock_file: PackageLockJson = parse(package_lock_json_text)?;
    println!("{:?}", lock_file);
    Ok(())
}
```

```rust
// If you just a new a simple list of dependencies try the parse_dependencies function.
use std::{error::Error, fs};
use package_lock_json::{parse_dependencies, SimpleDependency};

fn main() -> Result<(), Box<dyn Error>> {
    let package_lock_json_text = fs::read_to_string("package-lock.json")?;
    let dependencies: Vec<SimpleDependency> = parse_dependencies(package_lock_json_text)?;
    println!("{:?}", dependencies);
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
