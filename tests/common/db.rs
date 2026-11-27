//! Real-PostgreSQL test databases (ADR-011).
//!
//! `TestDb::new()` gives each test its own database, copied from a template that
//! has every migration applied. The database is dropped when the `TestDb` is
//! dropped, also when the test panics.
//!
//! Needs `TEST_DATABASE_URL`: a server URL whose path names any existing database,
//! such as `postgres://usuario:password@localhost:5432/postgres`.

use std::time::Duration;

use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, DbErr, Statement,
};
use tokio::sync::OnceCell;
use uuid::Uuid;

use eventManagement_api::migration::{Migrator, MigratorTrait};

/// Environment variable naming the PostgreSQL server for tests.
pub const TEST_DATABASE_URL: &str = "TEST_DATABASE_URL";

/// Prefix of every database the harness creates, to find leftovers by hand.
pub const TEST_DB_PREFIX: &str = "event_test_";

/// Advisory lock id that serializes template builds across test binaries.
const TEMPLATE_LOCK_ID: i64 = 0x5445_4d50_4c41_5445; // "TEMPLATE"

/// SQLSTATE `object_in_use`: the template is being accessed by another session.
const SQLSTATE_OBJECT_IN_USE: &str = "55006";

static TEMPLATE: OnceCell<String> = OnceCell::const_new();

/// Reads the admin URL from a variable lookup, or explains how to provide it.
pub fn admin_url_from(lookup: impl Fn(&str) -> Option<String>) -> Result<String, String> {
    match lookup(TEST_DATABASE_URL) {
        Some(url) if !url.trim().is_empty() => Ok(url),
        _ => Err(format!(
            "{TEST_DATABASE_URL} is not set. Tests need a PostgreSQL server (ADR-011): run \
             `docker compose up -d event-db` and set {TEST_DATABASE_URL}, for example \
             {TEST_DATABASE_URL}=postgres://usuario:password@localhost:5432/postgres"
        )),
    }
}

/// The admin URL from the environment (and `.env`), or a panic with instructions.
pub fn admin_url() -> String {
    dotenvy::dotenv().ok();
    admin_url_from(|name| std::env::var(name).ok()).unwrap_or_else(|message| panic!("{message}"))
}

/// Replaces the database name in a PostgreSQL URL, keeping any query string.
pub fn url_for_database(server_url: &str, database: &str) -> String {
    let (base, query) = match server_url.split_once('?') {
        Some((base, query)) => (base, Some(query)),
        None => (server_url, None),
    };
    let prefix = match base.rsplit_once('/') {
        // `postgres://host` has no path: the last '/' belongs to the scheme.
        Some((prefix, _)) if !prefix.ends_with('/') => prefix,
        _ => base,
    };
    match query {
        Some(query) => format!("{prefix}/{database}?{query}"),
        None => format!("{prefix}/{database}"),
    }
}

/// A connection to the admin database; `max_connections(1)` keeps every
/// statement on one session, which advisory locks need.
pub async fn admin() -> DatabaseConnection {
    let mut options = ConnectOptions::new(admin_url());
    options.max_connections(1).sqlx_logging(false);
    Database::connect(options)
        .await
        .expect("cannot connect to TEST_DATABASE_URL")
}

/// Whether a database with this name exists on the test server.
pub async fn database_exists(admin: &DatabaseConnection, name: &str) -> bool {
    admin
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT 1 AS found FROM pg_database WHERE datname = $1",
            [name.into()],
        ))
        .await
        .expect("cannot query pg_database")
        .is_some()
}

async fn execute(conn: &DatabaseConnection, sql: &str) -> Result<(), DbErr> {
    conn.execute_unprepared(sql).await.map(|_| ())
}

fn quote(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

/// Stable name for the template of the current migration set (FNV-1a over the
/// migration names), so adding a migration builds a new template.
fn template_name() -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for migration in Migrator::migrations() {
        for byte in migration.name().bytes().chain(std::iter::once(0)) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{TEST_DB_PREFIX}tpl_{hash:016x}")
}

async fn build_template_if_missing() -> String {
    let name = template_name();
    let admin = admin().await;
    execute(
        &admin,
        &format!("SELECT pg_advisory_lock({TEMPLATE_LOCK_ID})"),
    )
    .await
    .expect("cannot take the template lock");

    if !database_exists(&admin, &name).await {
        let building = format!("{name}_building");
        let server_url = admin_url();
        execute(
            &admin,
            &format!("DROP DATABASE IF EXISTS {} WITH (FORCE)", quote(&building)),
        )
        .await
        .expect("cannot drop a stale template build");
        execute(&admin, &format!("CREATE DATABASE {}", quote(&building)))
            .await
            .expect("cannot create the template database");

        let template_conn = Database::connect(url_for_database(&server_url, &building))
            .await
            .expect("cannot connect to the template database");
        Migrator::up(&template_conn, None)
            .await
            .expect("migrations failed on the template database");
        template_conn
            .close()
            .await
            .expect("cannot close the template connection");

        execute(
            &admin,
            &format!(
                "ALTER DATABASE {} RENAME TO {}",
                quote(&building),
                quote(&name)
            ),
        )
        .await
        .expect("cannot rename the template database");
        // Nobody may connect to the template, so copies never meet an active session.
        execute(
            &admin,
            &format!(
                "ALTER DATABASE {} WITH ALLOW_CONNECTIONS false",
                quote(&name)
            ),
        )
        .await
        .expect("cannot close the template to connections");
    }

    execute(
        &admin,
        &format!("SELECT pg_advisory_unlock({TEMPLATE_LOCK_ID})"),
    )
    .await
    .expect("cannot release the template lock");
    admin.close().await.ok();
    name
}

fn is_object_in_use(err: &DbErr) -> bool {
    err.to_string().contains(SQLSTATE_OBJECT_IN_USE)
        || err.to_string().contains("is being accessed by other users")
}

/// A database of its own for one test, with every migration applied.
pub struct TestDb {
    pub conn: DatabaseConnection,
    name: String,
    url: String,
    server_url: String,
}

impl TestDb {
    pub async fn new() -> TestDb {
        let template = TEMPLATE.get_or_init(build_template_if_missing).await;
        let server_url = admin_url();
        let name = format!("{TEST_DB_PREFIX}{}", Uuid::new_v4().simple());

        let admin = admin().await;
        let create = format!(
            "CREATE DATABASE {} TEMPLATE {}",
            quote(&name),
            quote(template)
        );
        let mut attempt = 0;
        loop {
            match execute(&admin, &create).await {
                Ok(()) => break,
                Err(err) if is_object_in_use(&err) && attempt < 20 => {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(err) => panic!("cannot create test database {name}: {err}"),
            }
        }
        admin.close().await.ok();

        let url = url_for_database(&server_url, &name);
        let mut options = ConnectOptions::new(url.clone());
        options.max_connections(5).sqlx_logging(false);
        let conn = Database::connect(options)
            .await
            .unwrap_or_else(|err| panic!("cannot connect to test database {name}: {err}"));

        TestDb {
            conn,
            name,
            url,
            server_url,
        }
    }

    /// Database name, for assertions about cleanup.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Connection URL of this test's database, for code that connects by itself.
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        // Async drop does not exist: drop the database from a short-lived thread with
        // its own runtime. This also runs while a failing test unwinds.
        let name = std::mem::take(&mut self.name);
        let server_url = std::mem::take(&mut self.server_url);
        let conn = std::mem::replace(&mut self.conn, DatabaseConnection::Disconnected);
        drop(conn);

        let result = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("cannot build cleanup runtime");
            runtime.block_on(async move {
                let mut options = ConnectOptions::new(server_url);
                options.max_connections(1).sqlx_logging(false);
                let admin = Database::connect(options)
                    .await
                    .map_err(|err| format!("{name}: {err}"))?;
                let dropped = execute(
                    &admin,
                    &format!("DROP DATABASE IF EXISTS {} WITH (FORCE)", quote(&name)),
                )
                .await;
                admin.close().await.ok();
                dropped.map_err(|err| format!("{name}: {err}"))
            })
        })
        .join();

        // Never panic in Drop: a second panic during unwinding aborts the test binary.
        match result {
            Ok(Ok(())) => {}
            Ok(Err(message)) => eprintln!("warning: test database not dropped: {message}"),
            Err(_) => eprintln!("warning: test database cleanup thread panicked"),
        }
    }
}
