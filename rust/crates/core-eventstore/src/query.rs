//! Query — read-side operations.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Trait for queries.
pub trait Query: Send + Sync + 'static {
    /// Query type name.
    fn query_type(&self) -> &str;
}

/// Result of a query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult<T> {
    /// Whether the query succeeded.
    pub success: bool,

    /// Result data.
    pub data: Option<T>,

    /// Error message if failed.
    pub error: Option<String>,
}

/// Trait for query handlers.
#[async_trait]
pub trait QueryHandler<Q: Query>: Send + Sync {
    type Output: Send + Sync;

    /// Handle a query and return a result.
    async fn handle(&self, query: Q) -> QueryResult<Self::Output>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestQuery {
        id: String,
    }

    impl Query for TestQuery {
        fn query_type(&self) -> &str {
            "test"
        }
    }

    struct TestQueryHandler;

    #[async_trait]
    impl QueryHandler<TestQuery> for TestQueryHandler {
        type Output = String;

        async fn handle(&self, query: TestQuery) -> QueryResult<String> {
            QueryResult {
                success: true,
                data: Some(format!("result for {}", query.id)),
                error: None,
            }
        }
    }

    #[tokio::test]
    async fn query_handler() {
        let handler = TestQueryHandler;
        let result = handler.handle(TestQuery { id: "1".into() }).await;
        assert!(result.success);
        assert_eq!(result.data.unwrap(), "result for 1");
    }
}
