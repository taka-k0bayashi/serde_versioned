# Changelog

## 0.3.1

### Added

- Document versions: `#[versioned(document = N)]` writes `"version"` once at the root, and
  nested types declared with `#[versioned(versions = [1 => UserV1, 3 => Self])]` are read
  according to it. Unchanged nested types stay plain serde types. Roots can be nested.
  Writing a member newer than its root's `document` fails instead of producing a file that
  cannot be read back.
- `with_document_version` and `document_version` to read a document member on its own.

## 0.3.0

### Breaking changes

- Version numbers are now written explicitly. The serialized format is unchanged.

  ```rust
  // 0.2
  #[versioned(versions = [UserV1, UserV2])]
  // 0.3
  #[versioned(versions = [1 => UserV1, 2 => UserV2])]
  ```

  Numbers must be strictly increasing. Removing an old entry no longer renumbers the others.
- `Versioned::from_version` returns `Self` instead of `Result`. Remove `.unwrap()` / `?`.
- `Versioned::from_format` returns the deserializer's own error (for example `serde_json::Error`)
  instead of `FormatError`.
- `VersionConversionError` and `FormatError` are removed.

### Added

- The derive implements `From` between the struct and its version enum, so the struct can use
  `#[serde(from = "UserVersion", into = "UserVersion")]` and be nested in other types.
- Version structs can be referenced by path, such as `models::UserV2`.

### Fixed

- Paths in the `versions` list were silently ignored.
- Mistakes in the `versioned` attribute reported "No version structs specified" instead of the
  actual error.
- Formatting a deserialization error panicked when byte 100 of the input was inside a
  multi-byte character, and it printed part of the input.
- Generic structs produced confusing compiler errors; they are now rejected with a clear message.
- The generated enum failed `#![deny(missing_docs)]`.
