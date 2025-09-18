use crossterm::event::{KeyCode, KeyEvent};
use reqwest::Client;

use crate::{
    api_calls::{self, close_task, create_task, delete_task},
    new_task, task_edit, App, TaskResult, tasks::Task,
};

pub fn handle_task_editor(
    app: &mut App,
    key: KeyEvent,
    client: Client,
    tx: std::sync::mpsc::Sender<TaskResult>,
) {
    if key.code == KeyCode::Esc {
        app.show_task_editor = !app.show_task_editor;
    } else if key.code == KeyCode::Enter {
        app.show_task_editor = !app.show_task_editor;
        let index = app.task_edit.current_task_index;

        app.tasks.tasks[index].content = app.task_edit.content.lines().join("\n");
        app.tasks.tasks[index].description = app.task_edit.description.lines().join("\n");
        
        // Parse and update priority
        let priority_text = app.task_edit.priority_string.lines().join("");
        let priority_text = priority_text.trim();
        if !priority_text.is_empty() {
            if let Ok(priority_value) = priority_text.parse::<u8>() {
                if priority_value >= 1 && priority_value <= 4 {
                    app.tasks.tasks[index].priority = priority_value;
                }
            }
        }

        let task = app.tasks.tasks[index].clone();

        let task_string = serde_json::to_string(&task).unwrap();
        let mut json: serde_json::Value = serde_json::from_str(&task_string).unwrap();

        json["due_string"] = serde_json::Value::String(app.task_edit.due_string.lines().join("\n"));

        tokio::spawn(async move {
            let _ = api_calls::update_task(&client, json, task.id.to_string(), tx).await;
        });
    }
    if key.code == KeyCode::Tab {
        if app.task_edit.currently_editing == task_edit::CurrentlyEditing::Content {
            app.task_edit.currently_editing = task_edit::CurrentlyEditing::Description
        } else if app.task_edit.currently_editing == task_edit::CurrentlyEditing::Description {
            app.task_edit.currently_editing = task_edit::CurrentlyEditing::Priority
        } else if app.task_edit.currently_editing == task_edit::CurrentlyEditing::Priority {
            app.task_edit.currently_editing = task_edit::CurrentlyEditing::DueString
        } else if app.task_edit.currently_editing == task_edit::CurrentlyEditing::DueString {
            app.task_edit.currently_editing = task_edit::CurrentlyEditing::ChildTasks
        } else if app.task_edit.currently_editing == task_edit::CurrentlyEditing::ChildTasks {
            app.task_edit.currently_editing = task_edit::CurrentlyEditing::Content
        }
        app.task_edit.update_cursor_styles();
        return;
    }

    if app.task_edit.currently_editing == task_edit::CurrentlyEditing::Content {
        app.task_edit.content.input(key);
    } else if app.task_edit.currently_editing == task_edit::CurrentlyEditing::Description {
        app.task_edit.description.input(key);
    } else if app.task_edit.currently_editing == task_edit::CurrentlyEditing::Priority {
        handle_priority_input(&mut app.task_edit.priority_string, key);
    } else if app.task_edit.currently_editing == task_edit::CurrentlyEditing::DueString {
        app.task_edit.due_string.input(key);
    } else if app.task_edit.currently_editing == task_edit::CurrentlyEditing::ChildTasks {
        if key.code == KeyCode::Char('j') || key.code == KeyCode::Down {
            app.task_edit.next();
        } else if key.code == KeyCode::Char('k') || key.code == KeyCode::Up {
            app.task_edit.previous();
        } else if key.code == KeyCode::Enter {
            if let Some(selected) = app.task_edit.children_list_state.selected() {
                app.show_task_editor = true;
                let index = app.task_edit.children[selected];
                let selected = &app.tasks.tasks[index];

                let mut children = Vec::new();

                for (index, task) in app.tasks.tasks.iter().enumerate() {
                    if task.parent_id == Some(selected.id.clone()) {
                        children.push(index);
                    }
                }

                app.task_edit = task_edit::TaskEdit::new(
                    selected.content.clone(),
                    selected.description.clone(),
                    selected.priority.to_string(),
                    selected.due.as_ref().map_or("", |d| &d.string).to_string(),
                    children,
                    index,
                    task_edit::CurrentlyEditing::Content,
                );
            }
        } else if key.code == KeyCode::Char('n') {
            let task = app.tasks.tasks[app.task_edit.current_task_index].clone();

            app.show_task_editor = false;
            app.show_new_task = true;

            app.new_task = new_task::NewTask::new(task.project_id, Some(task.id));
        }
    }
}

pub fn handle_projects(app: &mut App, key: KeyEvent) {
    if app.projects.move_mode {
        // In move mode, j/k move projects up/down
        if key.code == KeyCode::Char('j') || key.code == KeyCode::Down {
            app.projects.move_down();
        } else if key.code == KeyCode::Char('k') || key.code == KeyCode::Up {
            app.projects.move_up();
        } else if key.code == KeyCode::Esc || key.code == KeyCode::Char('m') {
            // Exit move mode and save project order
            app.projects.move_mode = false;
            let project_order: Vec<String> = app.projects.projects.iter().map(|p| p.id.clone()).collect();
            tokio::spawn(async move {
                if let Err(e) = crate::save_project_order(&project_order) {
                    eprintln!("Failed to save project order: {}", e);
                }
            });
        }
    } else {
        // Normal mode
        if key.code == KeyCode::Char('j') || key.code == KeyCode::Down {
            app.projects.next();
            if let Some(selected) = app.projects.state.selected() {
                let selected_id = app.projects.projects[selected].id.clone();
                app.tasks.filter = crate::tasks::Filter::ProjectId(selected_id.clone());
                app.tasks.filter_task_list(false);
                app.projects.selected_project = Some(selected_id);
            }
        } else if key.code == KeyCode::Char('k') || key.code == KeyCode::Up {
            app.projects.previous();
            if let Some(selected) = app.projects.state.selected() {
                let selected_id = app.projects.projects[selected].id.clone();
                app.tasks.filter = crate::tasks::Filter::ProjectId(selected_id.clone());
                app.tasks.filter_task_list(false);
                app.projects.selected_project = Some(selected_id);
            }
        } else if key.code == KeyCode::Char('m') {
            // Enter move mode
            app.projects.move_mode = true;
        } else if key.code == KeyCode::Char('x') {
            todo!("DELETE PROJECT");
        } else if key.code == KeyCode::Char('a') {
            if let Some(selected) = app.projects.state.selected() {
                let selected_id = app.projects.projects[selected].id.clone();
                app.show_new_task = true;
                app.new_task = new_task::NewTask::new(selected_id, None);
            }
        }
    }
}

pub fn handle_new_tasks(
    app: &mut App,
    key: KeyEvent,
    client: Client,
    tx: std::sync::mpsc::Sender<TaskResult>,
) {
    if key.code == KeyCode::Esc {
        app.show_new_task = !app.show_new_task;
    } else if key.code == KeyCode::Enter {
        app.show_new_task = !app.show_new_task;
        
        // Parse priority from priority_string
        let priority_text = app.new_task.priority_string.lines().join("");
        let priority_text = priority_text.trim();
        if !priority_text.is_empty() {
            if let Ok(priority_value) = priority_text.parse::<u8>() {
                if priority_value >= 1 && priority_value <= 4 {
                    app.new_task.priority = Some(priority_value);
                }
            }
        }
        
        let json = app.new_task.get_json();

        tokio::spawn(async move {
            let result = create_task(&client, json, tx).await;
            if let Err(e) = result {
                eprintln!("Failed to create task: {}", e);
            }
        });
    }
    if key.code == KeyCode::Tab {
        if app.new_task.currently_editing == new_task::CurrentlyEditing::Content {
            app.new_task.currently_editing = new_task::CurrentlyEditing::Description
        } else if app.new_task.currently_editing == new_task::CurrentlyEditing::Description {
            app.new_task.currently_editing = new_task::CurrentlyEditing::Priority
        } else if app.new_task.currently_editing == new_task::CurrentlyEditing::Priority {
            app.new_task.currently_editing = new_task::CurrentlyEditing::DueString
        } else if app.new_task.currently_editing == new_task::CurrentlyEditing::DueString {
            app.new_task.currently_editing = new_task::CurrentlyEditing::Content
        }
        return;
    }
    if app.new_task.currently_editing == new_task::CurrentlyEditing::Content {
        app.new_task.content.input(key);
    } else if app.new_task.currently_editing == new_task::CurrentlyEditing::Description {
        app.new_task.description.input(key);
    } else if app.new_task.currently_editing == new_task::CurrentlyEditing::Priority {
        handle_priority_input(&mut app.new_task.priority_string, key);
    } else if app.new_task.currently_editing == new_task::CurrentlyEditing::DueString {
        app.new_task.due_string.input(key);
    }
}

pub fn handle_tasks(app: &mut App, key: KeyEvent, client: Client) {
    if key.code == KeyCode::Char('j') || key.code == KeyCode::Down {
        app.tasks.next();
    } else if key.code == KeyCode::Char('k') || key.code == KeyCode::Up {
        app.tasks.previous();
    } else if key.code == KeyCode::Enter {
        if let Some(selected) = app.tasks.state.selected() {
            app.show_task_editor = true;
            let index = app.tasks.display_tasks[selected];
            let selected = &app.tasks.tasks[index];

            let mut children = Vec::new();

            for (index, task) in app.tasks.tasks.iter().enumerate() {
                if task.parent_id == Some(selected.id.clone()) {
                    children.push(index);
                }
            }

            app.task_edit = task_edit::TaskEdit::new(
                selected.content.clone(),
                selected.description.clone(),
                selected.priority.to_string(),
                selected.due.as_ref().map_or("", |d| &d.string).to_string(),
                children,
                index,
                task_edit::CurrentlyEditing::Content,
            );
        }
    } else if key.code == KeyCode::Char('x') {
        if let Some(selected) = app.tasks.state.selected() {
            let index = app.tasks.display_tasks[selected];
            let task_id = app.tasks.tasks[index].id.clone();
            
            // Find all children (and their children) to complete
            let mut tasks_to_complete = Vec::new();
            tasks_to_complete.push(task_id.clone());
            
            // Recursively find all children
            find_all_children(&app.tasks.tasks, &task_id, &mut tasks_to_complete);
            
            // Remove all tasks (parent + children) from the tasks list
            // Sort indices in descending order to avoid index shifting issues
            let mut indices_to_remove: Vec<usize> = tasks_to_complete.iter()
                .filter_map(|task_id| app.tasks.tasks.iter().position(|t| t.id == *task_id))
                .collect();
            indices_to_remove.sort_by(|a, b| b.cmp(a)); // Sort descending
            
            for &task_index in &indices_to_remove {
                app.tasks.tasks.remove(task_index);
            }
            
            // Rebuild display_tasks list to ensure valid indices
            app.tasks.filter_task_list(false);
            
            // Calculate new selection position after rebuilding the list
            let new_selection = if app.tasks.display_tasks.is_empty() {
                None
            } else if selected > 0 && selected <= app.tasks.display_tasks.len() {
                // Try to select the previous position, but clamp to valid range
                Some((selected - 1).min(app.tasks.display_tasks.len() - 1))
            } else {
                // If we were at the beginning or the list is shorter now, select the first item
                Some(0)
            };
            
            // Set the new selection
            app.tasks.state.select(new_selection);
            
            // Complete all tasks via API
            for task_id_to_complete in tasks_to_complete {
                let client_clone = client.clone();
                tokio::spawn(async move {
                    close_task(&client_clone, task_id_to_complete).await.unwrap();
                });
            }
        }
    } else if key.code == KeyCode::Char('n') {
        if let Some(selected) = app.projects.state.selected() {
            let selected_id = app.projects.projects[selected].id.clone();
            app.show_new_task = true;
            app.new_task = new_task::NewTask::new(selected_id, None);
        }
    } else if key.code == KeyCode::Char('a') {
        if let Some(selected) = app.projects.state.selected() {
            let selected_id = app.projects.projects[selected].id.clone();
            app.show_new_task = true;
            app.new_task = new_task::NewTask::new(selected_id, None);
        }
    } else if key.code == KeyCode::Char('d') {
        if let Some(selected) = app.tasks.state.selected() {
            let index = app.tasks.display_tasks[selected];
            let task_id = app.tasks.tasks[index].id.clone();
            
            // Store the task ID that should be selected after deletion
            // We'll try to select the next task in the display order, or the previous one if at the end
            let target_task_id = if selected + 1 < app.tasks.display_tasks.len() {
                // Select the next task
                Some(app.tasks.tasks[app.tasks.display_tasks[selected + 1]].id.clone())
            } else if selected > 0 {
                // Select the previous task
                Some(app.tasks.tasks[app.tasks.display_tasks[selected - 1]].id.clone())
            } else {
                // No other tasks to select
                None
            };
            
            // Find all children (and their children) to delete
            let mut tasks_to_delete = Vec::new();
            tasks_to_delete.push(task_id.clone());
            
            // Recursively find all children
            find_all_children(&app.tasks.tasks, &task_id, &mut tasks_to_delete);
            
            // Remove all tasks (parent + children) from the tasks list
            // Sort indices in descending order to avoid index shifting issues
            let mut indices_to_remove: Vec<usize> = tasks_to_delete.iter()
                .filter_map(|task_id| app.tasks.tasks.iter().position(|t| t.id == *task_id))
                .collect();
            indices_to_remove.sort_by(|a, b| b.cmp(a)); // Sort descending
            
            for &task_index in &indices_to_remove {
                app.tasks.tasks.remove(task_index);
            }
            
            // Rebuild display_tasks list to ensure valid indices
            app.tasks.filter_task_list(false);
            
            // Restore selection to the target task if it still exists
            if let Some(target_id) = target_task_id {
                if let Some(new_index) = app.tasks.display_tasks.iter().position(|&idx| app.tasks.tasks[idx].id == target_id) {
                    app.tasks.state.select(Some(new_index));
                } else if !app.tasks.display_tasks.is_empty() {
                    // If target task is no longer visible, select the first task
                    app.tasks.state.select(Some(0));
                } else {
                    app.tasks.state.select(None);
                }
            } else {
                // No target task, select first if available
                if !app.tasks.display_tasks.is_empty() {
                    app.tasks.state.select(Some(0));
                } else {
                    app.tasks.state.select(None);
                }
            }
            
            // Delete all tasks from the API
            for task_id_to_delete in tasks_to_delete {
                let client_clone = client.clone();
                tokio::spawn(async move {
                    delete_task(&client_clone, task_id_to_delete).await.unwrap();
                });
            }
        }
    } else if key.code == KeyCode::Char('o') {
        // Create subtask for selected task
        if let Some(selected) = app.tasks.state.selected() {
            let index = app.tasks.display_tasks[selected];
            let selected_task = &app.tasks.tasks[index];
            app.show_new_task = true;
            app.new_task = new_task::NewTask::new(selected_task.project_id.clone(), Some(selected_task.id.clone()));
        }
    } else if key.code == KeyCode::Char('1') {
        // Set priority to 1 (highest)
        if let Some(selected) = app.tasks.state.selected() {
            let index = app.tasks.display_tasks[selected];
            app.tasks.tasks[index].priority = 1;
        }
    } else if key.code == KeyCode::Char('2') {
        // Set priority to 2
        if let Some(selected) = app.tasks.state.selected() {
            let index = app.tasks.display_tasks[selected];
            app.tasks.tasks[index].priority = 2;
        }
    } else if key.code == KeyCode::Char('3') {
        // Set priority to 3
        if let Some(selected) = app.tasks.state.selected() {
            let index = app.tasks.display_tasks[selected];
            app.tasks.tasks[index].priority = 3;
        }
    } else if key.code == KeyCode::Char('4') {
        // Set priority to 4 (lowest)
        if let Some(selected) = app.tasks.state.selected() {
            let index = app.tasks.display_tasks[selected];
            app.tasks.tasks[index].priority = 4;
        }
    }
}

fn find_all_children(tasks: &Vec<Task>, parent_id: &String, tasks_to_delete: &mut Vec<String>) {
    for task in tasks {
        if let Some(task_parent_id) = &task.parent_id {
            if task_parent_id == parent_id {
                tasks_to_delete.push(task.id.clone());
                // Recursively find children of this child
                find_all_children(tasks, &task.id, tasks_to_delete);
            }
        }
    }
}

fn handle_priority_input(priority_string: &mut tui_textarea::TextArea, key: KeyEvent) {
    match key.code {
        KeyCode::Char(c) if c.is_ascii_digit() && c >= '1' && c <= '4' => {
            // Clear existing content and set the new digit
            *priority_string = tui_textarea::TextArea::from(vec![c.to_string()]);
        }
        KeyCode::Backspace => {
            // Allow backspace to clear the content
            *priority_string = tui_textarea::TextArea::from(vec!["".to_string()]);
        }
        KeyCode::Delete => {
            // Allow delete to clear the content
            *priority_string = tui_textarea::TextArea::from(vec!["".to_string()]);
        }
        _ => {
            // Ignore all other keys (including invalid digits like 0, 5-9)
        }
    }
}
