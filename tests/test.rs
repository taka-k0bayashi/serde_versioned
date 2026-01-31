extern crate serde_versioned;
extern crate serde;

use serde_versioned::Versioned;
use serde::{Deserialize, Serialize};

#[derive(Versioned, Serialize, Deserialize, Debug, PartialEq, Clone)]
#[versioned(versions = [UserV1, UserV2])]
struct User {
    pub name: String,
    pub age: u32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UserV1 {
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UserV2 {
    pub name: String,
    pub age: u32,
}

impl serde_versioned::FromVersion<User> for UserV1 {
    fn convert(self) -> User {
        User {
            name: self.name,
            age: 0, // default
        }
    }
}

impl serde_versioned::FromVersion<User> for UserV2 {
    fn convert(self) -> User {
        User {
            name: self.name,
            age: self.age,
        }
    }
}

#[test]
fn test_version_conversion() {
    let user = User {
        name: "Alice".to_string(),
        age: 30,
    };
    
    let version = user.to_version();
    let restored = User::from_version(version).unwrap();
    
    assert_eq!(user, restored);
}

#[test]
fn test_v1_to_current() {
    let v1 = UserV1 {
        name: "Bob".to_string(),
    };
    
    let user: User = serde_versioned::FromVersion::convert(v1);
    assert_eq!(user.name, "Bob");
    assert_eq!(user.age, 0);
}

#[test]
fn test_v2_to_current() {
    let v2 = UserV2 {
        name: "Charlie".to_string(),
        age: 25,
    };
    
    let user: User = serde_versioned::FromVersion::convert(v2);
    assert_eq!(user.name, "Charlie");
    assert_eq!(user.age, 25);
}

#[test]
fn test_serialization_json() {
    let user = User {
        name: "David".to_string(),
        age: 35,
    };
    
    let version = user.to_version();
    let json = serde_json::to_string(&version).unwrap();
    
    // Deserialize from JSON
    let version_restored: UserVersion = serde_json::from_str(&json).unwrap();
    let user_restored = User::from_version(version_restored).unwrap();
    
    assert_eq!(user, user_restored);
}

#[test]
fn test_serialization_from_format() {
    let user = User {
        name: "David".to_string(),
        age: 35,
    };
    
    let json = user.to_format(serde_json::to_string).unwrap();
    
    // Deserialize from JSON
    let version_restored: UserVersion = serde_json::from_str(&json).unwrap();
    let user_restored = User::from_version(version_restored).unwrap();
    
    assert_eq!(user, user_restored);
}

#[test]
fn test_from_format_json() {
    let v1_json = r#"{"version":"1","name":"Eve"}"#;
    
    let user = User::from_format(v1_json, serde_json::from_str).unwrap();
    assert_eq!(user.name, "Eve");
    assert_eq!(user.age, 0);
}

#[test]
fn test_to_format_json() {
    let user = User {
        name: "Frank".to_string(),
        age: 40,
    };
    
    let json = user.to_format(serde_json::to_string).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    
    assert_eq!(parsed["version"], "2");
    assert_eq!(parsed["name"], "Frank");
    assert_eq!(parsed["age"], 40);
}

// TOML format tests
#[test]
fn test_serialization_toml() {
    let user = User {
        name: "Grace".to_string(),
        age: 28,
    };
    
    let version = user.to_version();
    let toml_str = toml::to_string(&version).unwrap();
    
    // Deserialize from TOML
    let version_restored: UserVersion = toml::from_str(&toml_str).unwrap();
    let user_restored = User::from_version(version_restored).unwrap();
    
    assert_eq!(user, user_restored);
}

#[test]
fn test_from_format_toml() {
    let v1_toml = r#"version = "1"
name = "Henry"
"#;
    
    let user = User::from_format(v1_toml, toml::from_str).unwrap();
    assert_eq!(user.name, "Henry");
    assert_eq!(user.age, 0);
}

#[test]
fn test_to_format_toml() {
    let user = User {
        name: "Iris".to_string(),
        age: 33,
    };
    
    let toml_str = user.to_format(toml::to_string).unwrap();
    let parsed: toml::Value = toml::from_str(&toml_str).unwrap();
    
    assert_eq!(parsed["version"].as_str(), Some("2"));
    assert_eq!(parsed["name"].as_str(), Some("Iris"));
    assert_eq!(parsed["age"].as_integer(), Some(33));
}

// YAML format tests
#[test]
fn test_serialization_yaml() {
    let user = User {
        name: "Jack".to_string(),
        age: 45,
    };
    
    let version = user.to_version();
    let yaml_str = serde_yaml::to_string(&version).unwrap();
    
    // Deserialize from YAML
    let version_restored: UserVersion = serde_yaml::from_str(&yaml_str).unwrap();
    let user_restored = User::from_version(version_restored).unwrap();
    
    assert_eq!(user, user_restored);
}

#[test]
fn test_from_format_yaml() {
    let v1_yaml = r#"version: "1"
name: "Kate"
"#;
    
    let user = User::from_format(v1_yaml, serde_yaml::from_str).unwrap();
    assert_eq!(user.name, "Kate");
    assert_eq!(user.age, 0);
}

#[test]
fn test_to_format_yaml() {
    let user = User {
        name: "Liam".to_string(),
        age: 22,
    };
    
    let yaml_str = user.to_format(serde_yaml::to_string).unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml_str).unwrap();
    
    assert_eq!(parsed["version"].as_str(), Some("2"));
    assert_eq!(parsed["name"].as_str(), Some("Liam"));
    assert_eq!(parsed["age"].as_u64(), Some(22));
}

// Roundtrip tests for all formats
#[test]
fn test_roundtrip_all_formats() {
    let user = User {
        name: "Mia".to_string(),
        age: 29,
    };
    
    // JSON roundtrip
    let json = user.to_format(serde_json::to_string).unwrap();
    let user_from_json = User::from_format(&json, serde_json::from_str).unwrap();
    assert_eq!(user, user_from_json);
    
    // TOML roundtrip
    let toml_str = user.to_format(toml::to_string).unwrap();
    let user_from_toml = User::from_format(&toml_str, toml::from_str).unwrap();
    assert_eq!(user, user_from_toml);
    
    // YAML roundtrip
    let yaml_str = user.to_format(serde_yaml::to_string).unwrap();
    let user_from_yaml = User::from_format(&yaml_str, serde_yaml::from_str).unwrap();
    assert_eq!(user, user_from_yaml);
}

// Error handling tests
#[test]
fn test_deserialize_error() {
    let invalid_json = r#"{"invalid": json}"#;
    
    let result = User::from_format(invalid_json, serde_json::from_str);
    assert!(result.is_err());
    
    let error = result.unwrap_err();
    assert!(error.is_deserialize());
    assert!(!error.is_version_conversion());
    
    // Check that error message contains useful information
    let error_msg = error.to_string();
    assert!(error_msg.contains("Deserialization error"));
}

#[test]
fn test_deserialize_error_with_input() {
    let invalid_json = r#"{"version":"1","name":}"#;
    
    let result = User::from_format(invalid_json, serde_json::from_str);
    assert!(result.is_err());
    
    let error = result.unwrap_err();
    if let serde_versioned::FormatError::Deserialize { input, .. } = error {
        assert!(input.is_some());
        let input_str = input.unwrap();
        assert!(input_str.contains("version"));
    } else {
        panic!("Expected Deserialize error");
    }
}

#[test]
fn test_invalid_version_format() {
    // Test with invalid version number
    let invalid_version_json = r#"{"version":"99","name":"Test"}"#;
    
    let result: Result<UserVersion, _> = serde_json::from_str(invalid_version_json);
    // This should fail at deserialization level, not conversion level
    assert!(result.is_err());
}

#[test]
fn test_error_display_format() {
    use serde_versioned::{FormatError, VersionConversionError};
    use std::error::Error;
    
    // Test VersionConversionError display
    let source_error = Box::new(std::io::Error::other("Test error")) as Box<dyn Error + Send + Sync>;
    
    let vc_error = VersionConversionError::new("1", source_error);
    let error_msg = vc_error.to_string();
    assert!(error_msg.contains("Failed to convert from version 1"));
    assert!(error_msg.contains("Test error"));
    
    // Test VersionConversionError with context
    let source_error2 = Box::new(std::io::Error::other("Test error 2")) as Box<dyn Error + Send + Sync>;
    
    let vc_error2 = VersionConversionError::with_context(
        "2",
        source_error2,
        "Additional context"
    );
    let error_msg2 = vc_error2.to_string();
    assert!(error_msg2.contains("Failed to convert from version 2"));
    assert!(error_msg2.contains("Additional context"));
    
    // Test FormatError display
    let invalid_json = r#"{"invalid": json}"#;
    let deser_error: serde_json::Error = serde_json::from_str::<serde_json::Value>(invalid_json).unwrap_err();
    let format_error = FormatError::deserialize(deser_error, Some("test input".to_string()));
    let format_error_msg = format_error.to_string();
    assert!(format_error_msg.contains("Deserialization error"));
    assert!(format_error_msg.contains("test input"));
}

#[test]
fn test_error_source_chain() {
    use serde_versioned::{FormatError, VersionConversionError};
    use std::error::Error;
    
    // Test that error source chain works correctly
    let source_error = Box::new(std::io::Error::other("Original error")) as Box<dyn Error + Send + Sync>;
    
    let vc_error = VersionConversionError::new("1", source_error);
    assert!(vc_error.source().is_some());
    
    let format_error: FormatError<std::io::Error> = FormatError::VersionConversion(vc_error);
    assert!(format_error.source().is_some());
}

#[test]
fn test_version_conversion_error_version_accessor() {
    use serde_versioned::VersionConversionError;
    use std::error::Error;
    
    let source_error = Box::new(std::io::Error::other("Test")) as Box<dyn Error + Send + Sync>;
    
    let vc_error = VersionConversionError::new("3", source_error);
    assert_eq!(vc_error.version(), "3");
}
