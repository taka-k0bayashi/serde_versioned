//! # `serde_versioned_derive`
//!
//! Procedural macro derive for the `Versioned` trait.
//!
//! This crate provides the `#[derive(Versioned)]` macro that automatically generates
//! the implementation of the `Versioned` trait for structs.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data, DataStruct, Fields, Meta};
use proc_macro2::TokenStream as TokenStream2;

/// Derives the `Versioned` trait for a struct.
///
/// This macro generates:
/// - A version enum (e.g., `UserVersion`) with variants for each version
/// - Implementation of `Versioned` trait with `from_version` and `to_version` methods
///
/// # Attributes
///
/// The macro accepts a `versioned` attribute with the following format:
/// ```rust,ignore
/// #[versioned(versions = [Version1, Version2, ...])]
/// ```
///
/// # Requirements
///
/// - The struct must have named fields (not tuple structs or unit structs)
/// - Each version struct must implement `FromVersion<CurrentStruct>`
/// - Each version struct must implement `Serialize`, `Deserialize`, and `Clone`
///
/// # Panics
///
/// This function will panic if `versions` is empty (which should be caught during compilation),
/// or if the latest version cannot be determined.
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Versioned, Serialize, Deserialize, Clone)]
/// #[versioned(versions = [UserV1, UserV2])]
/// struct User {
///     pub name: String,
///     pub age: u32,
/// }
/// ```
#[proc_macro_derive(Versioned, attributes(versioned))]
pub fn derive_versioned(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;
    let vis = &input.vis;
    
    // Generate the version enum name (e.g., UserVersion for struct User)
    let version_enum_name = syn::Ident::new(
        &format!("{struct_name}Version"),
        struct_name.span()
    );
    
    // Extract version structs from the versioned attribute
    let versions = extract_versions(&input);
    
    // Validate that at least one version is specified
    if versions.is_empty() {
        return syn::Error::new(
            struct_name.span(),
            "Please specify version structs using #[versioned(versions = [...])] attribute"
        )
        .to_compile_error()
        .into();
    }
    
    // Generate enum variants for each version (e.g., Version1(UserV1), Version2(UserV2))
    let version_variants: Vec<_> = versions.iter().map(|(version_num, version_struct)| {
        let version_ident = syn::Ident::new(
            &format!("Version{version_num}"),
            version_struct.span()
        );
        quote! {
            #[serde(rename = #version_num)]
            #version_ident(#version_struct)
        }
    }).collect();
    
    // Generate the version enum definition
    let version_enum = quote! {
        #[derive(serde::Serialize, serde::Deserialize)]
        #[serde(tag = "version")]
        #vis enum #version_enum_name {
            #(#version_variants),*
        }
    };
    
    // Generate match arms for from_version implementation
    let from_version_match_arms: Vec<_> = versions.iter().map(|(version_num, version_struct)| {
        let version_ident = syn::Ident::new(
            &format!("Version{version_num}"),
            version_struct.span()
        );
        quote! {
            #version_enum_name::#version_ident(v) => Ok(serde_versioned::FromVersion::convert(v)),
        }
    }).collect();
    
    // Get the latest version for to_version implementation
    let (latest_version_num, latest_version_struct) = versions.last().unwrap();
    let latest_version_ident = syn::Ident::new(
        &format!("Version{latest_version_num}"),
        latest_version_struct.span()
    );
    
    // Extract field names for cloning into the latest version struct
    let fields = match &input.data {
        Data::Struct(DataStruct { fields: Fields::Named(fields), .. }) => {
            fields.named.iter().map(|f| {
                let field_name = &f.ident;
                quote! { #field_name: self.#field_name.clone() }
            }).collect::<Vec<_>>()
        }
        _ => {
            return syn::Error::new(
                struct_name.span(),
                "Versioned only supports structs with named fields"
            )
            .to_compile_error()
            .into();
        }
    };
    
    // Generate the to_version implementation body
    let to_version_impl = quote! {
        #version_enum_name::#latest_version_ident(#latest_version_struct {
            #(#fields),*
        })
    };
    
    // Combine everything into the final expanded code
    let expanded = quote! {
        #version_enum
        
        impl serde_versioned::Versioned for #struct_name {
            type VersionEnum = #version_enum_name;
            
            fn from_version(version: Self::VersionEnum) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
                match version {
                    #(#from_version_match_arms)*
                }
            }
            
            fn to_version(&self) -> Self::VersionEnum {
                #to_version_impl
            }
        }
    };
    
    TokenStream::from(expanded)
}

/// Extracts version struct names from the `versioned` attribute.
///
/// Parses the `#[versioned(versions = [V1, V2, ...])]` attribute and returns
/// a vector of tuples containing (`version_number`, `struct_ident`).
///
/// # Arguments
///
/// * `input` - The derive input containing the struct definition
///
/// # Returns
///
/// A vector of tuples where each tuple contains:
/// - A string version number (e.g., "1", "2")
/// - The identifier of the version struct
fn extract_versions(input: &DeriveInput) -> Vec<(String, syn::Ident)> {
    let mut versions = Vec::new();
    
    // Search for the versioned attribute
    for attr in &input.attrs {
        if attr.path().is_ident("versioned")
            && let Meta::List(meta_list) = &attr.meta {
            // Parse the format: versioned(versions = [SettingV1, SettingV2])
            let tokens: TokenStream2 = meta_list.tokens.clone();
            let result = syn::parse2::<VersionsList>(tokens);
            if let Ok(versions_list) = result {
                versions = versions_list.versions;
            }
        }
    }
    
    versions
}

/// Structure representing the parsed versions list from the attribute.
struct VersionsList {
    /// Vector of (`version_number`, `struct_identifier`) tuples
    versions: Vec<(String, syn::Ident)>,
}

impl syn::parse::Parse for VersionsList {
    /// Parses the `versions = [...]` syntax from the attribute.
    ///
    /// Expected format: `versions = [StructV1, StructV2, ...]`
    ///
    /// # Returns
    ///
    /// A `VersionsList` containing version numbers (starting from 1) and their corresponding struct identifiers.
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Parse the "versions" identifier
        let ident: syn::Ident = input.parse()?;
        if ident != "versions" {
            return Err(syn::Error::new(ident.span(), "expected `versions`"));
        }
        
        // Parse the `=` token
        input.parse::<syn::Token![=]>()?;
        
        // Parse the array brackets and content
        let array_content;
        syn::bracketed!(array_content in input);
        
        // Parse comma-separated list of expressions (struct identifiers)
        let elems = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated(&array_content)?;
        
        let mut versions = Vec::new();
        // Convert each struct identifier to a version number (1-indexed)
        for (idx, elem) in elems.iter().enumerate() {
            if let syn::Expr::Path(path) = elem
                && let Some(ident) = path.path.get_ident() {
                let version_num = (idx + 1).to_string();
                versions.push((version_num, ident.clone()));
            }
        }
        Ok(Self { versions })
    }
}
