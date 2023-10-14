use std::net::SocketAddr;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use rearch::Container;
use serde::Serialize;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/todos", get(list_todos).post(create_todo))
        .route("/todos/:id", get(read_todo).delete(delete_todo))
        .with_state(Container::new());

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Serialize)]
struct TodoWithId {
    uuid: Uuid,
    content: String,
}

// We define our todo db capsules here
mod todo_db {
    use std::sync::Arc;

    use rearch::CapsuleHandle;
    use redb::{Database, ReadOnlyTable, ReadableTable, Table, TableDefinition};
    use uuid::Uuid;

    use crate::TodoWithId;

    const TODOS_TABLE: TableDefinition<u128, &str> = TableDefinition::new("todos");

    fn db_capsule(_: CapsuleHandle) -> Arc<Database> {
        let db = Database::create("todos.redb").unwrap();
        {
            // Table must be created in order for reads to work before any writes
            let create_table_txn = db.begin_write().unwrap();
            create_table_txn.open_table(TODOS_TABLE).unwrap();
            create_table_txn.commit().unwrap();
        }
        Arc::new(db)
    }

    fn with_read_txn_capsule<F, R>(
        CapsuleHandle { mut get, .. }: CapsuleHandle,
    ) -> impl Fn(F) -> Result<R, redb::Error> + Send + Sync + Clone
    where
        F: FnOnce(ReadOnlyTable<'_, u128, &str>) -> Result<R, redb::Error>,
    {
        let db = get.get(db_capsule);
        move |with_table| {
            let txn = db.begin_read()?;
            let table = txn.open_table(TODOS_TABLE)?;
            with_table(table)
        }
    }

    fn with_write_txn_capsule<F, R>(
        CapsuleHandle { mut get, .. }: CapsuleHandle,
    ) -> impl Fn(F) -> Result<R, redb::Error> + Send + Sync + Clone
    where
        F: FnOnce(Table<'_, '_, u128, &str>) -> Result<R, redb::Error>,
    {
        let db = get.get(db_capsule);
        move |with_table| {
            let txn = db.begin_write()?;
            let table = txn.open_table(TODOS_TABLE)?;
            let result = with_table(table);
            txn.commit()?;
            result
        }
    }

    pub(super) fn read_todo_capsule(
        CapsuleHandle { mut get, .. }: CapsuleHandle,
    ) -> impl Fn(Uuid) -> Result<Option<String>, redb::Error> + Send + Sync + Clone {
        let with_txn = get.get(with_read_txn_capsule);
        move |uuid| {
            with_txn(move |table| {
                let content = table.get(uuid.as_u128())?.map(|s| s.value().to_owned());
                Ok(content)
            })
        }
    }

    pub(super) fn create_todo_capsule(
        CapsuleHandle { mut get, .. }: CapsuleHandle,
    ) -> impl Fn(String) -> Result<TodoWithId, redb::Error> + Send + Sync + Clone {
        let with_txn = get.get(with_write_txn_capsule);
        move |content| {
            with_txn(move |mut table| {
                let uuid = Uuid::new_v4();
                table.insert(uuid.as_u128(), content.as_str())?;
                Ok(TodoWithId { uuid, content })
            })
        }
    }

    pub(super) fn delete_todo_capsule(
        CapsuleHandle { mut get, .. }: CapsuleHandle,
    ) -> impl Fn(Uuid) -> Result<Option<String>, redb::Error> + Send + Sync + Clone {
        let with_txn = get.get(with_write_txn_capsule);
        move |uuid| {
            with_txn(move |mut table| {
                let removed_todo = table.remove(uuid.as_u128())?.map(|s| s.value().to_owned());
                Ok(removed_todo)
            })
        }
    }

    pub(super) fn list_todos_capsule(
        CapsuleHandle { mut get, .. }: CapsuleHandle,
    ) -> impl Fn() -> Result<Vec<TodoWithId>, redb::Error> + Send + Sync + Clone {
        let with_txn = get.get(with_read_txn_capsule);
        move || {
            with_txn(|table| {
                table
                    .iter()?
                    .map(|read_result| {
                        let (uuid, content) = read_result?;
                        let uuid = Uuid::from_u128(uuid.value());
                        let content = content.value().to_owned();
                        Ok(TodoWithId { uuid, content })
                    })
                    .collect::<Result<Vec<_>, redb::Error>>()
            })
        }
    }
}
use todo_db::{create_todo_capsule, delete_todo_capsule, list_todos_capsule, read_todo_capsule};

async fn list_todos(State(container): State<Container>) -> Result<Json<Vec<TodoWithId>>, AppError> {
    let todos = container.read(list_todos_capsule)()?;
    Ok(Json(todos))
}

async fn create_todo(
    State(container): State<Container>,
    content: String,
) -> Result<Json<TodoWithId>, AppError> {
    let todo = container.read(create_todo_capsule)(content)?;
    Ok(Json(todo))
}

async fn read_todo(
    State(container): State<Container>,
    Path(path): Path<Uuid>,
) -> Result<String, AppError> {
    let todo = container.read(read_todo_capsule)(path)?;
    match todo {
        Some(todo) => Ok(todo),
        None => Err(AppError::TodoNotFound),
    }
}

async fn delete_todo(
    State(container): State<Container>,
    Path(path): Path<Uuid>,
) -> Result<String, AppError> {
    let todo = container.read(delete_todo_capsule)(path)?;
    match todo {
        Some(todo) => Ok(todo),
        None => Err(AppError::TodoNotFound),
    }
}

enum AppError {
    Redb(redb::Error),
    TodoNotFound,
}
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::Redb(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database operation failed: {e}"),
            ),
            AppError::TodoNotFound => (StatusCode::NOT_FOUND, "Todo not found".to_owned()),
        }
        .into_response()
    }
}
impl From<redb::Error> for AppError {
    fn from(err: redb::Error) -> Self {
        Self::Redb(err.into())
    }
}
