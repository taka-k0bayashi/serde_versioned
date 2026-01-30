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
    
    // JSONからデシリアライズ
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
    
    // JSONからデシリアライズ
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
