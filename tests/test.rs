use serde::{Deserialize, Serialize};
use serde_versioned::{FromVersion, Versioned};

// Version 3 was dropped: removing an entry never renumbers the others.
#[derive(Versioned, Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(from = "UserVersion", into = "UserVersion")]
#[versioned(versions = [1 => UserV1, 2 => UserV2, 4 => models::UserV4])]
struct User {
    pub name: String,
    pub age: u32,
    pub email: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct UserV1 {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct UserV2 {
    pub name: String,
    pub age: u32,
}

// Version structs can be referenced by path
mod models {
    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct UserV4 {
        pub name: String,
        pub age: u32,
        pub email: Option<String>,
    }
}

impl FromVersion<User> for UserV1 {
    fn convert(self) -> User {
        User {
            name: self.name,
            age: 0, // default
            email: None,
        }
    }
}

impl FromVersion<User> for UserV2 {
    fn convert(self) -> User {
        User {
            name: self.name,
            age: self.age,
            email: None,
        }
    }
}

impl FromVersion<User> for models::UserV4 {
    fn convert(self) -> User {
        User {
            name: self.name,
            age: self.age,
            email: self.email,
        }
    }
}

fn alice() -> User {
    User {
        name: "Alice".to_string(),
        age: 30,
        email: Some("alice@example.com".to_string()),
    }
}

#[test]
fn test_writes_latest_version() {
    let json = alice().to_format(serde_json::to_string).unwrap();
    assert_eq!(
        json,
        r#"{"version":"4","name":"Alice","age":30,"email":"alice@example.com"}"#
    );
}

#[test]
fn test_roundtrip_all_formats() {
    let user = alice();
    assert_eq!(User::from_version(user.to_version()), user);

    let json = user.to_format(serde_json::to_string).unwrap();
    let from_json = User::from_format(&json, serde_json::from_str).unwrap();
    assert_eq!(from_json, user);

    let toml_str = user.to_format(toml::to_string).unwrap();
    let from_toml = User::from_format(&toml_str, toml::from_str).unwrap();
    assert_eq!(from_toml, user);

    let yaml_str = user.to_format(serde_yaml::to_string).unwrap();
    let from_yaml = User::from_format(&yaml_str, serde_yaml::from_str).unwrap();
    assert_eq!(from_yaml, user);
}

#[test]
fn test_older_versions_are_converted() {
    let v1_json = r#"{"version":"1","name":"Bob"}"#;
    let v1 = User::from_format(v1_json, serde_json::from_str).unwrap();
    assert_eq!(v1.name, "Bob");
    assert_eq!(v1.age, 0);

    let v2_json = r#"{"version":"2","name":"Carol","age":25}"#;
    let v2 = User::from_format(v2_json, serde_json::from_str).unwrap();
    assert_eq!(v2.age, 25);
    assert_eq!(v2.email, None);

    let v1_toml = "version = \"1\"\nname = \"Henry\"\n";
    let from_toml = User::from_format(v1_toml, toml::from_str).unwrap();
    assert_eq!(from_toml.name, "Henry");

    let v1_yaml = "version: \"1\"\nname: Kate\n";
    let from_yaml = User::from_format(v1_yaml, serde_yaml::from_str).unwrap();
    assert_eq!(from_yaml.name, "Kate");
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Team {
    owner: User,
    members: Vec<User>,
}

#[test]
fn test_nested_with_serde_from_into() {
    let json = r#"{"owner":{"version":"1","name":"Bob"},"members":[{"version":"2","name":"Carol","age":25}]}"#;
    let team: Team = serde_json::from_str(json).unwrap();
    assert_eq!(team.owner.age, 0);
    assert_eq!(team.members[0].age, 25);

    let written = serde_json::to_string(&team).unwrap();
    assert!(written.contains(r#"{"version":"4","name":"Bob","age":0,"email":null}"#));
    assert_eq!(serde_json::from_str::<Team>(&written).unwrap(), team);
}

#[test]
fn test_invalid_input_is_rejected() {
    let invalid_json = r#"{"invalid": json}"#;
    assert!(User::from_format(invalid_json, serde_json::from_str).is_err());

    // Unknown and dropped versions fail at deserialization
    for version in ["3", "99"] {
        let json = format!(r#"{{"version":"{version}","name":"Test"}}"#);
        assert!(User::from_format(&json, serde_json::from_str).is_err());
    }
}
