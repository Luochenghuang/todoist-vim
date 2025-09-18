extern crate chrono;
use std::collections::HashMap;

use chrono::{Local, NaiveDate};
use ratatui::widgets::ListState;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Default)]
pub struct Tasks {
    pub tasks: Vec<Task>,
    pub filter: Filter,
    pub state: ListState,
    pub tasks_with_children: HashMap<String, u16>,
    pub display_tasks: Vec<usize>,
}

#[derive(Debug)]
pub enum SortCriterion {
    Priority,
    Date,
}

impl Tasks {
    pub fn new(items: Vec<Task>) -> Tasks {
        Tasks {
            tasks: items,
            filter: Filter::All,
            state: ListState::default(),
            tasks_with_children: HashMap::new(),
            display_tasks: Vec::new(),
        }
    }

    pub fn find_tasks_with_children(&mut self) {
        for task in &self.tasks {
            if let Some(parent_id) = &task.parent_id {
                *self
                    .tasks_with_children
                    .entry(parent_id.clone())
                    .or_insert(0) += 1;
            }
        }
    }

    pub fn sort_tasks(&mut self, criterion: SortCriterion) {
        // Instead of sorting the entire display_tasks vector, we need to rebuild it
        // with proper hierarchical sorting that preserves parent-child relationships
        self.sort_tasks_hierarchically(criterion);
    }

    fn sort_tasks_hierarchically(&mut self, criterion: SortCriterion) {
        // Store the currently selected task ID to preserve selection after sorting
        let selected_task_id = if let Some(selected_index) = self.state.selected() {
            if selected_index < self.display_tasks.len() {
                Some(self.tasks[self.display_tasks[selected_index]].id.clone())
            } else {
                None
            }
        } else {
            None
        };
        
        // Get root tasks (tasks without parent_id) that match the filter
        let mut root_tasks = Vec::new();
        for (index, task) in self.tasks.iter().enumerate() {
            if task.parent_id.is_none() {
                let matches_filter = match &self.filter {
                    Filter::All => true,
                    Filter::Today => {
                        let today = Local::now().date_naive();
                        if let Some(due) = &task.due {
                            due.date == today
                        } else {
                            false
                        }
                    }
                    Filter::ProjectId(project_id) => {
                        task.project_id == *project_id
                    }
                    Filter::Overdue => {
                        let today = Local::now().date_naive();
                        if let Some(due) = &task.due {
                            due.date < today
                        } else {
                            false
                        }
                    }
                };
                
                if matches_filter {
                    root_tasks.push(index);
                }
            }
        }
        
        // Sort root tasks by the specified criterion
        match criterion {
            SortCriterion::Priority => {
                root_tasks.sort_by(|a, b| {
                    let task_a = &self.tasks[*a];
                    let task_b = &self.tasks[*b];
                    task_a.priority.cmp(&task_b.priority)
                });
            }
            SortCriterion::Date => {
                root_tasks.sort_by(|a, b| {
                    let task_a = &self.tasks[*a];
                    let task_b = &self.tasks[*b];
                    match (&task_a.due, &task_b.due) {
                        (Some(due_a), Some(due_b)) => due_a.date.cmp(&due_b.date),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    }
                });
            }
        }
        
        // Rebuild display_tasks with sorted root tasks and their subtasks
        self.display_tasks = Vec::new();
        for root_index in root_tasks {
            self.display_tasks.push(root_index);
            self.add_subtasks_recursively(root_index);
        }
        
        // Restore selection to the same task if it still exists in the display list
        if let Some(task_id) = selected_task_id {
            if let Some(new_index) = self.display_tasks.iter().position(|&idx| self.tasks[idx].id == task_id) {
                self.state.select(Some(new_index));
            } else {
                // If the selected task is no longer visible, select the first task
                if !self.display_tasks.is_empty() {
                    self.state.select(Some(0));
                } else {
                    self.state.select(None);
                }
            }
        }
    }

    pub fn filter_task_list(&mut self, auto_select: bool) {
        self.state = ListState::default();
        self.display_tasks = Vec::new();
        
        // Get root tasks (tasks without parent_id) that match the filter
        let mut root_tasks = Vec::new();
        for (index, task) in self.tasks.iter().enumerate() {
            if task.parent_id.is_none() {
                let matches_filter = match &self.filter {
                    Filter::All => true,
                    Filter::Today => {
                        let today = Local::now().date_naive();
                        if let Some(due) = &task.due {
                            due.date == today
                        } else {
                            false
                        }
                    }
                    Filter::ProjectId(project_id) => {
                        task.project_id == *project_id
                    }
                    Filter::Overdue => {
                        let today = Local::now().date_naive();
                        if let Some(due) = &task.due {
                            due.date < today
                        } else {
                            false
                        }
                    }
                };
                
                if matches_filter {
                    root_tasks.push(index);
                }
            }
        }
        
        // Sort root tasks by priority by default
        root_tasks.sort_by(|a, b| {
            let task_a = &self.tasks[*a];
            let task_b = &self.tasks[*b];
            task_a.priority.cmp(&task_b.priority)
        });
        
        // Build tree structure by adding subtasks after their parents
        for root_index in root_tasks {
            self.display_tasks.push(root_index);
            self.add_subtasks_recursively(root_index);
        }
        
        // Automatically select the first item if there are any tasks and auto_select is true
        if auto_select && !self.display_tasks.is_empty() {
            self.state.select(Some(0));
        }
    }
    
    fn add_subtasks_recursively(&mut self, parent_index: usize) {
        let parent_id = &self.tasks[parent_index].id;
        
        // Find all direct children of this parent
        let mut children: Vec<usize> = self.tasks.iter()
            .enumerate()
            .filter(|(_, task)| task.parent_id.as_ref() == Some(parent_id))
            .map(|(index, _)| index)
            .collect();
        
        // Sort children by order field
        children.sort_by_key(|&index| self.tasks[index].order);
        
        // Add children and their subtasks recursively
        for child_index in children {
            self.display_tasks.push(child_index);
            self.add_subtasks_recursively(child_index);
        }
    }

    pub fn next(&mut self) {
        if self.display_tasks.len() == 0 {
            self.state.select(None);
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.display_tasks.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        if self.display_tasks.len() == 0 {
            self.state.select(None);
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.display_tasks.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn unselect(&mut self) {
        let offset = self.state.offset();
        self.state.select(None);
        *self.state.offset_mut() = offset;
    }
}

#[derive(Debug, Default)]
pub enum Filter {
    #[default]
    All,
    Today,
    Overdue,
    ProjectId(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub section_id: Option<String>,
    pub content: String,
    pub description: String,
    pub is_completed: bool,
    pub labels: Vec<String>,
    pub parent_id: Option<String>,
    pub order: i32,
    pub priority: u8,
    pub due: Option<Due>,
    pub url: String,
    pub comment_count: u16,
    #[serde(skip_serializing)]
    pub created_at: String,
    #[serde(skip_serializing)]
    pub creator_id: String,
    pub assignee_id: Option<String>,
    pub assigner_id: Option<String>,
    pub duration: Option<Duration>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Due {
    pub string: String,
    #[serde(with = "date_format")]
    pub date: NaiveDate,
    pub is_recurring: bool,
    pub datetime: Option<String>,
    pub timezone: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Duration {
    amount: u32,
    unit: String,
}

pub mod date_format {
    use super::*;

    pub fn serialize<S>(date: &NaiveDate, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{}", date.format("%Y-%m-%d"));
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<NaiveDate, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        NaiveDate::parse_from_str(&s, "%Y-%m-%d").map_err(serde::de::Error::custom)
    }
}
