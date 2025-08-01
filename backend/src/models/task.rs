use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, Type};
use ts_rs::TS;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, TS, ToSchema)]
#[sqlx(type_name = "task_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum TaskStatus {
    Todo,
    InProgress,
    InReview,
    Done,
    Cancelled,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct Task {
    pub id: Uuid,
    pub project_id: Uuid, // Foreign key to Project
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub wish_id: String, // Required: Grouping field for task organization
    pub parent_task_attempt: Option<Uuid>, // Foreign key to parent TaskAttempt
    pub created_by: Option<Uuid>, // User who created this task
    pub assigned_to: Option<Uuid>, // User assigned to this task
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct TaskWithAttemptStatus {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub wish_id: String,
    pub parent_task_attempt: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub assigned_to: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub has_in_progress_attempt: bool,
    pub has_merged_attempt: bool,
    pub last_attempt_failed: bool,
    pub latest_attempt_executor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct TaskWithUsers {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub wish_id: String,
    pub parent_task_attempt: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub assigned_to: Option<Uuid>,
    pub creator_username: Option<String>,
    pub creator_display_name: Option<String>,
    pub assignee_username: Option<String>,
    pub assignee_display_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct CreateTask {
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub wish_id: String, // Required: Wish grouping identifier
    pub parent_task_attempt: Option<Uuid>,
    pub created_by: Option<Uuid>, // User creating this task
    pub assigned_to: Option<Uuid>, // User assigned to this task
}

#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct CreateTaskAndStart {
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub wish_id: String, // Required: Wish grouping identifier
    pub parent_task_attempt: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub assigned_to: Option<Uuid>,
    pub executor: Option<crate::executor::ExecutorConfig>,
}

#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct UpdateTask {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub wish_id: Option<String>, // Optional: Can reassign wish
    pub parent_task_attempt: Option<Uuid>,
    pub assigned_to: Option<Uuid>, // Optional: Can reassign task
}

impl Task {
    pub async fn find_by_project_id_with_attempt_status(
        pool: &SqlitePool,
        project_id: Uuid,
    ) -> Result<Vec<TaskWithAttemptStatus>, sqlx::Error> {
        let records = sqlx::query!(
            r#"SELECT
  t.id                            AS "id!: Uuid",
  t.project_id                    AS "project_id!: Uuid",
  t.title,
  t.description,
  t.status                        AS "status!: TaskStatus",
  t.wish_id,
  t.parent_task_attempt           AS "parent_task_attempt: Uuid",
  t.created_by                    AS "created_by: Uuid",
  t.assigned_to                   AS "assigned_to: Uuid",
  t.created_at                    AS "created_at!: DateTime<Utc>",
  t.updated_at                    AS "updated_at!: DateTime<Utc>",

  CASE WHEN EXISTS (
    SELECT 1
      FROM task_attempts ta
      JOIN execution_processes ep
        ON ep.task_attempt_id = ta.id
     WHERE ta.task_id       = t.id
       AND ep.status        = 'running'
       AND ep.process_type IN ('setupscript','cleanupscript','codingagent')
     LIMIT 1
  ) THEN 1 ELSE 0 END            AS "has_in_progress_attempt!: i64",

  CASE WHEN EXISTS (
    SELECT 1
      FROM task_attempts ta
     WHERE ta.task_id       = t.id
       AND ta.merge_commit IS NOT NULL
     LIMIT 1
  ) THEN 1 ELSE 0 END            AS "has_merged_attempt!: i64",

  CASE WHEN (
    SELECT ep.status
      FROM task_attempts ta
      JOIN execution_processes ep
        ON ep.task_attempt_id = ta.id
     WHERE ta.task_id       = t.id
     AND ep.process_type IN ('setupscript','cleanupscript','codingagent')
     ORDER BY ep.created_at DESC
     LIMIT 1
  ) IN ('failed','killed') THEN 1 ELSE 0 END
                                 AS "last_attempt_failed!: i64",

  ( SELECT ta.executor
      FROM task_attempts ta
     WHERE ta.task_id = t.id
     ORDER BY ta.created_at DESC
     LIMIT 1
  )                               AS "latest_attempt_executor"

FROM tasks t
WHERE t.project_id = $1
ORDER BY t.created_at DESC"#,
            project_id
        )
        .fetch_all(pool)
        .await?;

        let tasks = records
            .into_iter()
            .map(|rec| TaskWithAttemptStatus {
                id: rec.id,
                project_id: rec.project_id,
                title: rec.title,
                description: rec.description,
                status: rec.status,
                wish_id: rec.wish_id,
                parent_task_attempt: rec.parent_task_attempt,
                created_by: rec.created_by,
                assigned_to: rec.assigned_to,
                created_at: rec.created_at,
                updated_at: rec.updated_at,
                has_in_progress_attempt: rec.has_in_progress_attempt != 0,
                has_merged_attempt: rec.has_merged_attempt != 0,
                last_attempt_failed: rec.last_attempt_failed != 0,
                latest_attempt_executor: rec.latest_attempt_executor,
            })
            .collect();

        Ok(tasks)
    }

    #[allow(dead_code)]
    pub async fn find_by_project_id_with_users(
        pool: &SqlitePool,
        project_id: Uuid,
    ) -> Result<Vec<TaskWithUsers>, sqlx::Error> {
        let records = sqlx::query!(
            r#"SELECT 
                t.id as "id!: Uuid", t.project_id as "project_id!: Uuid", t.title, t.description, 
                t.status as "status!: TaskStatus", t.wish_id, 
                t.parent_task_attempt as "parent_task_attempt: Uuid", 
                t.created_by as "created_by: Uuid", t.assigned_to as "assigned_to: Uuid", 
                t.created_at as "created_at!: DateTime<Utc>", t.updated_at as "updated_at!: DateTime<Utc>",
                creator.username as creator_username,
                creator.display_name as creator_display_name,
                assignee.username as assignee_username,
                assignee.display_name as assignee_display_name
               FROM tasks t
               LEFT JOIN users creator ON t.created_by = creator.id
               LEFT JOIN users assignee ON t.assigned_to = assignee.id
               WHERE t.project_id = $1
               ORDER BY t.created_at DESC"#,
            project_id
        )
        .fetch_all(pool)
        .await?;

        let tasks = records
            .into_iter()
            .map(|rec| TaskWithUsers {
                id: rec.id,
                project_id: rec.project_id,
                title: rec.title,
                description: rec.description,
                status: rec.status,
                wish_id: rec.wish_id,
                parent_task_attempt: rec.parent_task_attempt,
                created_by: rec.created_by,
                assigned_to: rec.assigned_to,
                creator_username: rec.creator_username,
                creator_display_name: rec.creator_display_name,
                assignee_username: rec.assignee_username,
                assignee_display_name: rec.assignee_display_name,
                created_at: rec.created_at,
                updated_at: rec.updated_at,
            })
            .collect();

        Ok(tasks)
    }

    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Task,
            r#"SELECT id as "id!: Uuid", project_id as "project_id!: Uuid", title, description, status as "status!: TaskStatus", wish_id, parent_task_attempt as "parent_task_attempt: Uuid", created_by as "created_by: Uuid", assigned_to as "assigned_to: Uuid", created_at as "created_at!: DateTime<Utc>", updated_at as "updated_at!: DateTime<Utc>"
               FROM tasks 
               WHERE id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_id_and_project_id(
        pool: &SqlitePool,
        id: Uuid,
        project_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Task,
            r#"SELECT id as "id!: Uuid", project_id as "project_id!: Uuid", title, description, status as "status!: TaskStatus", wish_id, parent_task_attempt as "parent_task_attempt: Uuid", created_by as "created_by: Uuid", assigned_to as "assigned_to: Uuid", created_at as "created_at!: DateTime<Utc>", updated_at as "updated_at!: DateTime<Utc>"
               FROM tasks 
               WHERE id = $1 AND project_id = $2"#,
            id,
            project_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        data: &CreateTask,
        task_id: Uuid,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            Task,
            r#"INSERT INTO tasks (id, project_id, title, description, status, wish_id, parent_task_attempt, created_by, assigned_to) 
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) 
               RETURNING id as "id!: Uuid", project_id as "project_id!: Uuid", title, description, status as "status!: TaskStatus", wish_id, parent_task_attempt as "parent_task_attempt: Uuid", created_by as "created_by: Uuid", assigned_to as "assigned_to: Uuid", created_at as "created_at!: DateTime<Utc>", updated_at as "updated_at!: DateTime<Utc>""#,
            task_id,
            data.project_id,
            data.title,
            data.description,
            TaskStatus::Todo as TaskStatus,
            data.wish_id,
            data.parent_task_attempt,
            data.created_by,
            data.assigned_to
        )
        .fetch_one(pool)
        .await
    }

    pub async fn update(
        pool: &SqlitePool,
        id: Uuid,
        project_id: Uuid,
        title: String,
        description: Option<String>,
        status: TaskStatus,
        wish_id: String,
        parent_task_attempt: Option<Uuid>,
        assigned_to: Option<Uuid>,
    ) -> Result<Self, sqlx::Error> {
        let status_value = status as TaskStatus;
        sqlx::query_as!(
            Task,
            r#"UPDATE tasks 
               SET title = $3, description = $4, status = $5, wish_id = $6, parent_task_attempt = $7, assigned_to = $8 
               WHERE id = $1 AND project_id = $2 
               RETURNING id as "id!: Uuid", project_id as "project_id!: Uuid", title, description, status as "status!: TaskStatus", wish_id, parent_task_attempt as "parent_task_attempt: Uuid", created_by as "created_by: Uuid", assigned_to as "assigned_to: Uuid", created_at as "created_at!: DateTime<Utc>", updated_at as "updated_at!: DateTime<Utc>""#,
            id,
            project_id,
            title,
            description,
            status_value,
            wish_id,
            parent_task_attempt,
            assigned_to
        )
        .fetch_one(pool)
        .await
    }

    pub async fn update_status(
        pool: &SqlitePool,
        id: Uuid,
        project_id: Uuid,
        status: TaskStatus,
    ) -> Result<(), sqlx::Error> {
        let status_value = status as TaskStatus;
        sqlx::query!(
            "UPDATE tasks SET status = $3, updated_at = CURRENT_TIMESTAMP WHERE id = $1 AND project_id = $2",
            id,
            project_id,
            status_value
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid, project_id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            "DELETE FROM tasks WHERE id = $1 AND project_id = $2",
            id,
            project_id
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn exists(
        pool: &SqlitePool,
        id: Uuid,
        project_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            "SELECT id as \"id!: Uuid\" FROM tasks WHERE id = $1 AND project_id = $2",
            id,
            project_id
        )
        .fetch_optional(pool)
        .await?;
        Ok(result.is_some())
    }

    pub async fn find_related_tasks_by_attempt_id(
        pool: &SqlitePool,
        attempt_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        // Find both children and parent for this attempt
        sqlx::query_as!(
            Task,
            r#"SELECT DISTINCT t.id as "id!: Uuid", t.project_id as "project_id!: Uuid", t.title, t.description, t.status as "status!: TaskStatus", t.wish_id, t.parent_task_attempt as "parent_task_attempt: Uuid", t.created_by as "created_by: Uuid", t.assigned_to as "assigned_to: Uuid", t.created_at as "created_at!: DateTime<Utc>", t.updated_at as "updated_at!: DateTime<Utc>"
               FROM tasks t
               WHERE (
                   -- Find children: tasks that have this attempt as parent
                   t.parent_task_attempt = $1 AND t.project_id = $2
               ) OR (
                   -- Find parent: task that owns the parent attempt of current task
                   EXISTS (
                       SELECT 1 FROM tasks current_task 
                       JOIN task_attempts parent_attempt ON current_task.parent_task_attempt = parent_attempt.id
                       WHERE parent_attempt.task_id = t.id 
                       AND parent_attempt.id = $1 
                       AND current_task.project_id = $2
                   )
               )
               -- Exclude the current task itself to prevent circular references
               AND t.id != (SELECT task_id FROM task_attempts WHERE id = $1)
               ORDER BY t.created_at DESC"#,
            attempt_id,
            project_id
        )
        .fetch_all(pool)
        .await
    }


}
