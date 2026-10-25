//! # `serde_versioned`
//!
//! A library for handling versioned serialization and deserialization of Rust structs.
//! This crate provides traits and derive macros to support multiple versions of data structures
//! while maintaining backward compatibility.
//!
//! ## Usage
//!
//! ```rust
//! use serde_versioned::{Versioned, FromVersion};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Versioned)]
//! #[versioned(versions = [1 => UserV1, 2 => UserV2])]
//! struct User {
//!     pub name: String,
//!     pub age: u32,
//! }
//!
//! #[derive(Serialize, Deserialize)]
//! pub struct UserV1 {
//!     pub name: String,
//! }
//!
//! #[derive(Serialize, Deserialize)]
//! pub struct UserV2 {
//!     pub name: String,
//!     pub age: u32,
//! }
//!
//! impl FromVersion<User> for UserV1 {
//!     fn convert(self) -> User {
//!         User { name: self.name, age: 0 }
//!     }
//! }
//!
//! impl FromVersion<User> for UserV2 {
//!     fn convert(self) -> User {
//!         User { name: self.name, age: self.age }
//!     }
//! }
//!
//! // Data is always written as the latest version
//! let json = User { name: "Alice".to_string(), age: 30 }.to_format(serde_json::to_string)?;
//! assert_eq!(json, r#"{"version":"2","name":"Alice","age":30}"#);
//!
//! // Older versions are converted when read
//! let user = User::from_format(r#"{"version":"1","name":"Bob"}"#, serde_json::from_str)?;
//! assert_eq!(user.age, 0);
//! # Ok::<(), serde_json::Error>(())
//! ```
//!
//! ## Serializing the struct directly
//!
//! The derive also implements `From` between the struct and its version enum, so serde's
//! `from`/`into` attributes make the struct itself read any version and write the latest one,
//! even when it is nested inside other types:
//!
//! ```rust
//! # use serde_versioned::{FromVersion, Versioned};
//! # use serde::{Deserialize, Serialize};
//! #[derive(Versioned, Serialize, Deserialize, Clone)]
//! #[serde(from = "UserVersion", into = "UserVersion")]
//! #[versioned(versions = [1 => UserV1, 2 => UserV2])]
//! struct User {
//!     pub name: String,
//!     pub age: u32,
//! }
//! # #[derive(Serialize, Deserialize)]
//! # struct UserV1 { name: String }
//! # #[derive(Serialize, Deserialize)]
//! # struct UserV2 { name: String, age: u32 }
//! # impl FromVersion<User> for UserV1 {
//! #     fn convert(self) -> User { User { name: self.name, age: 0 } }
//! # }
//! # impl FromVersion<User> for UserV2 {
//! #     fn convert(self) -> User { User { name: self.name, age: self.age } }
//! # }
//!
//! #[derive(Serialize, Deserialize)]
//! struct Team {
//!     members: Vec<User>,
//! }
//!
//! let team: Team = serde_json::from_str(r#"{"members":[{"version":"1","name":"Bob"}]}"#)?;
//! let json = serde_json::to_string(&team)?;
//! assert_eq!(json, r#"{"members":[{"version":"2","name":"Bob","age":0}]}"#);
//! # Ok::<(), serde_json::Error>(())
//! ```
//!
//! ## Document versions
//!
//! For a file made of many nested types, the version can be written once at the root instead.
//! `#[versioned(document = N)]` marks the root and `N` is the current version of the whole
//! file. A nested type lists the document versions at which its shape changed, ending with
//! `Self`; types that never changed stay plain serde types. Bump `N` whenever any type changes.
//!
//! ```rust
//! # use serde_versioned::{FromVersion, Versioned};
//! # use serde::{Deserialize, Serialize};
//! #[derive(Versioned)]
//! #[versioned(document = 2)]
//! struct Project {
//!     teams: Vec<Team>,
//! }
//!
//! #[derive(Serialize, Deserialize)]
//! struct Team {
//!     members: Vec<User>,
//! }
//!
//! // Documents 1 used UserV1, documents 2 and later use the current shape
//! #[derive(Versioned)]
//! #[versioned(versions = [1 => UserV1, 2 => Self])]
//! struct User {
//!     name: String,
//!     age: u32,
//! }
//!
//! #[derive(Deserialize)]
//! struct UserV1 {
//!     name: String,
//! }
//!
//! impl FromVersion<User> for UserV1 {
//!     fn convert(self) -> User {
//!         User { name: self.name, age: 0 }
//!     }
//! }
//!
//! let project: Project =
//!     serde_json::from_str(r#"{"version":"1","teams":[{"members":[{"name":"Bob"}]}]}"#)?;
//! let json = serde_json::to_string(&project)?;
//! assert_eq!(json, r#"{"version":"2","teams":[{"members":[{"name":"Bob","age":0}]}]}"#);
//! # Ok::<(), serde_json::Error>(())
//! ```

use std::cell::Cell;

use serde::{Serialize, de::DeserializeOwned};

pub use serde_versioned_derive::Versioned;

thread_local! {
    static DOCUMENT_VERSION: Cell<Option<u32>> = const { Cell::new(None) };
}

/// Returns the version of the document being deserialized on this thread, if any.
///
/// Set by `#[versioned(document = N)]` roots while their contents are read or written, and by
/// [`with_document_version`].
///
/// Use it to write a `Deserialize` by hand for a document member the derive does not support,
/// such as an enum. A hand-written member is not checked against the document version when it
/// is written, so bump `document` yourself when its shape changes.
///
/// ```rust
/// use serde::{Deserialize, Deserializer, Serialize, Serializer};
/// use serde_versioned::Versioned;
///
/// #[derive(Versioned)]
/// #[versioned(document = 2)]
/// struct Drawing {
///     shapes: Vec<Shape>,
/// }
///
/// // `remote = "Self"` makes serde generate `Shape::serialize`/`Shape::deserialize` as plain
/// // functions, so the trait impls below can wrap them
/// #[derive(Serialize, Deserialize)]
/// #[serde(remote = "Self")]
/// enum Shape {
///     Circle(f64),
///     Square(f64),
/// }
///
/// impl<'de> Deserialize<'de> for Shape {
///     fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
///         match serde_versioned::document_version() {
///             // Document 1 stored only the radius of a circle
///             Some(1) => f64::deserialize(deserializer).map(Shape::Circle),
///             _ => Shape::deserialize(deserializer),
///         }
///     }
/// }
///
/// impl Serialize for Shape {
///     fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
///         Shape::serialize(self, serializer)
///     }
/// }
///
/// let drawing: Drawing = serde_json::from_str(r#"{"version":"1","shapes":[1.5]}"#)?;
/// let json = serde_json::to_string(&drawing)?;
/// assert_eq!(json, r#"{"version":"2","shapes":[{"Circle":1.5}]}"#);
/// # Ok::<(), serde_json::Error>(())
/// ```
pub fn document_version() -> Option<u32> {
    DOCUMENT_VERSION.with(Cell::get)
}

/// Runs `f` as if inside a document of the given version.
///
/// Use it to deserialize a document member on its own as an older version; outside any
/// document, members are read as their latest version. The previous value is restored when
/// `f` returns or panics.
///
/// ```rust
/// # use serde_versioned::{FromVersion, Versioned};
/// # use serde::Deserialize;
/// #[derive(Versioned)]
/// #[versioned(versions = [1 => UserV1, 3 => Self])]
/// struct User { name: String, age: u32 }
///
/// #[derive(Deserialize)]
/// struct UserV1 { name: String }
///
/// impl FromVersion<User> for UserV1 {
///     fn convert(self) -> User { User { name: self.name, age: 0 } }
/// }
///
/// let user: User = serde_versioned::with_document_version(2, || {
///     serde_json::from_str(r#"{"name":"Bob"}"#)
/// })?;
/// assert_eq!(user.age, 0);
/// # Ok::<(), serde_json::Error>(())
/// ```
pub fn with_document_version<R>(version: u32, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<u32>);
    impl Drop for Restore {
        fn drop(&mut self) {
            DOCUMENT_VERSION.with(|v| v.set(self.0));
        }
    }
    let _restore = Restore(DOCUMENT_VERSION.with(|v| v.replace(Some(version))));
    f()
}

#[doc(hidden)]
pub mod __private {
    use serde::{Deserialize, Deserializer};

    /// Deserializes `T` inside a document of version `N`; used by `#[versioned(document = N)]`.
    pub struct Scoped<T, const N: u32>(pub T);

    impl<'de, T: Deserialize<'de>, const N: u32> Deserialize<'de> for Scoped<T, N> {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            crate::with_document_version(N, || T::deserialize(deserializer)).map(Scoped)
        }
    }
}

/// Trait for converting from a versioned struct to the current struct.
///
/// This trait must be implemented for each version struct to define how it converts
/// to the current version of the struct.
///
/// # Example
///
/// ```rust
/// use serde_versioned::FromVersion;
///
/// struct User { name: String, age: u32 }
/// struct UserV1 { name: String }
///
/// impl FromVersion<User> for UserV1 {
///     fn convert(self) -> User {
///         User { name: self.name, age: 0 }
///     }
/// }
/// ```
pub trait FromVersion<T>: Sized {
    /// Converts a versioned struct instance to the current struct type.
    ///
    /// # Arguments
    ///
    /// * `self` - The versioned struct instance to convert
    ///
    /// # Returns
    ///
    /// The converted current struct instance
    fn convert(self) -> T;
}

/// Trait for handling versioned serialization and deserialization.
///
/// This trait is automatically derived using the `#[derive(Versioned)]` macro.
/// It provides methods to convert between the current struct and its versioned enum.
///
/// # Example
///
/// ```rust
/// # use serde::{Deserialize, Serialize};
/// # use serde_versioned::{FromVersion, Versioned};
/// # #[derive(Versioned)]
/// # #[versioned(versions = [1 => UserV1, 2 => UserV2])]
/// # struct User { name: String, age: u32 }
/// # #[derive(Serialize, Deserialize)]
/// # struct UserV1 { name: String }
/// # #[derive(Serialize, Deserialize)]
/// # struct UserV2 { name: String, age: u32 }
/// # impl FromVersion<User> for UserV1 {
/// #     fn convert(self) -> User { User { name: self.name, age: 0 } }
/// # }
/// # impl FromVersion<User> for UserV2 {
/// #     fn convert(self) -> User { User { name: self.name, age: self.age } }
/// # }
/// // Convert to versioned enum
/// let user = User { name: "Alice".to_string(), age: 30 };
/// let version = user.to_version();
///
/// // Serialize to JSON
/// let json = serde_json::to_string(&version)?;
///
/// // Deserialize and convert back
/// let version: UserVersion = serde_json::from_str(&json)?;
/// let user = User::from_version(version);
/// assert_eq!(user.age, 30);
/// # Ok::<(), serde_json::Error>(())
/// ```
pub trait Versioned: Sized {
    /// The version enum type that represents all versions of this struct.
    ///
    /// This enum is automatically generated by the `#[derive(Versioned)]` macro
    /// and contains variants for each version specified in the `versions` attribute.
    type VersionEnum: DeserializeOwned + Serialize;

    /// Converts a versioned enum instance back to the current struct,
    /// using the `FromVersion` implementation of the stored version.
    fn from_version(version: Self::VersionEnum) -> Self;

    /// Converts the current struct instance to its versioned enum representation.
    ///
    /// This always uses the latest version specified in the `versions` attribute.
    ///
    /// # Returns
    ///
    /// The versioned enum instance representing this struct in its latest version.
    fn to_version(&self) -> Self::VersionEnum;

    /// Deserializes from a string format and converts to the current struct.
    ///
    /// This is a convenience method that combines deserialization and version conversion.
    ///
    /// # Arguments
    ///
    /// * `input` - The string to deserialize from
    /// * `deserializer` - A function that deserializes the string into `Self::VersionEnum`
    ///
    /// # Errors
    ///
    /// Returns the deserializer's error unchanged.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use serde::{Deserialize, Serialize};
    /// # use serde_versioned::{FromVersion, Versioned};
    /// # #[derive(Versioned)]
    /// # #[versioned(versions = [1 => UserV1, 2 => UserV2])]
    /// # struct User { name: String, age: u32 }
    /// # #[derive(Serialize, Deserialize)]
    /// # struct UserV1 { name: String }
    /// # #[derive(Serialize, Deserialize)]
    /// # struct UserV2 { name: String, age: u32 }
    /// # impl FromVersion<User> for UserV1 {
    /// #     fn convert(self) -> User { User { name: self.name, age: 0 } }
    /// # }
    /// # impl FromVersion<User> for UserV2 {
    /// #     fn convert(self) -> User { User { name: self.name, age: self.age } }
    /// # }
    /// let json = r#"{"version":"1","name":"Alice"}"#;
    /// let user = User::from_format(json, serde_json::from_str)?;
    /// assert_eq!(user.age, 0);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    fn from_format<'a, F, E>(input: &'a str, deserializer: F) -> Result<Self, E>
    where
        F: FnOnce(&'a str) -> Result<Self::VersionEnum, E>,
    {
        deserializer(input).map(Self::from_version)
    }

    /// Serializes the current struct to a string format via its versioned enum.
    ///
    /// This is a convenience method that converts the struct to its versioned enum
    /// and then serializes it using the provided serializer function.
    ///
    /// # Arguments
    ///
    /// * `serializer` - A function that serializes `Self::VersionEnum` to the desired format
    ///
    /// # Returns
    ///
    /// * `Ok(T)` - Successfully serialized data
    /// * `Err(E)` - Error during serialization
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use serde::{Deserialize, Serialize};
    /// # use serde_versioned::{FromVersion, Versioned};
    /// # #[derive(Versioned)]
    /// # #[versioned(versions = [1 => UserV1, 2 => UserV2])]
    /// # struct User { name: String, age: u32 }
    /// # #[derive(Serialize, Deserialize)]
    /// # struct UserV1 { name: String }
    /// # #[derive(Serialize, Deserialize)]
    /// # struct UserV2 { name: String, age: u32 }
    /// # impl FromVersion<User> for UserV1 {
    /// #     fn convert(self) -> User { User { name: self.name, age: 0 } }
    /// # }
    /// # impl FromVersion<User> for UserV2 {
    /// #     fn convert(self) -> User { User { name: self.name, age: self.age } }
    /// # }
    /// let user = User { name: "Alice".to_string(), age: 30 };
    /// let json = user.to_format(serde_json::to_string)?;
    /// assert_eq!(json, r#"{"version":"2","name":"Alice","age":30}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    fn to_format<F, T, E>(&self, serializer: F) -> Result<T, E>
    where
        F: FnOnce(&Self::VersionEnum) -> Result<T, E>,
    {
        let version = self.to_version();
        serializer(&version)
    }
}
