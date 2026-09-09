//! People — used to attribute shared-card spending. Seeded with a single
//! `is_self` person the first time the list is read.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::util::{new_id, now};

#[derive(Serialize, sqlx::FromRow)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub is_self: bool,
    pub color: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct PersonInput {
    pub name: String,
    pub color: Option<String>,
}

#[tauri::command]
pub async fn list_people(state: State<'_, AppState>) -> AppResult<Vec<Person>> {
    let db = state.db().await?;
    let pool = &db.pool;
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM people")
        .fetch_one(pool)
        .await?;
    if count.0 == 0 {
        sqlx::query(
            "INSERT INTO people (id, name, is_self, color, created_at) VALUES (?1, ?2, 1, ?3, ?4)",
        )
        .bind(new_id())
        .bind("Me")
        .bind("#4f46e5")
        .bind(now())
        .execute(pool)
        .await?;
    }
    let people = sqlx::query_as::<_, Person>(
        "SELECT id, name, is_self, color, created_at FROM people
         ORDER BY is_self DESC, name COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await?;
    Ok(people)
}

#[tauri::command]
pub async fn create_person(state: State<'_, AppState>, input: PersonInput) -> AppResult<Person> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::Invalid("name is required".into()));
    }
    let db = state.db().await?;
    let id = new_id();
    sqlx::query(
        "INSERT INTO people (id, name, is_self, color, created_at) VALUES (?1, ?2, 0, ?3, ?4)",
    )
    .bind(&id)
    .bind(name)
    .bind(input.color.as_deref())
    .bind(now())
    .execute(&db.pool)
    .await?;
    sqlx::query_as::<_, Person>(
        "SELECT id, name, is_self, color, created_at FROM people WHERE id = ?1",
    )
    .bind(&id)
    .fetch_one(&db.pool)
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn update_person(
    state: State<'_, AppState>,
    id: String,
    input: PersonInput,
) -> AppResult<()> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::Invalid("name is required".into()));
    }
    let db = state.db().await?;
    let res = sqlx::query("UPDATE people SET name = ?2, color = ?3 WHERE id = ?1")
        .bind(&id)
        .bind(name)
        .bind(input.color.as_deref())
        .execute(&db.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("person {id}")));
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_person(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let db = state.db().await?;
    let person: Option<Person> = sqlx::query_as::<_, Person>(
        "SELECT id, name, is_self, color, created_at FROM people WHERE id = ?1",
    )
    .bind(&id)
    .fetch_optional(&db.pool)
    .await?;
    let Some(person) = person else {
        return Err(AppError::NotFound(format!("person {id}")));
    };
    if person.is_self {
        return Err(AppError::Invalid("cannot delete the primary person".into()));
    }
    // Detach any transactions / rules that referenced them.
    sqlx::query("UPDATE transactions SET owner_person_id = NULL WHERE owner_person_id = ?1")
        .bind(&id)
        .execute(&db.pool)
        .await?;
    sqlx::query("UPDATE transactions SET suggested_person_id = NULL WHERE suggested_person_id = ?1")
        .bind(&id)
        .execute(&db.pool)
        .await?;
    sqlx::query("DELETE FROM rules WHERE set_person_id = ?1")
        .bind(&id)
        .execute(&db.pool)
        .await?;
    sqlx::query("DELETE FROM people WHERE id = ?1")
        .bind(&id)
        .execute(&db.pool)
        .await?;
    Ok(())
}
