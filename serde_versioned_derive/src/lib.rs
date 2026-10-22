//! # `serde_versioned_derive`
//!
//! Procedural macro derive for the `Versioned` trait.
//!
//! This crate provides the `#[derive(Versioned)]` macro that automatically generates
//! the implementation of the `Versioned` trait for structs.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Data, DataStruct, DeriveInput, Fields, Token, parse_macro_input};

/// Derives the `Versioned` trait for a struct.
///
/// This macro generates:
/// - A version enum (e.g., `UserVersion`) with variants for each version
/// - Implementation of `Versioned` trait with `from_version` and `to_version` methods
///
/// # Attributes
///
/// The macro accepts a `versioned` attribute with the following format:
/// ```text
/// #[versioned(versions = [1 => Version1, 2 => Version2, ...])]
/// ```
///
/// Each number is stored in the `"version"` field of the serialized data, so it must never
/// change once data has been written. Numbers must be strictly increasing and the last entry
/// is the latest version. Removing an old entry keeps the other numbers unchanged.
///
/// # Requirements
///
/// - The struct must have named fields and no generic parameters
/// - Every field of the struct must implement `Clone`
/// - The latest version struct must have the same field names as the struct
/// - Each version struct must implement `Serialize`, `Deserialize`, and `FromVersion<CurrentStruct>`
///
/// # Example
///
/// ```rust
/// use serde_versioned::Versioned;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Versioned)]
/// #[versioned(versions = [1 => UserV1, 2 => UserV2])]
/// struct User {
///     pub name: String,
///     pub age: u32,
/// }
///
/// #[derive(Serialize, Deserialize)]
/// struct UserV1 {
///     pub name: String,
/// }
///
/// #[derive(Serialize, Deserialize)]
/// struct UserV2 {
///     pub name: String,
///     pub age: u32,
/// }
///
/// impl serde_versioned::FromVersion<User> for UserV1 {
///     fn convert(self) -> User {
///         User { name: self.name, age: 0 }
///     }
/// }
///
/// impl serde_versioned::FromVersion<User> for UserV2 {
///     fn convert(self) -> User {
///         User { name: self.name, age: self.age }
///     }
/// }
/// ```
#[proc_macro_derive(Versioned, attributes(versioned))]
pub fn derive_versioned(input: TokenStream) -> TokenStream {
    expand(&parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Generates the version enum and the `Versioned` implementation for `input`.
fn expand(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let struct_name = &input.ident;
    let vis = &input.vis;

    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            format!("{struct_name}: Versioned derive macro does not support generic parameters."),
        ));
    }

    // Extract field names for cloning into the latest version struct
    let fields: Vec<_> = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => fields.named.iter().map(|f| &f.ident).collect(),
        Data::Struct(DataStruct {
            fields: Fields::Unnamed(_),
            ..
        }) => {
            return Err(syn::Error::new(
                struct_name.span(),
                format!(
                    "{struct_name}: Versioned derive macro only supports structs with named fields (not tuple structs).\n\nConsider changing:\n  struct {struct_name}(...);\n\nto:\n  struct {struct_name} {{ ... }};"
                ),
            ));
        }
        Data::Struct(DataStruct {
            fields: Fields::Unit,
            ..
        }) => {
            return Err(syn::Error::new(
                struct_name.span(),
                format!(
                    "{struct_name}: Versioned derive macro does not support unit structs. The struct must have at least one named field."
                ),
            ));
        }
        _ => {
            return Err(syn::Error::new(
                struct_name.span(),
                format!(
                    "{struct_name}: Versioned derive macro only supports structs, not enums or unions."
                ),
            ));
        }
    };

    // Extract version structs from the versioned attribute
    let versions = extract_versions(input)?;
    let Some(latest) = versions.last() else {
        return Err(syn::Error::new(
            struct_name.span(),
            format!(
                "No version structs specified for {struct_name}. Please specify at least one version using #[versioned(versions = [1 => Version1, ...])] attribute.\n\nExample:\n  #[versioned(versions = [1 => {struct_name}V1, 2 => {struct_name}V2])]"
            ),
        ));
    };

    // Generate the version enum name (e.g., UserVersion for struct User)
    let version_enum_name = format_ident!("{}Version", struct_name);
    let enum_doc =
        format!("Serialized form of [`{struct_name}`], generated by `#[derive(Versioned)]`.");

    // One variant per version (e.g., Version1(UserV1)), tagged with its number
    let variants: Vec<_> = versions
        .iter()
        .map(|v| format_ident!("Version{}", v.number))
        .collect();
    let tags: Vec<_> = versions.iter().map(|v| v.number.to_string()).collect();
    let variant_docs = tags
        .iter()
        .map(|tag| format!("Stored as `\"version\": \"{tag}\"`."));
    let version_structs = versions.iter().map(|v| &v.path);

    // to_version always writes the latest version
    let latest_variant = format_ident!("Version{}", latest.number);
    let latest_struct = &latest.path;

    Ok(quote! {
        #[doc = #enum_doc]
        #[derive(::serde::Serialize, ::serde::Deserialize)]
        #[serde(tag = "version")]
        #vis enum #version_enum_name {
            #(
                #[doc = #variant_docs]
                #[serde(rename = #tags)]
                #variants(#version_structs),
            )*
        }

        impl ::serde_versioned::Versioned for #struct_name {
            type VersionEnum = #version_enum_name;

            fn from_version(version: Self::VersionEnum) -> Self {
                match version {
                    #(#version_enum_name::#variants(v) => ::serde_versioned::FromVersion::convert(v),)*
                }
            }

            fn to_version(&self) -> Self::VersionEnum {
                #version_enum_name::#latest_variant(#latest_struct {
                    #(#fields: self.#fields.clone(),)*
                })
            }
        }
    })
}

/// Extracts the version list from the `versioned` attribute.
///
/// Returns an empty list when the attribute is missing.
///
/// # Errors
///
/// Returns the parse error when the attribute is malformed.
fn extract_versions(input: &DeriveInput) -> syn::Result<Vec<Version>> {
    match input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("versioned"))
    {
        Some(attr) => Ok(attr.parse_args::<VersionsList>()?.versions),
        None => Ok(Vec::new()),
    }
}

/// One `<number> => <struct>` entry of the versions list.
struct Version {
    /// The number stored in the `"version"` field
    number: u32,
    /// Span of the number, for error messages
    span: Span,
    /// The version struct (an identifier or a path such as `models::UserV1`)
    path: syn::Path,
}

impl Parse for Version {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let number: syn::LitInt = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                "Expected `<number> => <struct>`, e.g. `1 => UserV1`.",
            )
        })?;
        input.parse::<Token![=>]>()?;
        Ok(Self {
            number: number.base10_parse()?,
            span: number.span(),
            path: input.parse()?,
        })
    }
}

/// Structure representing the parsed versions list from the attribute.
struct VersionsList {
    /// Versions in the order they were written
    versions: Vec<Version>,
}

impl Parse for VersionsList {
    /// Parses the `versions = [...]` syntax from the attribute.
    ///
    /// Expected format: `versions = [1 => StructV1, 2 => StructV2, ...]`
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parse the "versions" identifier
        let ident: syn::Ident = input.parse()?;
        if ident != "versions" {
            return Err(syn::Error::new(
                ident.span(),
                format!(
                    "Expected `versions`, found `{ident}`. The correct syntax is: #[versioned(versions = [1 => Version1, 2 => Version2, ...])]"
                ),
            ));
        }

        // Parse the `=` token
        input.parse::<Token![=]>()?;

        // Parse the array brackets and content
        let array_content;
        syn::bracketed!(array_content in input);

        // Parse comma-separated `<number> => <struct>` entries
        let versions: Vec<Version> =
            syn::punctuated::Punctuated::<Version, Token![,]>::parse_terminated(&array_content)?
                .into_iter()
                .collect();

        // Strictly increasing numbers rule out duplicates and make the last entry the latest
        if let Some(pair) = versions
            .windows(2)
            .find(|pair| pair[1].number <= pair[0].number)
        {
            return Err(syn::Error::new(
                pair[1].span,
                format!(
                    "Version numbers must be strictly increasing, but {} follows {}.",
                    pair[1].number, pair[0].number
                ),
            ));
        }

        Ok(Self { versions })
    }
}

#[cfg(test)]
mod tests {
    use super::expand;
    use syn::parse_quote;

    fn error(input: &syn::DeriveInput) -> String {
        expand(input).unwrap_err().to_string()
    }

    #[test]
    fn reports_invalid_input() {
        let cases: [(syn::DeriveInput, &str); 7] = [
            (
                parse_quote! { #[versioned(version = [1 => A])] struct S { x: u32 } },
                "Expected `versions`, found `version`",
            ),
            (
                parse_quote! { #[versioned(versions = [A, B])] struct S { x: u32 } },
                "e.g. `1 => UserV1`",
            ),
            (
                parse_quote! { #[versioned(versions = [1 => A, 1 => B])] struct S { x: u32 } },
                "strictly increasing",
            ),
            (
                parse_quote! { #[versioned(versions = [])] struct S { x: u32 } },
                "No version structs",
            ),
            (parse_quote! { struct S { x: u32 } }, "No version structs"),
            (
                parse_quote! { #[versioned(versions = [1 => A])] struct S<T> { x: T } },
                "generic parameters",
            ),
            (
                parse_quote! { #[versioned(versions = [1 => A])] struct S(u32); },
                "named fields",
            ),
        ];
        for (input, expected) in &cases {
            let message = error(input);
            assert!(
                message.contains(expected),
                "{expected:?} not in {message:?}"
            );
        }
    }
}
