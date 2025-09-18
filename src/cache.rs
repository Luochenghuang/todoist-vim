use color_eyre::Result;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::BufReader;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::projects::Project;
use crate::sections::Section;
use crate::tasks::Task;

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheData {
    pub projects: Vec<Project>,
    pub tasks: Vec<Task>,
    pub sections: Vec<Section>,
    pub timestamp: u64,
    pub cursor_position: Option<usize>,
    pub selected_project_id: Option<String>,
}

impl CacheData {
    pub fn new(projects: Vec<Project>, tasks: Vec<Task>, sections: Vec<Section>) -> Self {
        Self {
            projects,
            tasks,
            sections,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            cursor_position: None,
            selected_project_id: None,
        }
    }

    pub fn with_cursor_info(
        projects: Vec<Project>,
        tasks: Vec<Task>,
        sections: Vec<Section>,
        cursor_position: Option<usize>,
        selected_project_id: Option<String>,
    ) -> Self {
        Self {
            projects,
            tasks,
            sections,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            cursor_position,
            selected_project_id,
        }
    }
}

#[derive(Debug)]
pub struct CacheManager {
    cache_file_path: std::path::PathBuf,
}

impl CacheManager {
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| color_eyre::eyre::eyre!("No cache directory found"))?
            .join("todoist-vim");
        
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir)?;
        }

        let cache_file_path = cache_dir.join("cache.json");

        Ok(Self { cache_file_path })
    }

    pub fn save_cache(&self, cache_data: &CacheData) -> Result<()> {
        let json = serde_json::to_string_pretty(cache_data)?;
        fs::write(&self.cache_file_path, json)?;
        Ok(())
    }

    pub fn load_cache(&self) -> Result<Option<CacheData>> {
        if !self.cache_file_path.exists() {
            return Ok(None);
        }

        let file = File::open(&self.cache_file_path)?;
        let reader = BufReader::new(file);
        let cache_data: CacheData = serde_json::from_reader(reader)?;
        Ok(Some(cache_data))
    }

    pub fn is_cache_valid(&self, max_age_seconds: u64) -> Result<bool> {
        match self.load_cache()? {
            Some(cache_data) => {
                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                Ok(current_time - cache_data.timestamp < max_age_seconds)
            }
            None => Ok(false),
        }
    }

    pub fn clear_cache(&self) -> Result<()> {
        if self.cache_file_path.exists() {
            fs::remove_file(&self.cache_file_path)?;
        }
        Ok(())
    }
}