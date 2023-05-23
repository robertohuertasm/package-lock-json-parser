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
    pub name: String,
    pub version: Option<String>,
    #[serde(rename = "lockfileVersion")]
    pub lockfile_version: u32,
    pub dependencies: Option<HashMap<String, V1Dependency>>,
    #[serde(deserialize_with = "deserialize_packages", default)]
    pub packages: Option<HashMap<String, V2Dependency>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Default)]
pub struct V1Dependency {
    pub version: String,
    pub resolved: Option<String>,
    pub integrity: Option<String>,
    #[serde(default)]
    pub bundled: bool,
    #[serde(rename = "dev", default)]
    pub is_dev: bool,
    #[serde(rename = "optional", default)]
    pub is_optional: bool,
    pub requires: Option<HashMap<String, String>>,
    pub dependencies: Option<HashMap<String, V1Dependency>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Default)]
pub struct V2Dependency {
    pub version: String,
    pub name: Option<String>,
    pub resolved: Option<String>,
    pub integrity: Option<String>,
    #[serde(default)]
    pub bundled: bool,
    #[serde(rename = "dev", default)]
    pub is_dev: bool,
    #[serde(rename = "optional", default)]
    pub is_optional: bool,
    #[serde(rename = "devOptional", default)]
    pub is_dev_optional: bool,
    #[serde(rename = "inBundle", default)]
    pub is_in_bundle: bool,
    #[serde(rename = "hasInstallScript", default)]
    pub has_install_script: bool,
    #[serde(rename = "hasShrinkwrap", default)]
    pub has_shrink_wrap: bool,
    pub dependencies: Option<HashMap<String, String>>,
    #[serde(rename = "devDependencies")]
    pub dev_dependencies: Option<HashMap<String, String>>,
    #[serde(rename = "optionalDependencies")]
    pub optional_dependencies: Option<HashMap<String, String>>,
    #[serde(rename = "peerDependencies")]
    pub peer_dependencies: Option<HashMap<String, String>>,
    pub license: Option<String>,
    pub engines: Option<HashMap<String, String>>,
    pub bin: Option<HashMap<String, String>>,
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
    let mut json: PackageLockJson = serde_json::from_str(&content.into())?;
    // fix version for v2 and workspaces
    // version = "file:mainlib" -> version = "0.0.0"
    if let (Some(dependencies), Some(packages)) =
        (json.dependencies.as_mut(), json.packages.as_ref())
    {
        for (name, dependency) in dependencies {
            if dependency.version.starts_with("file:") {
                if let Some(pkg) = packages.get(name) {
                    dependency.version = pkg.version.clone();
                }
            }
        }
    }
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
                    let pattern = "node_modules/";
                    if key.starts_with(pattern) {
                        if !key.contains("/node_modules/") {
                            // we are ignoring nested dependencies
                            let key = key.replace(pattern, "");
                            packages.insert(key, package);
                        }
                    } else {
                        // possibly workspaces, let's look for name
                        if let Some(ref name) = package.name {
                            // if name, we will use it as the key.
                            // these packages will also have a version with a `node_modules/` prefix.
                            // as that version won't have a version, it will fail to parse and will be silently ignored.
                            packages.insert(name.clone(), package);
                        } else {
                            packages.insert(key, package);
                        }
                    }
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
            resolved: Some("https://registry.npmjs.org/@babel/highlight/-/highlight-7.18.6.tgz".to_string()),
            integrity: Some("sha512-u7stbOuYjaPezCuLj29hNW1v64M2Md2qupEKP1fHc7WdOA3DgLh37suiSrZYY7haUB7iBeQZ9P1uiRF359do3g==".to_string()),
            bundled: false,
            is_dev: true,
            is_optional: false,
            requires: Some(HashMap::from([("js-tokens".to_string(), "^4.0.0".to_string()), ("chalk".to_string(), "^2.0.0".to_string()),("@babel/helper-validator-identifier".to_string(), "^7.18.6".to_string())])),
            dependencies: Some(HashMap::from([("js-tokens".to_string(), V1Dependency {
                version: "4.0.0".to_string(),
                resolved: Some("https://registry.npmjs.org/js-tokens/-/js-tokens-4.0.0.tgz".to_string()),
                integrity: Some("sha512-RdJUflcE3cUzKiMqQgsCu06FPu9UdIJO0beYbPhHN4k6apgJtifcoCtT9bcxOpYBtpD2kCM6Sbzg4CausW/PKQ==".to_string()),
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
            resolved: Some("https://registry.npmjs.org/@babel/highlight/-/highlight-7.18.6.tgz".to_string()),
            integrity: Some("sha512-u7stbOuYjaPezCuLj29hNW1v64M2Md2qupEKP1fHc7WdOA3DgLh37suiSrZYY7haUB7iBeQZ9P1uiRF359do3g==".to_string()),
            bundled: false,
            is_dev: true,
            is_optional: false,
            dependencies: Some(HashMap::from([("js-tokens".to_string(), "^4.0.0".to_string()), ("chalk".to_string(), "^2.0.0".to_string()),("@babel/helper-validator-identifier".to_string(), "^7.18.6".to_string())])),
            engines: Some(HashMap::from([("node".to_string(), ">=6.9.0".to_string())])),
            ..V2Dependency::default()
        }
    }

    #[test]
    fn works_without_version() {
        let content = std::fs::read_to_string("tests/cool-project/package-lock.json").unwrap();
        let lock_file = parse(content).unwrap();
        assert_eq!(lock_file.name, "cool-project");
        assert!(lock_file.version.is_none());
    }

    #[test]
    fn cool_project_works() {
        let content = std::fs::read_to_string("tests/cool-project/package-lock.json").unwrap();
        let lock_file = parse(content).unwrap();
        assert_eq!(lock_file.name, "cool-project");
        assert!(lock_file.version.is_none());
        assert_eq!(lock_file.lockfile_version, 2);

        assert!(lock_file.dependencies.is_some());
        assert!(lock_file.packages.is_some());

        let packages = lock_file.packages.unwrap();
        let cool = packages.get("cool-project").unwrap();
        assert_eq!(cool.name, Some("cool-project".to_string()));
        assert_eq!(cool.version, "23.1.21".to_string());

        let dependencies = lock_file.dependencies.unwrap();
        let cool = dependencies.get("cool-project").unwrap();
        assert_eq!(cool.version, "23.1.21".to_string());
    }

    #[test]
    fn parse_moon_workspace_dependencies_works() {
        let content = std::fs::read_to_string("tests/workspace/moon/package-lock.json").unwrap();
        let lock_file = parse(content).unwrap();
        assert_eq!(lock_file.name, "moon-examples");
        assert_eq!(lock_file.version, Some("1.2.3".to_string()));
        assert_eq!(lock_file.lockfile_version, 3);

        assert!(lock_file.dependencies.is_none());
        assert!(lock_file.packages.is_some());

        let packages = lock_file.packages.unwrap();

        let yaml = packages.get("yaml").unwrap();
        let expected_yaml = V2Dependency {
            version: "2.2.2".to_string(),
            resolved: Some("https://registry.npmjs.org/yaml/-/yaml-2.2.2.tgz".to_string()),
            integrity: Some("sha512-CBKFWExMn46Foo4cldiChEzn7S7SRV+wqiluAb6xmueD/fGyRHIhX8m14vVGgeFWjN540nKCNVj6P21eQjgTuA==".to_string()),
            is_dev: true,
            engines: Some(HashMap::from([("node".to_string(), ">= 14".to_string())])),
            ..V2Dependency::default()
        };
        assert_eq!(yaml, &expected_yaml);

        // workspace?
        let libnpmdiff = packages.get("workspaces/libnpmdiff").unwrap();
        assert_eq!(libnpmdiff.version, "5.0.17".to_string());
        assert_eq!(libnpmdiff.license, Some("ISC".to_string()));
        assert!(libnpmdiff.dependencies.is_some());
        let dependencies = libnpmdiff.dependencies.as_ref().unwrap();
        assert!(dependencies.contains_key("pacote"));
        assert!(dependencies.contains_key("tar"));
    }

    #[test]
    fn parse_v2_workspace_dependencies_works() {
        let content = std::fs::read_to_string("tests/workspace/v2/package-lock.json").unwrap();
        let lock_file = parse(content).unwrap();
        assert_eq!(lock_file.name, "test-node-npm");
        assert_eq!(lock_file.version, Some("1.0.0".to_string()));
        assert_eq!(lock_file.lockfile_version, 2);

        assert!(lock_file.dependencies.is_some());
        assert!(lock_file.packages.is_some());

        let packages = lock_file.packages.unwrap();

        let test_node_npm_base = packages.get("test-node-npm-base").unwrap();
        let expected_base = V2Dependency {
            version: "1.0.0".to_string(),
            name: Some("test-node-npm-base".to_string()),
            dependencies: Some(HashMap::from([("react".to_string(), "17.0.0".to_string())])),
            ..V2Dependency::default()
        };
        assert_eq!(test_node_npm_base, &expected_base);

        // ensure base is not present
        let base = packages.get("base");
        assert!(base.is_none());

        // let's check now v1 version
        let dependencies = lock_file.dependencies.unwrap();
        let test_node_npm_v1 = dependencies.get("test-node-npm-base").unwrap();
        assert_eq!(
            test_node_npm_v1,
            &V1Dependency {
                version: "file:base".to_string(),
                requires: Some(HashMap::from([("react".to_string(), "17.0.0".to_string())])),
                ..V1Dependency::default()
            }
        );
    }

    #[test]
    fn parse_v3_workspace_dependencies_works() {
        let content = std::fs::read_to_string("tests/workspace/v3/package-lock.json").unwrap();
        let lock_file = parse(content).unwrap();
        assert_eq!(lock_file.name, "kk");
        assert_eq!(lock_file.version, Some("1.0.0".to_string()));
        assert_eq!(lock_file.lockfile_version, 3);

        assert!(lock_file.dependencies.is_none());
        assert!(lock_file.packages.is_some());

        let packages = lock_file.packages.unwrap();

        // check liba
        let liba = packages.get("liba").unwrap();
        let expected_liba = V2Dependency {
            version: "1.0.0".to_string(),
            resolved: None,
            integrity: None,
            bundled: false,
            is_dev: false,
            is_optional: false,
            dependencies: Some(HashMap::from([("libb2".to_string(), "*".to_string())])),
            license: Some("ISC".to_string()),
            engines: None,
            ..V2Dependency::default()
        };
        assert_eq!(liba, &expected_liba);

        // ensure libb is not present
        let libb = packages.get("libb");
        assert!(libb.is_none());

        // ensure libb2 is present
        let libb2 = packages.get("libb2").unwrap();
        let expected_libb2 = V2Dependency {
            name: Some("libb2".to_string()),
            version: "1.0.0".to_string(),
            resolved: None,
            integrity: None,
            bundled: false,
            is_dev: false,
            is_optional: false,
            dependencies: None,
            license: Some("ISC".to_string()),
            engines: None,
            ..V2Dependency::default()
        };
        assert_eq!(libb2, &expected_libb2);
    }

    #[test]

    fn parse_v1_from_file_works() {
        let content = std::fs::read_to_string("tests/v1/package-lock.json").unwrap();
        let lock_file = parse(content).unwrap();
        assert_eq!(lock_file.name, "cxtl");
        assert_eq!(lock_file.version, Some("1.0.0".to_string()));
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
        assert_eq!(lock_file.version, Some("1.0.0".to_string()));
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
        assert_eq!(lock_file.version, Some("1.0.0".to_string()));
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
