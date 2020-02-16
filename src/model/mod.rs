use diesel::connection::{Connection, SimpleConnection};
use diesel::deserialize::{Queryable, QueryableByName};
use diesel::query_builder::{AsQuery, QueryFragment, QueryId};
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::r2d2::PooledConnection;
use diesel::result::{ConnectionResult, QueryResult};
use diesel::sql_types::HasSqlType;
use diesel::sqlite::SqliteConnection;
use parking_lot::Mutex;
use parking_lot::MutexGuard;
use std::fmt;
use std::sync::Arc;

pub mod queues;
pub mod repositories;
pub mod schema;
pub mod users;

/// Source: https://stackoverflow.com/a/57717533
pub struct WriteConnection(SqliteConnection);

impl fmt::Debug for WriteConnection {
	fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
		fmt.debug_struct("DbConnection").finish()
	}
}

impl SimpleConnection for WriteConnection {
	fn batch_execute(&self, query: &str) -> QueryResult<()> {
		self.0.batch_execute(query)
	}
}

impl Connection for WriteConnection {
	type Backend = <SqliteConnection as Connection>::Backend;
	type TransactionManager = <SqliteConnection as Connection>::TransactionManager;

	fn establish(database_url: &str) -> ConnectionResult<Self> {
		let connection = SqliteConnection::establish(database_url);
		match connection {
			Ok(connection) => {
				connection
					.batch_execute(
						r#"
							PRAGMA synchronous = NORMAL;
							PRAGMA journal_mode = WAL;
							PRAGMA foreign_keys = ON;
							PRAGMA busy_timeout = 60000;
						"#,
					)
					.expect("Could not establish a new connection");
				Ok(Self(connection))
			}
			Err(error) => Err(error),
		}
	}

	fn execute(&self, query: &str) -> QueryResult<usize> {
		self.0.execute(query)
	}

	fn query_by_index<T, U>(&self, source: T) -> QueryResult<Vec<U>>
	where
		T: AsQuery,
		T::Query: QueryFragment<Self::Backend> + QueryId,
		Self::Backend: HasSqlType<T::SqlType>,
		U: Queryable<T::SqlType, Self::Backend>,
	{
		self.0.query_by_index(source)
	}

	fn query_by_name<T, U>(&self, source: &T) -> QueryResult<Vec<U>>
	where
		T: QueryFragment<Self::Backend> + QueryId,
		U: QueryableByName<Self::Backend>,
	{
		self.0.query_by_name(source)
	}

	fn execute_returning_count<T>(&self, source: &T) -> QueryResult<usize>
	where
		T: QueryFragment<Self::Backend> + QueryId,
	{
		self.0.execute_returning_count(source)
	}

	fn transaction_manager(&self) -> &Self::TransactionManager {
		self.0.transaction_manager()
	}
}

pub struct ReadConnection(SqliteConnection);

impl fmt::Debug for ReadConnection {
	fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
		fmt.debug_struct("DbConnection").finish()
	}
}

impl SimpleConnection for ReadConnection {
	fn batch_execute(&self, query: &str) -> QueryResult<()> {
		self.0.batch_execute(query)
	}
}

impl Connection for ReadConnection {
	type Backend = <SqliteConnection as Connection>::Backend;
	type TransactionManager = <SqliteConnection as Connection>::TransactionManager;

	fn establish(database_url: &str) -> ConnectionResult<Self> {
		let connection = SqliteConnection::establish(database_url);
		match connection {
			Ok(connection) => {
				connection
					.batch_execute(
						r#"
							PRAGMA foreign_keys = ON;
							PRAGMA busy_timeout = 60000;
						"#,
					)
					.expect("Could not establish a new connection");
				Ok(Self(connection))
			}
			Err(error) => Err(error),
		}
	}

	fn execute(&self, query: &str) -> QueryResult<usize> {
		self.0.execute(query)
	}

	fn query_by_index<T, U>(&self, source: T) -> QueryResult<Vec<U>>
	where
		T: AsQuery,
		T::Query: QueryFragment<Self::Backend> + QueryId,
		Self::Backend: HasSqlType<T::SqlType>,
		U: Queryable<T::SqlType, Self::Backend>,
	{
		self.0.query_by_index(source)
	}

	fn query_by_name<T, U>(&self, source: &T) -> QueryResult<Vec<U>>
	where
		T: QueryFragment<Self::Backend> + QueryId,
		U: QueryableByName<Self::Backend>,
	{
		self.0.query_by_name(source)
	}

	fn execute_returning_count<T>(&self, source: &T) -> QueryResult<usize>
	where
		T: QueryFragment<Self::Backend> + QueryId,
	{
		self.0.execute_returning_count(source)
	}

	fn transaction_manager(&self) -> &Self::TransactionManager {
		self.0.transaction_manager()
	}
}

pub type PooledDbConnection = PooledConnection<ConnectionManager<ReadConnection>>;
pub type ReadPool = Pool<ConnectionManager<ReadConnection>>;

#[derive(Debug, Clone)]
pub struct DbConnectionManager {
	pub write_connection: Arc<Mutex<WriteConnection>>,
	pub read_pool: Arc<Mutex<ReadPool>>,
}

impl DbConnectionManager {
	pub fn get_write(&self) -> MutexGuard<WriteConnection> {
		self.write_connection.lock()
	}

	pub fn get_read(&self) -> PooledDbConnection {
		self.read_pool.lock().get().unwrap()
	}
}
