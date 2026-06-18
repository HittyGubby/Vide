use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tokio::sync::Semaphore;

/// Status of a tracked task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Running,
    Cancelled,
    Completed,
    Failed(String),
}

/// Information about a running plugin task.
#[derive(Debug)]
pub struct TaskInfo {
    pub plugin: String,
    pub child: Child,
    pub status: TaskStatus,
    pub started_at: Instant,
}

impl TaskInfo {
    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }
}

/// Manage plugin subprocess lifecycle: spawning, tracking, cancellation, concurrency limits.
pub struct ProcessManager {
    tasks: Mutex<HashMap<String, TaskInfo>>,
    semaphores: Mutex<HashMap<String, Arc<Semaphore>>>,
}

impl ProcessManager {
    /// Create a new empty ProcessManager.
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(HashMap::new()),
            semaphores: Mutex::new(HashMap::new()),
        }
    }

    /// Set the maximum number of concurrent processes for a plugin.
    /// Defaults to 4 if not set.
    pub fn set_concurrency(&self, plugin: &str, max: usize) {
        let mut semaphores = self.semaphores.lock().unwrap();
        semaphores.insert(plugin.to_string(), Arc::new(Semaphore::new(max)));
    }

    /// Get (or create) the semaphore for a plugin.
    fn get_semaphore(&self, plugin: &str) -> Arc<Semaphore> {
        let mut semaphores = self.semaphores.lock().unwrap();
        semaphores
            .get(plugin)
            .cloned()
            .unwrap_or_else(|| {
                let s = Arc::new(Semaphore::new(4));
                semaphores.insert(plugin.to_string(), Arc::clone(&s));
                s
            })
    }

    /// Spawn a plugin process, register the task, and return its task ID.
    ///
    /// The process is spawned with stdin/stdout piped for JSON lines communication.
    /// The task is tracked in the task table for cancellation and monitoring.
    pub fn spawn_plugin(
        &self,
        task_id: String,
        plugin: &str,
        exec: &[String],
    ) -> std::io::Result<()> {
        let permit = self.get_semaphore(plugin);
        // Acquire synchronously — in production this should be async.
        // For Phase 2, we use try_acquire to avoid blocking.
        let _permit = permit.try_acquire().map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                format!("concurrency limit reached for plugin '{plugin}'"),
            )
        })?;

        let child = Command::new(&exec[0])
            .args(&exec[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let info = TaskInfo {
            plugin: plugin.to_string(),
            child,
            status: TaskStatus::Running,
            started_at: Instant::now(),
        };

        let mut tasks = self.tasks.lock().unwrap();
        tasks.insert(task_id, info);

        Ok(())
    }

    /// Mark a task as cancelled and kill its child process.
    ///
    /// On Unix, this sends SIGKILL. Phase 3 will add a two-stage
    /// SIGTERM → timeout → SIGKILL mechanism.
    pub fn cancel_task(&self, task_id: &str) -> Option<TaskStatus> {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(info) = tasks.get_mut(task_id) {
            if info.status == TaskStatus::Running {
                let _ = info.child.kill();
                let _ = info.child.wait();
                info.status = TaskStatus::Cancelled;
            }
            Some(info.status.clone())
        } else {
            None
        }
    }

    /// Get the status of a task.
    pub fn task_status(&self, task_id: &str) -> Option<TaskStatus> {
        let tasks = self.tasks.lock().unwrap();
        tasks.get(task_id).map(|t| t.status.clone())
    }

    /// Remove a completed task from the table and return its info.
    pub fn remove_task(&self, task_id: &str) -> Option<TaskInfo> {
        let mut tasks = self.tasks.lock().unwrap();
        tasks.remove(task_id)
    }

    /// Check and clean up tasks whose child processes have exited.
    /// Returns the IDs of tasks that finished since the last check.
    pub fn reap_tasks(&self) -> Vec<String> {
        let mut tasks = self.tasks.lock().unwrap();
        let mut reaped = Vec::new();

        let to_remove: Vec<String> = tasks
            .iter_mut()
            .filter_map(|(id, info)| {
                if info.status != TaskStatus::Running {
                    return None;
                }
                match info.child.try_wait() {
                    Ok(Some(status)) => {
                        if status.success() {
                            info.status = TaskStatus::Completed;
                        } else {
                            let msg = status
                                .code()
                                .map(|c| format!("exit code {c}"))
                                .unwrap_or_else(|| "signal".to_string());
                            info.status = TaskStatus::Failed(msg);
                        }
                        Some(id.clone())
                    }
                    Ok(None) => None,
                    Err(_) => {
                        info.status = TaskStatus::Failed("wait error".to_string());
                        Some(id.clone())
                    }
                }
            })
            .collect();

        for id in &to_remove {
            if let Some(info) = tasks.remove(id) {
                // Drop the child (closes stdin, etc.)
                drop(info);
                reaped.push(id.clone());
            }
        }

        reaped
    }

    /// Number of currently tracked running tasks.
    pub fn running_count(&self) -> usize {
        let tasks = self.tasks.lock().unwrap();
        tasks.values().filter(|t| t.status == TaskStatus::Running).count()
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_manager_is_empty() {
        let pm = ProcessManager::new();
        assert_eq!(pm.running_count(), 0);
    }

    #[test]
    fn test_set_concurrency() {
        let pm = ProcessManager::new();
        pm.set_concurrency("test-plugin", 2);
        let sem = pm.get_semaphore("test-plugin");
        assert_eq!(sem.available_permits(), 2);
    }

    #[test]
    fn test_concurrency_defaults_to_4() {
        let pm = ProcessManager::new();
        let sem = pm.get_semaphore("default-plugin");
        assert_eq!(sem.available_permits(), 4);
    }
}
