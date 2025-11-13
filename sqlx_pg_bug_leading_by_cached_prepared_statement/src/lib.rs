//! This is a postgres specified bug in sqlx.
//!
//! In this example, setup postgres database first:
//! `podman run -itd --name postgres -p 5432:5432 -e POSTGRES_PASSWORD=postgres --replace postgres:latest`

#![allow(dead_code)]

use sqlx::{Encode, PgConnection, Postgres, Type, postgres::PgRow, prelude::FromRow};

/// Init the databse as as example
async fn init(con: &mut PgConnection) {
    // Create a table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            name VARCHAR
        )",
    )
    .execute(&mut *con)
    .await
    .unwrap();
    // Insert one record
    sqlx::query("INSERT INTO users (name) VALUES ('Louis')")
        .execute(con)
        .await
        .unwrap();
}

#[derive(FromRow)]
struct User {
    id: i32,
    name: String,
}

// Query the first user in table users by field.
//
// Notice: the value is generic, which can be i32 or i64 and so on.
async fn first_by<V, T>(con: &mut PgConnection, field: impl AsRef<str>, value: V) -> Option<T>
where
    for<'a> V: Encode<'a, Postgres> + Type<Postgres>,
    for<'r> T: FromRow<'r, PgRow> + Send + Unpin,
{
    sqlx::query_as(&format!(
        "SELECT * FROM users WHERE \"{}\" = $1",
        field.as_ref()
    ))
    .bind(value)
    .fetch_optional(con)
    .await
    .unwrap()
}

#[tokio::test]
async fn test() {
    use sqlx::Connection as _;

    // establish the connection
    let mut con =
        sqlx::PgConnection::connect("postgres://postgres:postgres@127.0.0.1:5432/postgres")
            .await
            .unwrap();
    // init the database
    init(&mut con).await;

    // query with i64 id
    let _: User = first_by(&mut con, "id", 1i64).await.unwrap();
    // query with i32 id
    let _: User = first_by(&mut con, "id", 1i32).await.unwrap();

    // ```text
    // thread 'test' (1437412) panicked at sqlx_bug_leading_by_invalid_cache/src/lib.rs:50:6:
    // called `Result::unwrap()` on an `Err` value: Database(PgDatabaseError {
    //      severity: Error,
    //      code: "08P01",
    //      message: "insufficient data left in message",
    //      detail: None, hint: None,
    //      position: None,
    //      where: Some("unnamed portal parameter $1"),
    //      schema: None, table: None,
    //      column: None, data_type: None,
    //      constraint: None,
    //      file: Some("pqformat.c"),
    //      line: Some(531),
    //      routine: Some("pq_copymsgbytes")
    //  })
    // ```
    //
    // The reason is that changing parameter types does not invalidate cached prepared statement.
    //
    // And this bug is postgres specified, as I tested, sqlite doesn't have this problem.
    //
    // The details are in this issue: https://github.com/launchbadge/sqlx/issues/2885
}

// What's the influence?
//
// We have to admit, `first_by` above is very convinient
// and releases us from writing tedious function like `first_by_id`/`first_by_name`.
//
// However, generic value param is dangerous to some extent.
//
// In example above, we only established one connection for simplifting, so it fails easily.
//
// But in real production environment, we would initialize multiple connections as a Pool.
// Each first call of `first_by` with the same field but different value types will
// lead postgres db to prepare different cached statement.
// And the following call of `first_by` with different value types may or may_not be sent
// to the correct connection. Then coders will find things fails randomly!
//
// Which is to say, both i32 and i64 fulfill the trait bound, but the order of usage will
// leads one can work but another crash. This breaks Rust's coding model.
//
// To avoid this situation, coders should keep in mind the connection is stateful,
// and queries are actually a little bit unsafe.
