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
/// - `From` conversions in both directions between the struct and the version enum, so the
///   struct can use `#[serde(from = "UserVersion", into = "UserVersion")]`
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
/// # Document versions
///
/// ```text
/// #[versioned(document = 3)]                          // root: writes "version": "3" once
/// #[versioned(versions = [1 => UserV1, 3 => Self])]   // member: no tag of its own
/// ```
///
/// A list ending with `Self` makes the struct a document member: the numbers are document
/// versions, and the struct is read with the entry for the enclosing document's version
/// (the largest number not above it). Members and roots implement `Serialize` and
/// `Deserialize` themselves, so do not derive them; `#[serde(...)]` attributes still apply.
/// A root may also take a `versions` list ending with `Self` when its own fields change.
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
#[proc_macro_derive(Versioned, attributes(versioned, serde))]
pub fn derive_versioned(input: TokenStream) -> TokenStream {
    expand(&parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Checks the input and dispatches on the kind of `versioned` attribute.
fn expand(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let struct_name = &input.ident;

    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            format!("{struct_name}: Versioned derive macro does not support generic parameters."),
        ));
    }

    let fields: Vec<&syn::Field> = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => fields.named.iter().collect(),
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

    let args = extract_args(input)?;
    let scoped = args.versions.last().is_some_and(Version::is_self);
    if let Some((document, span)) = args.document {
        let versions = if args.versions.is_empty() {
            vec![Version {
                number: 1,
                span,
                path: syn::parse_quote!(Self),
            }]
        } else if scoped {
            args.versions
        } else {
            return Err(syn::Error::new(
                span,
                "The `versions` list of a document must end with `Self`.",
            ));
        };
        let latest = &versions[versions.len() - 1];
        if latest.number > document {
            return Err(syn::Error::new(
                latest.span,
                format!(
                    "Version {} is greater than the document version {document}.",
                    latest.number
                ),
            ));
        }
        return Ok(expand_document(input, &fields, &versions, document));
    }
    if scoped {
        return Ok(expand_member(input, &fields, &args.versions));
    }
    expand_tagged(input, &fields, &args.versions)
}

/// Generates the version enum and the `Versioned` implementation (a tag on every value).
fn expand_tagged(
    input: &DeriveInput,
    fields: &[&syn::Field],
    versions: &[Version],
) -> syn::Result<TokenStream2> {
    let struct_name = &input.ident;
    let vis = &input.vis;
    // Field names for cloning into the latest version struct
    let fields: Vec<_> = fields.iter().map(|f| &f.ident).collect();

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

        // Lets the struct use #[serde(from = "...Version", into = "...Version")]
        impl ::core::convert::From<#version_enum_name> for #struct_name {
            fn from(version: #version_enum_name) -> Self {
                <Self as ::serde_versioned::Versioned>::from_version(version)
            }
        }

        impl ::core::convert::From<#struct_name> for #version_enum_name {
            fn from(value: #struct_name) -> Self {
                #version_enum_name::#latest_variant(#latest_struct {
                    #(#fields: value.#fields,)*
                })
            }
        }
    })
}

/// Generated pieces shared by document members and roots: a `serde(remote)` copy of the
/// struct that (de)serializes its current shape, and the dispatch on the document version.
fn scoped_parts(
    input: &DeriveInput,
    fields: &[&syn::Field],
    versions: &[Version],
) -> (TokenStream2, TokenStream2) {
    let name = input.ident.to_string();
    let is_serde = |attr: &&syn::Attribute| attr.path().is_ident("serde");
    let container_attrs = input.attrs.iter().filter(is_serde);
    let shadow_fields = fields.iter().map(|f| {
        let attrs = f.attrs.iter().filter(is_serde);
        let (ident, ty) = (&f.ident, &f.ty);
        quote! { #(#attrs)* #ident: #ty }
    });
    let shadow = quote! {
        #[allow(dead_code)]
        #[derive(::serde::Serialize, ::serde::Deserialize)]
        #[serde(remote = #name)]
        #(#container_attrs)*
        struct __Shadow { #(#shadow_fields,)* }
    };

    // Newest entry first, so the first match is the largest number not above the version
    let arms = versions.iter().rev().map(|v| {
        let number = v.number;
        let read = if v.is_self() {
            quote! { __Shadow::deserialize(deserializer) }
        } else {
            let path = &v.path;
            quote! {
                <#path as ::serde::Deserialize>::deserialize(deserializer)
                    .map(::serde_versioned::FromVersion::convert)
            }
        };
        quote! { ::core::option::Option::Some(v) if v >= #number => #read, }
    });
    let dispatch = quote! {
        match ::serde_versioned::document_version() {
            ::core::option::Option::None => __Shadow::deserialize(deserializer),
            #(#arms)*
            ::core::option::Option::Some(v) => ::core::result::Result::Err(
                <D::Error as ::serde::de::Error>::custom(::core::format_args!(
                    "`{}` does not exist in document version {}", #name, v
                )),
            ),
        }
    };
    (shadow, dispatch)
}

/// Generates `Serialize`/`Deserialize` for a document member, which has no tag of its own.
fn expand_member(
    input: &DeriveInput,
    fields: &[&syn::Field],
    versions: &[Version],
) -> TokenStream2 {
    let struct_name = &input.ident;
    let (shadow, dispatch) = scoped_parts(input, fields, versions);
    let name = struct_name.to_string();
    let latest = versions[versions.len() - 1].number;
    quote! {
        const _: () = {
            #shadow

            impl ::serde::Serialize for #struct_name {
                fn serialize<S: ::serde::Serializer>(
                    &self,
                    serializer: S,
                ) -> ::core::result::Result<S::Ok, S::Error> {
                    // Writing the current shape under an older document label could not be
                    // read back, so the enclosing root must have been bumped
                    if let ::core::option::Option::Some(v) = ::serde_versioned::document_version() {
                        if v < #latest {
                            return ::core::result::Result::Err(
                                <S::Error as ::serde::ser::Error>::custom(::core::format_args!(
                                    "`{}` is at document version {} but is written in document version {}; bump `document` of the enclosing root",
                                    #name, #latest, v
                                )),
                            );
                        }
                    }
                    __Shadow::serialize(self, serializer)
                }
            }

            impl<'de> ::serde::Deserialize<'de> for #struct_name {
                fn deserialize<D: ::serde::Deserializer<'de>>(
                    deserializer: D,
                ) -> ::core::result::Result<Self, D::Error> {
                    #dispatch
                }
            }
        };
    }
}

/// Generates `Serialize`/`Deserialize` for a document root, which writes `"version"` once
/// and makes every member below it read with that version.
fn expand_document(
    input: &DeriveInput,
    fields: &[&syn::Field],
    versions: &[Version],
    document: u32,
) -> TokenStream2 {
    let struct_name = &input.ident;
    let (shadow, dispatch) = scoped_parts(input, fields, versions);
    let latest_tag = document.to_string();
    let numbers: Vec<_> = (1..=document).collect();
    let tags = numbers.iter().map(u32::to_string);
    let variants: Vec<_> = numbers.iter().map(|n| format_ident!("V{n}")).collect();
    quote! {
        const _: () = {
            #shadow

            struct __Body(#struct_name);

            impl<'de> ::serde::Deserialize<'de> for __Body {
                fn deserialize<D: ::serde::Deserializer<'de>>(
                    deserializer: D,
                ) -> ::core::result::Result<Self, D::Error> {
                    (#dispatch).map(__Body)
                }
            }

            struct __BodyRef<'a>(&'a #struct_name);

            impl ::serde::Serialize for __BodyRef<'_> {
                fn serialize<S: ::serde::Serializer>(
                    &self,
                    serializer: S,
                ) -> ::core::result::Result<S::Ok, S::Error> {
                    ::serde_versioned::with_document_version(#document, || {
                        __Shadow::serialize(self.0, serializer)
                    })
                }
            }

            #[derive(::serde::Serialize)]
            #[serde(tag = "version")]
            enum __DocumentRef<'a> {
                #[serde(rename = #latest_tag)]
                Latest(__BodyRef<'a>),
            }

            // Every document version reads the body with that version in scope
            #[derive(::serde::Deserialize)]
            #[serde(tag = "version")]
            enum __Document {
                #(
                    #[serde(rename = #tags)]
                    #variants(::serde_versioned::__private::Scoped<__Body, #numbers>),
                )*
            }

            impl ::serde::Serialize for #struct_name {
                fn serialize<S: ::serde::Serializer>(
                    &self,
                    serializer: S,
                ) -> ::core::result::Result<S::Ok, S::Error> {
                    ::serde::Serialize::serialize(
                        &__DocumentRef::Latest(__BodyRef(self)),
                        serializer,
                    )
                }
            }

            impl<'de> ::serde::Deserialize<'de> for #struct_name {
                fn deserialize<D: ::serde::Deserializer<'de>>(
                    deserializer: D,
                ) -> ::core::result::Result<Self, D::Error> {
                    ::core::result::Result::Ok(
                        match <__Document as ::serde::Deserialize>::deserialize(deserializer)? {
                            #(__Document::#variants(document) => document.0.0,)*
                        },
                    )
                }
            }
        };
    }
}

/// Extracts the arguments of the `versioned` attribute.
///
/// Returns empty arguments when the attribute is missing.
///
/// # Errors
///
/// Returns the parse error when the attribute is malformed.
fn extract_args(input: &DeriveInput) -> syn::Result<Args> {
    match input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("versioned"))
    {
        Some(attr) => attr.parse_args::<Args>(),
        None => Ok(Args::default()),
    }
}

/// One `<number> => <struct>` entry of the versions list.
struct Version {
    /// The number stored in the `"version"` field
    number: u32,
    /// Span of the number, for error messages
    span: Span,
    /// The version struct (an identifier or a path such as `models::UserV1`), or `Self`
    path: syn::Path,
}

impl Version {
    /// Whether this entry is `Self`, the struct's own current shape.
    fn is_self(&self) -> bool {
        self.path.is_ident("Self")
    }
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

/// Parsed arguments of the `versioned` attribute.
#[derive(Default)]
struct Args {
    /// Versions in the order they were written
    versions: Vec<Version>,
    /// `document = N`, with the span of `N`
    document: Option<(u32, Span)>,
}

impl Parse for Args {
    /// Parses `versions = [1 => StructV1, ...]` and `document = N`, separated by commas.
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = Self::default();
        loop {
            let ident: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            if ident == "versions" {
                args.versions = parse_versions(input)?;
            } else if ident == "document" {
                let number: syn::LitInt = input.parse()?;
                args.document = Some((number.base10_parse()?, number.span()));
            } else {
                return Err(syn::Error::new(
                    ident.span(),
                    format!(
                        "Expected `versions` or `document`, found `{ident}`. The correct syntax is: #[versioned(versions = [1 => Version1, 2 => Version2, ...])]"
                    ),
                ));
            }
            if input.is_empty() {
                return Ok(args);
            }
            input.parse::<Token![,]>()?;
        }
    }
}

/// Parses the `[1 => StructV1, 2 => StructV2, ...]` part of `versions = [...]`.
fn parse_versions(input: ParseStream) -> syn::Result<Vec<Version>> {
    let array_content;
    syn::bracketed!(array_content in input);

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
    if let Some(v) = versions.iter().rev().skip(1).find(|v| v.is_self()) {
        return Err(syn::Error::new(
            v.span,
            "`Self` must be the last entry of `versions`.",
        ));
    }

    Ok(versions)
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
        let cases: [(syn::DeriveInput, &str); 10] = [
            (
                parse_quote! { #[versioned(version = [1 => A])] struct S { x: u32 } },
                "Expected `versions` or `document`, found `version`",
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
            (
                parse_quote! { #[versioned(versions = [1 => Self, 2 => A])] struct S { x: u32 } },
                "`Self` must be the last",
            ),
            (
                parse_quote! { #[versioned(document = 2, versions = [1 => A])] struct S { x: u32 } },
                "must end with `Self`",
            ),
            (
                parse_quote! { #[versioned(document = 2, versions = [1 => A, 3 => Self])] struct S { x: u32 } },
                "greater than the document version 2",
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
