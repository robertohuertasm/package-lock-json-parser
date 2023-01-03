use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::instrument;

#[derive(Debug, Error)]
#[error("package-lock.json error")]
pub enum PackageLockJsonError {
    #[error("Error parsing file: {0}")]
    ParseError(#[from] serde_json::Error),
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, Eq, PartialEq)]
pub struct PackageLockJson {
    name: String,
    version: String,
    #[serde(rename = "lockfileVersion")]
    lockfile_version: u32,
    dependencies: Option<HashMap<String, V1Dependency>>,
    #[serde(deserialize_with = "deserialize_packages", default)]
    packages: Option<HashMap<String, V2Dependency>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Default)]
pub struct V1Dependency {
    version: String,
    resolved: String,
    integrity: String,
    #[serde(default)]
    bundled: bool,
    #[serde(rename = "dev", default)]
    is_dev: bool,
    #[serde(rename = "optional", default)]
    is_optional: bool,
    requires: Option<HashMap<String, String>>,
    dependencies: Option<HashMap<String, V1Dependency>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Default)]
pub struct V2Dependency {
    version: String,
    resolved: String,
    integrity: String,
    #[serde(default)]
    bundled: bool,
    #[serde(rename = "dev", default)]
    is_dev: bool,
    #[serde(rename = "optional", default)]
    is_optional: bool,
    #[serde(rename = "devOptional", default)]
    is_dev_optional: bool,
    #[serde(rename = "inBundle", default)]
    is_in_bundle: bool,
    #[serde(rename = "hasInstallScript", default)]
    has_install_script: bool,
    #[serde(rename = "hasShrinkwrap", default)]
    has_shrink_wrap: bool,
    dependencies: Option<HashMap<String, String>>,
    #[serde(rename = "optionalDependencies")]
    optional_dependencies: Option<HashMap<String, String>>,
    #[serde(rename = "peerDependencies")]
    peer_dependencies: Option<HashMap<String, String>>,
    license: Option<String>,
    engines: Option<HashMap<String, String>>,
    bin: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SimpleDependency {
    pub name: String,
    pub version: String,
    pub is_dev: bool,
    pub is_optional: bool,
}

/// Parses a package-lock.json file.
/// Support v1, v2 and v3 lock files
#[instrument(skip(content))]
pub fn parse(
    content: impl Into<String> + std::fmt::Debug,
) -> Result<PackageLockJson, PackageLockJsonError> {
    let json: PackageLockJson = serde_json::from_str(&content.into())?;
    Ok(json)
}

/// Returns a list of dependencies from a package-lock.json file.
/// The dependencies returned by this function only show a few fields.
/// If you need more information, use the parse function.
#[instrument(skip(content))]
pub fn parse_dependencies(
    content: impl Into<String> + std::fmt::Debug,
) -> Result<Vec<SimpleDependency>, PackageLockJsonError> {
    let json = parse(content)?;
    let mut entries = Vec::new();
    if let Some(dependencies) = json.dependencies {
        for (name, dependency) in dependencies {
            entries.push(SimpleDependency {
                name,
                version: dependency.version,
                is_dev: dependency.is_dev,
                is_optional: dependency.is_optional,
            });
        }
    } else if let Some(packages) = json.packages {
        for (name, dependency) in packages {
            entries.push(SimpleDependency {
                name,
                version: dependency.version,
                is_dev: dependency.is_dev,
                is_optional: dependency.is_optional,
            });
        }
    }
    Ok(entries)
}

fn deserialize_packages<'de, D>(
    deserializer: D,
) -> Result<Option<HashMap<String, V2Dependency>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<HashMap<String, serde_json::Value>> =
        serde::Deserialize::deserialize(deserializer)?;
    if let Some(package) = value {
        let mut packages = HashMap::new();
        for (key, mut value) in package {
            if key == "" {
                // skipping package information as it doesn't follow the schema.
                tracing::info!("Skipping package information in packages.");
                continue;
            }
            // check for engine bad formats.
            // some people use an array instead of an object.
            if let Some(engines) = value.get("engines").and_then(|e| e.as_array()) {
                tracing::warn!(
                    "Found engines as an array instead of an object. Fixing it. ({})",
                    key
                );
                if engines.len() > 0 {
                    let mut new_engines = HashMap::new();
                    for engine in engines {
                        let engine = engine.as_str().unwrap();
                        let (name, version) =
                            engine.split_once(" ").unwrap_or(("not_found", "not_found"));
                        new_engines.insert(name, version);
                    }
                    value["engines"] = serde_json::value::to_value(new_engines).unwrap();
                } else {
                    value["engines"] = serde_json::Value::Null;
                }
            }

            let vclone = value.clone();

            let package = serde_json::from_value::<V2Dependency>(value);
            match package {
                Ok(package) => {
                    let key = key.replace("node_modules/", "");
                    packages.insert(key, package);
                }
                Err(e) => {
                    // swallowing the error as we don't want to break the whole process
                    // let's just log the error:
                    tracing::error!(
                        "Could not parse this dependency: {:?}, ERROR: {}",
                        vclone,
                        e
                    );
                    continue;
                }
            };
        }
        Ok(Some(packages))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected_v1() -> V1Dependency {
        V1Dependency{
            version : "7.18.6".to_string(),
            resolved: "https://registry.npmjs.org/@babel/highlight/-/highlight-7.18.6.tgz".to_string(),
            integrity: "sha512-u7stbOuYjaPezCuLj29hNW1v64M2Md2qupEKP1fHc7WdOA3DgLh37suiSrZYY7haUB7iBeQZ9P1uiRF359do3g==".to_string(),
            bundled: false,
            is_dev: true,
            is_optional: false,
            requires: Some(HashMap::from([("js-tokens".to_string(), "^4.0.0".to_string()), ("chalk".to_string(), "^2.0.0".to_string()),("@babel/helper-validator-identifier".to_string(), "^7.18.6".to_string())])),
            dependencies: Some(HashMap::from([("js-tokens".to_string(), V1Dependency {
                version: "4.0.0".to_string(),
                resolved: "https://registry.npmjs.org/js-tokens/-/js-tokens-4.0.0.tgz".to_string(),
                integrity: "sha512-RdJUflcE3cUzKiMqQgsCu06FPu9UdIJO0beYbPhHN4k6apgJtifcoCtT9bcxOpYBtpD2kCM6Sbzg4CausW/PKQ==".to_string(),
                is_dev: true,
                bundled: false,
                ..V1Dependency::default()
                })]
            ))
        }
    }

    fn expected_v2() -> V2Dependency {
        V2Dependency{
            version : "7.18.6".to_string(),
            resolved: "https://registry.npmjs.org/@babel/highlight/-/highlight-7.18.6.tgz".to_string(),
            integrity: "sha512-u7stbOuYjaPezCuLj29hNW1v64M2Md2qupEKP1fHc7WdOA3DgLh37suiSrZYY7haUB7iBeQZ9P1uiRF359do3g==".to_string(),
            bundled: false,
            is_dev: true,
            is_optional: false,
            dependencies: Some(HashMap::from([("js-tokens".to_string(), "^4.0.0".to_string()), ("chalk".to_string(), "^2.0.0".to_string()),("@babel/helper-validator-identifier".to_string(), "^7.18.6".to_string())])),
            engines: Some(HashMap::from([("node".to_string(), ">=6.9.0".to_string())])),
            ..V2Dependency::default()
        }
    }

    #[test]

    fn parse_v1_from_file_works() {
        let content = std::fs::read_to_string("tests/v1/package-lock.json").unwrap();
        let lock_file = parse(content).unwrap();
        assert_eq!(lock_file.name, "cxtl");
        assert_eq!(lock_file.version, "1.0.0");
        assert_eq!(lock_file.lockfile_version, 1);

        assert!(lock_file.dependencies.is_some());
        assert!(lock_file.packages.is_none());

        let dependencies = lock_file.dependencies.unwrap();
        let babel_highlight = dependencies.get("@babel/highlight").unwrap();

        let expected = expected_v1();

        assert_eq!(babel_highlight, &expected);
    }

    #[test]

    fn parse_v2_from_file_works() {
        let content = std::fs::read_to_string("tests/v2/package-lock.json").unwrap();
        let lock_file = parse(content).unwrap();
        assert_eq!(lock_file.name, "cxtl");
        assert_eq!(lock_file.version, "1.0.0");
        assert_eq!(lock_file.lockfile_version, 2);

        assert!(lock_file.dependencies.is_some());
        assert!(lock_file.packages.is_some());

        // v1
        let dependencies = lock_file.dependencies.unwrap();
        let babel_highlight = dependencies.get("@babel/highlight").unwrap();

        let expected = expected_v1();
        assert_eq!(babel_highlight, &expected);

        // v2
        let packages = lock_file.packages.unwrap();
        let babel_highlight = packages.get("@babel/highlight").unwrap();

        let expected = expected_v2();

        assert_eq!(babel_highlight, &expected);
    }

    #[test]
    fn parse_v3_from_file_works() {
        let content = std::fs::read_to_string("tests/v3/package-lock.json").unwrap();
        let lock_file = parse(content).unwrap();
        assert_eq!(lock_file.name, "cxtl");
        assert_eq!(lock_file.version, "1.0.0");
        assert_eq!(lock_file.lockfile_version, 3);

        assert!(lock_file.dependencies.is_none());
        assert!(lock_file.packages.is_some());

        let packages = lock_file.packages.unwrap();
        let babel_highlight = packages.get("@babel/highlight").unwrap();

        let expected = expected_v2();

        assert_eq!(babel_highlight, &expected);
    }

    #[test]
    fn deserialize_packages_works() {
        let content = r#"{
            "node_modules/extsprintf": {
                "version": "1.3.0",
                "resolved": "https://registry.npmjs.org/extsprintf/-/extsprintf-1.3.0.tgz",
                "integrity": "sha512-11Ndz7Nv+mvAC1j0ktTa7fAb0vLyGGX+rMHNBYQviQDGU0Hw7lhctJANqbPhu9nV9/izT/IntTgZ7Im/9LJs9g==",
                "dev": true,
                "engines": [
                    "node >=0.6.0"
                ]
            }
        }"#;

        let mut deserializer = serde_json::Deserializer::from_str(content);
        let packages = deserialize_packages(&mut deserializer).unwrap().unwrap();
        // removes node_modules/ from the key
        let package = packages.get("extsprintf").unwrap();
        assert_eq!(package.version, "1.3.0");
        assert!(package.is_dev);
        assert_eq!(
            package.engines,
            Some(HashMap::from([("node".to_string(), ">=0.6.0".to_string())]))
        );
    }

    #[test]
    fn parse_entries_v1_works() {
        let content = std::fs::read_to_string("tests/v1/package-lock.json").unwrap();
        let mut dependencies = parse_dependencies(content).unwrap();
        dependencies.sort();

        let first = dependencies.first().unwrap();
        assert_eq!(first.name, "@babel/code-frame");
        assert_eq!(first.version, "7.18.6");
        assert!(first.is_dev);
        assert!(!first.is_optional);
    }

    #[test]
    fn parse_entries_v2_works() {
        let content = std::fs::read_to_string("tests/v3/package-lock.json").unwrap();
        let mut dependencies = parse_dependencies(content).unwrap();
        dependencies.sort();

        let first = dependencies.first().unwrap();
        assert_eq!(first.name, "@babel/code-frame");
        assert_eq!(first.version, "7.18.6");
        assert!(first.is_dev);
        assert!(!first.is_optional);
    }
}
