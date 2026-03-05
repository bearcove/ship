use std::env;
use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use fs_err::tokio as fs;
use ship_types::{ProjectInfo, ProjectName};

#[derive(Debug)]
pub struct ProjectRegistryError {
    message: String,
}

impl ProjectRegistryError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ProjectRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for ProjectRegistryError {}

impl From<io::Error> for ProjectRegistryError {
    fn from(value: io::Error) -> Self {
        Self::new(value.to_string())
    }
}

// r[server.config-dir]
#[derive(Debug)]
pub struct ProjectRegistry {
    config_dir: PathBuf,
    projects_file: PathBuf,
    projects: Vec<ProjectInfo>,
}

impl ProjectRegistry {
    // r[server.config-dir]
    pub async fn load_default() -> Result<Self, ProjectRegistryError> {
        let config_dir = default_config_dir()?;
        Self::load_in(config_dir).await
    }

    pub async fn load_in(config_dir: PathBuf) -> Result<Self, ProjectRegistryError> {
        fs::create_dir_all(&config_dir).await?;
        let projects_file = config_dir.join("projects.json");
        let projects = match fs::read(&projects_file).await {
            Ok(bytes) => facet_json::from_slice::<Vec<ProjectInfo>>(&bytes).map_err(|error| {
                ProjectRegistryError::new(format!("invalid projects.json: {error}"))
            })?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error.into()),
        };

        Ok(Self {
            config_dir,
            projects_file,
            projects,
        })
    }

    // r[project.registration]
    // r[project.identity]
    pub async fn add(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<ProjectInfo, ProjectRegistryError> {
        let absolute = absolutize(path.as_ref())?;
        let base_name = derive_project_name(&absolute);
        let unique_name = self.unique_project_name(&base_name);
        let project = ProjectInfo {
            name: ProjectName(unique_name),
            path: absolute.to_string_lossy().into_owned(),
            valid: true,
            invalid_reason: None,
        };
        self.projects.push(project.clone());
        self.save().await?;
        Ok(project)
    }

    pub async fn remove(&mut self, name: &str) -> Result<bool, ProjectRegistryError> {
        let before = self.projects.len();
        self.projects.retain(|project| project.name.0 != name);
        let removed = self.projects.len() != before;
        if removed {
            self.save().await?;
        }
        Ok(removed)
    }

    pub fn list(&self) -> Vec<ProjectInfo> {
        self.projects.clone()
    }

    pub fn get(&self, name: &str) -> Option<ProjectInfo> {
        self.projects
            .iter()
            .find(|project| project.name.0 == name)
            .cloned()
    }

    // r[project.validation]
    pub async fn validate_all(&mut self) -> Result<(), ProjectRegistryError> {
        for project in &mut self.projects {
            let path = PathBuf::from(&project.path);
            if fs::metadata(&path).await.is_err() {
                project.valid = false;
                project.invalid_reason = Some("path does not exist".to_owned());
                continue;
            }
            let git_dir = path.join(".git");
            let git_metadata = fs::metadata(&git_dir).await;
            if !matches!(git_metadata, Ok(metadata) if metadata.is_dir()) {
                project.valid = false;
                project.invalid_reason = Some("not a git repository (.git missing)".to_owned());
                continue;
            }
            project.valid = true;
            project.invalid_reason = None;
        }
        self.save().await?;
        Ok(())
    }

    // r[project.persistence-dir]
    pub fn project_ship_dir(&self, name: &str) -> Option<PathBuf> {
        self.get(name)
            .map(|project| PathBuf::from(project.path).join(".ship"))
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    fn unique_project_name(&self, base: &str) -> String {
        if !self.projects.iter().any(|project| project.name.0 == base) {
            return base.to_owned();
        }

        let mut suffix = 2usize;
        loop {
            let candidate = format!("{base}-{suffix}");
            if !self
                .projects
                .iter()
                .any(|project| project.name.0 == candidate)
            {
                return candidate;
            }
            suffix += 1;
        }
    }

    async fn save(&self) -> Result<(), ProjectRegistryError> {
        let bytes = facet_json::to_vec_pretty(&self.projects).map_err(|error| {
            ProjectRegistryError::new(format!("failed to serialize projects: {error}"))
        })?;
        fs::write(&self.projects_file, bytes).await?;
        Ok(())
    }
}

// r[server.config-dir]
fn default_config_dir() -> Result<PathBuf, ProjectRegistryError> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| ProjectRegistryError::new("HOME is not set"))?;
    Ok(home.join(".config").join("ship"))
}

fn derive_project_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("project")
        .to_owned()
}

fn absolutize(path: &Path) -> Result<PathBuf, ProjectRegistryError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}
