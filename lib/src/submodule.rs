// Copyright 2026 The Jujutsu Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Git submodule support.
//!
//! This module provides functionality for working with Git submodules in jj,
//! aiming for a Sapling-like experience where submodules are handled
//! automatically during clone and checkout operations.

use std::path::Path;
use std::path::PathBuf;

use bstr::BStr;
use thiserror::Error;

/// Information about a single submodule.
#[derive(Debug, Clone)]
pub struct SubmoduleInfo {
    /// The submodule's name (from .gitmodules [submodule "name"] section).
    pub name: String,
    /// The path where the submodule should be checked out, relative to repo root.
    pub path: PathBuf,
    /// The URL to clone the submodule from.
    pub url: String,
}

/// Errors that can occur when working with submodules.
#[derive(Debug, Error)]
pub enum SubmoduleError {
    #[error("Failed to open .gitmodules: {0}")]
    OpenModulesFile(String),
    #[error("Failed to parse submodule configuration: {0}")]
    ParseConfig(String),
    #[error("Submodule '{name}' has no URL configured")]
    MissingUrl { name: String },
    #[error("Submodule '{name}' has no path configured")]
    MissingPath { name: String },
    #[error("Failed to clone submodule '{name}': {source}")]
    CloneFailed {
        name: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Git backend required for submodule operations")]
    NotGitBackend,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// List all submodules defined in the repository.
///
/// This reads the .gitmodules file from the working copy (preferring disk,
/// falling back to the tree) and returns information about each submodule.
#[cfg(feature = "git")]
pub fn list_submodules(git_repo: &gix::Repository) -> Result<Vec<SubmoduleInfo>, SubmoduleError> {
    let submodules = match git_repo.submodules() {
        Ok(Some(iter)) => iter,
        Ok(None) => return Ok(vec![]),
        Err(e) => {
            return Err(SubmoduleError::OpenModulesFile(e.to_string()));
        }
    };

    let mut result = Vec::new();
    for submodule in submodules {
        let name = submodule.name().to_string();

        let path = match submodule.path() {
            Ok(p) => PathBuf::from(p.to_string()),
            Err(e) => {
                return Err(SubmoduleError::ParseConfig(format!(
                    "Failed to get path for submodule '{}': {}",
                    name, e
                )));
            }
        };

        let url = match submodule.url() {
            Ok(u) => u.to_bstring().to_string(),
            Err(e) => {
                return Err(SubmoduleError::ParseConfig(format!(
                    "Failed to get URL for submodule '{}': {}",
                    name, e
                )));
            }
        };

        result.push(SubmoduleInfo { name, path, url });
    }

    Ok(result)
}

/// Returns the directory where submodule repositories should be stored.
///
/// For a jj repo at `/path/to/repo`, submodules are stored in
/// `/path/to/repo/.jj/repo/submodules/<sanitized-name>/`.
pub fn submodule_store_path(repo_path: &Path, submodule_name: &str) -> PathBuf {
    // Sanitize the submodule name to be safe for filesystem paths
    let sanitized = submodule_name.replace(['/', '\\'], "__");
    repo_path.join("submodules").join(sanitized)
}

/// Check if a submodule repository exists at the expected location.
pub fn submodule_repo_exists(repo_path: &Path, submodule_name: &str) -> bool {
    let submodule_path = submodule_store_path(repo_path, submodule_name);
    submodule_path.join(".jj").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_submodule_store_path_simple() {
        let repo_path = Path::new("/home/user/repo/.jj/repo");
        let path = submodule_store_path(repo_path, "mysubmodule");
        assert_eq!(
            path,
            PathBuf::from("/home/user/repo/.jj/repo/submodules/mysubmodule")
        );
    }

    #[test]
    fn test_submodule_store_path_nested() {
        let repo_path = Path::new("/home/user/repo/.jj/repo");
        let path = submodule_store_path(repo_path, "path/to/submodule");
        assert_eq!(
            path,
            PathBuf::from("/home/user/repo/.jj/repo/submodules/path__to__submodule")
        );
    }

    #[test]
    fn test_submodule_store_path_backslash() {
        let repo_path = Path::new("/home/user/repo/.jj/repo");
        let path = submodule_store_path(repo_path, r"path\to\submodule");
        assert_eq!(
            path,
            PathBuf::from("/home/user/repo/.jj/repo/submodules/path__to__submodule")
        );
    }
}
