use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior, params};

const DATABASE_PATH_ENV: &str = "ZETA_PRACTICE_DB_PATH";
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "storage_metadata",
        sql: include_str!("../migrations/0001_storage_metadata.sql"),
    },
    Migration {
        version: 2,
        name: "accounts",
        sql: include_str!("../migrations/0002_accounts.sql"),
    },
];

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

#[derive(Debug)]
pub(crate) struct StorageError(String);

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for StorageError {}

pub(crate) fn initialize_from_env() -> Result<(), StorageError> {
    initialize_configured(env::var_os(DATABASE_PATH_ENV)).map(|_| ())
}

pub(crate) fn open_from_env() -> Result<Connection, StorageError> {
    let path = env::var_os(DATABASE_PATH_ENV)
        .ok_or_else(|| StorageError(format!("{DATABASE_PATH_ENV} is required")))?;
    open_connection(&PathBuf::from(path))
}

fn initialize_configured(value: Option<OsString>) -> Result<Connection, StorageError> {
    let path = value.ok_or_else(|| StorageError(format!("{DATABASE_PATH_ENV} is required")))?;
    let path = PathBuf::from(path);
    initialize(&path)
}

fn initialize(path: &Path) -> Result<Connection, StorageError> {
    let mut connection = open_connection(path)?;
    apply_migrations(&mut connection, MIGRATIONS)?;
    Ok(connection)
}

fn open_connection(path: &Path) -> Result<Connection, StorageError> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(StorageError(format!(
            "{DATABASE_PATH_ENV} must be an absolute database file path: {}",
            path.display()
        )));
    }
    let connection = Connection::open(path).map_err(|error| {
        StorageError(format!("cannot open database {}: {error}", path.display()))
    })?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| StorageError(format!("cannot set database busy timeout: {error}")))?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(|error| StorageError(format!("cannot enable foreign keys: {error}")))?;
    Ok(connection)
}

fn apply_migrations(
    connection: &mut Connection,
    migrations: &[Migration],
) -> Result<(), StorageError> {
    for (index, migration) in migrations.iter().enumerate() {
        if migration.version != index as i64 + 1 {
            return Err(StorageError(format!(
                "migration {} ({}) is out of order; expected version {}",
                migration.version,
                migration.name,
                index + 1
            )));
        }
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| StorageError(format!("cannot begin schema migration: {error}")))?;
    transaction
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (\
             version INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, sql TEXT NOT NULL)",
        )
        .map_err(|error| StorageError(format!("cannot initialize migration history: {error}")))?;

    let applied = {
        let mut statement = transaction
            .prepare("SELECT version, name, sql FROM schema_migrations ORDER BY version")
            .map_err(|error| StorageError(format!("cannot read migration history: {error}")))?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>())
            .map_err(|error| StorageError(format!("cannot read migration history: {error}")))?
    };

    for (index, (version, name, sql)) in applied.iter().enumerate() {
        let expected = migrations.get(index).ok_or_else(|| {
            StorageError(format!(
                "database has unsupported migration {version} ({name})"
            ))
        })?;
        if *version != expected.version || name != expected.name || sql != expected.sql {
            return Err(StorageError(format!(
                "database migration history mismatch at {version} ({name}); expected {} ({})",
                expected.version, expected.name
            )));
        }
    }

    for migration in migrations.iter().skip(applied.len()) {
        transaction.execute_batch(migration.sql).map_err(|error| {
            StorageError(format!(
                "migration {} ({}) failed: {error}",
                migration.version, migration.name
            ))
        })?;
        transaction
            .execute(
                "INSERT INTO schema_migrations (version, name, sql) VALUES (?1, ?2, ?3)",
                params![migration.version, migration.name, migration.sql],
            )
            .map_err(|error| {
                StorageError(format!(
                    "cannot record migration {} ({}): {error}",
                    migration.version, migration.name
                ))
            })?;
    }
    transaction
        .commit()
        .map_err(|error| StorageError(format!("cannot commit schema migrations: {error}")))
}

#[cfg(test)]
mod tests {
    use super::{MIGRATIONS, Migration, apply_migrations, initialize, initialize_configured};
    use rusqlite::Connection;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    fn temporary_database() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "zeta-practice-storage-{}-{nonce}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).expect("create test directory");
        directory.join("practice.sqlite")
    }

    #[test]
    fn initializes_replays_and_preserves_data_after_restart() {
        let path = temporary_database();
        let connection = initialize_configured(Some(path.clone().into_os_string()))
            .expect("fresh initialization at configured path");
        connection
            .execute(
                "INSERT INTO storage_metadata (key, value) VALUES ('probe', 'retained')",
                [],
            )
            .expect("write probe");
        drop(connection);

        let connection = initialize_configured(Some(path.clone().into_os_string()))
            .expect("restart and migration replay at configured path");
        let value: String = connection
            .query_row(
                "SELECT value FROM storage_metadata WHERE key = 'probe'",
                [],
                |row| row.get(0),
            )
            .expect("read probe");
        assert_eq!(value, "retained");
        let count: i64 = connection
            .query_row("SELECT count(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("read history");
        assert_eq!(count, 2);
        drop(connection);
        fs::remove_file(&path).expect("remove test database");
        fs::remove_dir(path.parent().expect("test directory")).expect("remove test directory");
    }

    #[test]
    fn failed_migration_reports_version_and_rolls_back() {
        let mut connection = Connection::open_in_memory().expect("memory database");
        let failing = [
            Migration {
                version: 1,
                name: "storage_metadata",
                sql: MIGRATIONS[0].sql,
            },
            Migration {
                version: 2,
                name: "broken",
                sql: "CREATE TABLE transient (id INTEGER); INVALID SQL;",
            },
        ];
        let error = apply_migrations(&mut connection, &failing).expect_err("invalid SQL must fail");
        assert!(error.to_string().contains("migration 2 (broken) failed"));
        let table_count: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
                [],
                |row| row.get(0),
            )
            .expect("read tables");
        assert_eq!(table_count, 0);
        apply_migrations(&mut connection, MIGRATIONS).expect("retry valid migration");
    }

    #[test]
    fn unknown_history_is_rejected() {
        let mut connection = Connection::open_in_memory().expect("memory database");
        apply_migrations(&mut connection, MIGRATIONS).expect("initial migration");
        connection
            .execute(
                "UPDATE schema_migrations SET name = 'other' WHERE version = 1",
                [],
            )
            .expect("tamper history");
        let error = apply_migrations(&mut connection, MIGRATIONS).expect_err("mismatch must fail");
        assert!(error.to_string().contains("history mismatch"));
    }

    #[test]
    fn requires_absolute_file_path() {
        let error = initialize(PathBuf::from("relative.sqlite").as_path())
            .expect_err("relative path must fail");
        assert!(error.to_string().contains("absolute database file path"));
    }

    #[test]
    fn requires_database_path_configuration() {
        let error = initialize_configured(None).expect_err("missing path must fail");
        assert!(
            error
                .to_string()
                .contains("ZETA_PRACTICE_DB_PATH is required")
        );
    }
}
