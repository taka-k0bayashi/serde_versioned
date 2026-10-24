# serde_versioned

![CI](https://github.com/taka-k0bayashi/serde_versioned/actions/workflows/ci.yml/badge.svg)

A Rust library built on top of the [Serde](https://serde.rs) crate for handling versioned serialization and deserialization of structs with backward compatibility support.

## Features

- **Version Management**: Easily manage multiple versions of your data structures
- **Backward Compatibility**: Automatically convert older versions to the current version
- **Type Safety**: Compile-time guarantees for version conversions
- **Serde Integration**: Seamless integration with serde for serialization/deserialization
- **Flexible Format Support**: Works with self-describing formats supported by serde (JSON, YAML, TOML, etc.)

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
serde_versioned = "0.3.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"  # Optional, for JSON support
```

## Quick Start

### Basic Usage

```rust
use serde_versioned::{Versioned, FromVersion};
use serde::{Serialize, Deserialize};

// Define your current struct
#[derive(Versioned, Debug, PartialEq)]
#[versioned(versions = [1 => UserV1, 2 => UserV2])]
struct User {
    pub name: String,
    pub age: u32,
}

// Define version 1 (older version)
#[derive(Serialize, Deserialize)]
pub struct UserV1 {
    pub name: String,
}

// Define version 2 (current version)
#[derive(Serialize, Deserialize)]
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
let restored = User::from_version(version);
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
let user_restored = User::from_version(version_restored);
assert_eq!(user, user_restored);
```

### Working with Older Versions

```rust
// Deserialize an older version (v1) and convert to current
let v1_json = r#"{"version":"1","name":"Eve"}"#;
let user = User::from_format(v1_json, serde_json::from_str).unwrap();
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
let json = user.to_format(serde_json::to_string).unwrap();

// Deserialize using convenience method
let user = User::from_format(&json, serde_json::from_str).unwrap();
```

### Serializing the Struct Directly

The derive also implements `From` between the struct and its version enum, so serde's
`from`/`into` attributes make the struct itself read any version and write the latest one.
This works anywhere serde does, including inside other types:

```rust
#[derive(Versioned, Serialize, Deserialize, Clone)]
#[serde(from = "UserVersion", into = "UserVersion")]
#[versioned(versions = [1 => UserV1, 2 => UserV2])]
struct User {
    pub name: String,
    pub age: u32,
}

#[derive(Serialize, Deserialize)]
struct Team {
    members: Vec<User>, // each member is read from any version and written as version 2
}
```

## Version Numbers

- The number before `=>` is written to the `"version"` field as a string (`"version": "1"`;
  in TOML write `version = "1"`, not `version = 1`).
- Never change a number once data has been written with it. Numbers must be strictly
  increasing, and the last entry is the latest version.
- Removing an old version is safe: the other numbers stay the same, and data stored with the
  removed version fails to deserialize.

## Document Versions

For a file made of many nested types (a project file, for example), the version can be
written once at the root instead of on every value:

```rust
#[derive(Versioned)]
#[versioned(document = 3)]        // the current version of the whole file
struct Project {
    teams: Vec<Team>,
    settings: Settings,
}

#[derive(Serialize, Deserialize)] // never changed: a plain serde type
struct Team {
    members: Vec<User>,
}

#[derive(Versioned)]               // documents 1-2 used UserV1, 3 and later use Self
#[versioned(versions = [1 => UserV1, 3 => Self])]
struct User {
    name: String,
    age: u32,
}

#[derive(Versioned)]               // changed in document 2
#[versioned(versions = [1 => SettingsV1, 2 => Self])]
struct Settings {
    auto_save: bool,
    theme: String,
}
```

```json
{"version":"3","teams":[{"members":[{"name":"A","age":30}]}],"settings":{...}}
```

- A list ending with `Self` makes the struct a member of the document. Its numbers are
  document versions, and it is read with the largest entry not above the file's version.
  Old entries only need `Deserialize` and `FromVersion`.
- Whenever any type changes shape, bump `document` and add `N => Self` to the changed type,
  keeping its previous shape as an old version struct. Unchanged types need nothing.
- Roots and members implement `Serialize`/`Deserialize` themselves; do not derive them.
  `#[serde(...)]` attributes on the struct and its fields still apply.
- A root can be nested inside another root; its part of the file has its own `"version"`.
  A root whose own fields change takes `versions = [..., N => Self]` too.
- A member read on its own (outside any document) is read as its latest shape. Use
  `serde_versioned::with_document_version(n, || ...)` to read it as document version `n`.
- A forgotten bump cannot be detected at compile time. Keep a file written by every document
  version in your tests, check that they all still deserialize, and check that the current
  data serializes exactly to the latest one (see `tests/document.rs`).

## Requirements

- The struct must have named fields and no generic parameters (tuple structs and unit structs are not supported)
- Every field of the struct must implement `Clone` (not required for document roots and members)
- The latest version struct must have the same field names as the struct
- Each version struct must implement:
  - `Serialize` and `Deserialize` (from serde)
  - `FromVersion<CurrentStruct>` trait

## Limitations

- The version is stored as a field inside the data (an internally tagged enum), so only
  self-describing formats such as JSON, YAML and TOML are supported. Formats like bincode can
  serialize the data but cannot deserialize it.
- The serialized data cannot contain another field named `version`. If a version struct has a
  `version` field, rename it with `#[serde(rename = "...")]`.

## Examples

See the `tests/test.rs` file for more comprehensive examples including:
- Version conversion
- JSON, TOML and YAML serialization/deserialization
- Handling multiple versions, referenced by path
- Using versioned structs inside other structs

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
