#![allow(dead_code)]
mod fixed;

use sqlx::postgres::{PgPool, PgPoolOptions};
use tokio::sync::OnceCell;

const PG_URL: &str = "postgres://postgres:postgres@127.0.0.1:5432/postgres";

/// Init ModelManager for test, so that all the test func use the same ModelManager
async fn init_test() -> ModelManager {
    static INIT: OnceCell<ModelManager> = OnceCell::const_new();
    let mm = INIT
        .get_or_init(|| async {
            println!("INIT ModelManager");
            ModelManager::new().await
        })
        .await;
    mm.clone()
}

#[derive(Clone)]
struct ModelManager {
    // postgres connection pool
    pg: PgPool,
}

impl ModelManager {
    async fn new() -> Self {
        let pg = PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_secs(2))
            .max_connections(4)
            .connect(PG_URL)
            .await
            .unwrap();
        Self { pg }
    }

    async fn pg(&self) {
        self.pg.acquire().await.unwrap();
    }
    async fn foo(&self) {
        println!("foo");
    }
}

#[cfg(test)]
mod tests1 {
    use super::*;

    #[tokio::test]
    async fn bug_test() {
        let mm = init_test().await;
        mm.pg().await;
    }
}

#[cfg(test)]
mod tests2 {
    use super::*;

    #[tokio::test]
    async fn bug_test() {
        let _mm = init_test().await;
    }
}
