# serde_versioned

A Rust library built on top of the [Serde](https://serde.rs) crate for handling versioned serialization and deserialization of structs with backward compatibility support.

## Features

- **Version Management**: Easily manage multiple versions of your data structures
- **Backward Compatibility**: Automatically convert older versions to the current version
- **Type Safety**: Compile-time guarantees for version conversions
- **Serde Integration**: Seamless integration with serde for serialization/deserialization
- **Flexible Format Support**: Works with any format supported by serde (JSON, YAML, TOML, etc.)

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
serde_versioned = "0.1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"  # Optional, for JSON support
```

## Quick Start

### Basic Usage

```rust
use serde_versioned::{Versioned, FromVersion};
use serde::{Serialize, Deserialize};

// Define your current struct
#[derive(Versioned, Serialize, Deserialize, Debug, PartialEq, Clone)]
#[versioned(versions = [UserV1, UserV2])]
struct User {
    pub name: String,
    pub age: u32,
}

// Define version 1 (older version)
#[derive(Serialize, Deserialize, Clone)]
pub struct UserV1 {
    pub name: String,
}

// Define version 2 (current version)
#[derive(Serialize, Deserialize, Clone)]
pub struct UserV2 {
    pub name: String,
    pub age: u32,
}

// Implement conversion from version 1 to current
impl FromVersion<User> for UserV1 {
    fn convert(self) -> User {
        User {
            name: self.name,
            age: 0, // default value for missing field
        }
    }
}

// Implement conversion from version 2 to current
impl FromVersion<User> for UserV2 {
    fn convert(self) -> User {
        User {
            name: self.name,
            age: self.age,
        }
    }
}
```

### Converting Between Versions

```rust
// Convert current struct to versioned enum
let user = User {
    name: "Alice".to_string(),
    age: 30,
};
let version = user.to_version();

// Convert versioned enum back to current struct
let restored = User::from_version(version).unwrap();
assert_eq!(user, restored);
```

### Serialization and Deserialization

```rust
use serde_json;

// Serialize to JSON
let user = User {
    name: "David".to_string(),
    age: 35,
};
let version = user.to_version();
let json = serde_json::to_string(&version).unwrap();
// json: {"version":"2","name":"David","age":35}

// Deserialize from JSON
let version_restored: UserVersion = serde_json::from_str(&json).unwrap();
let user_restored = User::from_version(version_restored).unwrap();
assert_eq!(user, user_restored);
```

### Working with Older Versions

```rust
// Deserialize an older version (v1) and convert to current
let v1_json = r#"{"version":"1","name":"Eve"}"#;
let user = User::from_format(v1_json, |s| serde_json::from_str(s)).unwrap();
assert_eq!(user.name, "Eve");
assert_eq!(user.age, 0); // default value from conversion
```

### Convenience Methods

```rust
// Serialize using convenience method
let user = User {
    name: "Frank".to_string(),
    age: 40,
};
let json = user.to_format(|v| serde_json::to_string(v)).unwrap();

// Deserialize using convenience method
let user = User::from_format(&json, |s| serde_json::from_str(s)).unwrap();
```

## Requirements

- The struct must have named fields (tuple structs and unit structs are not supported)
- Each version struct must implement:
  - `Serialize` and `Deserialize` (from serde)
  - `Clone`
  - `FromVersion<CurrentStruct>` trait

## Examples

See the `tests/test.rs` file for more comprehensive examples including:
- Version conversion
- JSON serialization/deserialization
- Handling multiple versions
- Default value handling for missing fields

## License

This project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details.
