//! Document versions: one `"version"` at the root, members read with it.

use serde::{Deserialize, Serialize};
use serde_versioned::{FromVersion, Versioned, document_version, with_document_version};

#[derive(Versioned, Debug, PartialEq)]
#[versioned(document = 3)]
struct Project {
    name: String,
    teams: Vec<Team>,
    settings: Settings,
    plugins: Vec<Plugin>,
}

// Never changed: a plain serde type
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Team {
    members: Vec<User>,
}

#[derive(Versioned, Debug, PartialEq)]
#[versioned(versions = [1 => UserV1, 3 => Self])]
struct User {
    name: String,
    age: u32,
}

#[derive(Deserialize)]
struct UserV1 {
    name: String,
}

impl FromVersion<User> for UserV1 {
    fn convert(self) -> User {
        User {
            name: self.name,
            age: 0,
        }
    }
}

#[derive(Versioned, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[versioned(versions = [1 => SettingsV1, 2 => Self])]
struct Settings {
    auto_save: bool,
    theme: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsV1 {
    auto_save: bool,
}

impl FromVersion<Settings> for SettingsV1 {
    fn convert(self) -> Settings {
        Settings {
            auto_save: self.auto_save,
            theme: "light".to_string(),
        }
    }
}

// A nested document with its own numbering, which also changed its own fields
#[derive(Versioned, Debug, PartialEq)]
#[versioned(document = 2, versions = [1 => PluginV1, 2 => Self])]
struct Plugin {
    id: String,
    enabled: bool,
}

#[derive(Deserialize)]
struct PluginV1 {
    id: String,
}

impl FromVersion<Plugin> for PluginV1 {
    fn convert(self) -> Plugin {
        Plugin {
            id: self.id,
            enabled: true,
        }
    }
}

// Files written by each document version. Never edit these: if the latest one no longer
// matches what is written, the format changed and the document version must be bumped.
const V1: &str = r#"{"version":"1","name":"p","teams":[{"members":[{"name":"A"}]}],"settings":{"autoSave":true},"plugins":[{"version":"1","id":"x"}]}"#;
const V2: &str = r#"{"version":"2","name":"p","teams":[{"members":[{"name":"A"}]}],"settings":{"autoSave":true,"theme":"light"},"plugins":[{"version":"2","id":"x","enabled":true}]}"#;
const V3: &str = r#"{"version":"3","name":"p","teams":[{"members":[{"name":"A","age":0}]}],"settings":{"autoSave":true,"theme":"light"},"plugins":[{"version":"2","id":"x","enabled":true}]}"#;

fn project() -> Project {
    Project {
        name: "p".to_string(),
        teams: vec![Team {
            members: vec![User {
                name: "A".to_string(),
                age: 0,
            }],
        }],
        settings: Settings {
            auto_save: true,
            theme: "light".to_string(),
        },
        plugins: vec![Plugin {
            id: "x".to_string(),
            enabled: true,
        }],
    }
}

#[test]
fn writes_the_latest_snapshot() {
    assert_eq!(serde_json::to_string(&project()).unwrap(), V3);
}

#[test]
fn reads_every_document_version() {
    for json in [V1, V2, V3] {
        let read: Project = serde_json::from_str(json).unwrap();
        assert_eq!(read, project(), "{json}");
    }
}

#[test]
fn round_trips_through_toml_and_yaml() {
    let toml = toml::to_string(&project()).unwrap();
    assert_eq!(toml::from_str::<Project>(&toml).unwrap(), project());
    let yaml = serde_yaml_ng::to_string(&project()).unwrap();
    assert_eq!(
        serde_yaml_ng::from_str::<Project>(&yaml).unwrap(),
        project()
    );
}

#[test]
fn version_field_may_come_last() {
    let json = r#"{"name":"p","teams":[{"members":[{"name":"A"}]}],"settings":{"autoSave":true},"plugins":[],"version":"1"}"#;
    let read: Project = serde_json::from_str(json).unwrap();
    assert_eq!(read.teams[0].members[0].age, 0);
}

#[test]
fn members_read_alone() {
    let latest: User = serde_json::from_str(r#"{"name":"A","age":5}"#).unwrap();
    assert_eq!(latest.age, 5);
    let old: User = with_document_version(2, || serde_json::from_str(r#"{"name":"A"}"#)).unwrap();
    assert_eq!(old.age, 0);
    let missing = with_document_version(0, || serde_json::from_str::<User>("{}")).unwrap_err();
    assert!(
        missing
            .to_string()
            .contains("does not exist in document version 0")
    );
}

#[test]
fn rejects_unknown_versions_and_restores_scope() {
    let future = V3.replace(r#""version":"3""#, r#""version":"4""#);
    assert!(serde_json::from_str::<Project>(&future).is_err());
    // A member failing deep inside the document must not leave the version set
    assert!(
        serde_json::from_str::<Project>(r#"{"version":"1","teams":[{"members":[{}]}]}"#).is_err()
    );
    assert_eq!(document_version(), None);
}
