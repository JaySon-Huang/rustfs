// Copyright 2026 RustFS Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::sync::OnceLock;

use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ImageExt, runners::AsyncRunner},
};
use tokio::sync::Mutex;

use super::cleanup;

const POSTGRES_PASSWORD: &str = "rustfs";
const POSTGRES_DATABASE: &str = "rustfs_events";
const POSTGRES_DSN_ENV: &str = "RUSTFS_TEST_PG_DSN";

pub struct PostgresFixture {
    pub dsn: String,
}

static FIXTURE: OnceLock<PostgresFixture> = OnceLock::new();
static INIT: Mutex<()> = Mutex::const_new(());

pub async fn shared_postgres_fixture() -> &'static PostgresFixture {
    if let Some(fixture) = FIXTURE.get() {
        return fixture;
    }

    let _guard = INIT.lock().await;
    if let Some(fixture) = FIXTURE.get() {
        return fixture;
    }

    let fixture = if let Ok(dsn) = std::env::var(POSTGRES_DSN_ENV) {
        PostgresFixture { dsn }
    } else {
        let container = Postgres::default()
            .with_env_var("POSTGRES_PASSWORD", POSTGRES_PASSWORD)
            .with_env_var("POSTGRES_DB", POSTGRES_DATABASE)
            .start()
            .await
            .expect("start PostgreSQL test container");
        cleanup::register_container(container.id().to_string());
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("resolve PostgreSQL host port");
        let dsn = format!("postgres://postgres:{POSTGRES_PASSWORD}@127.0.0.1:{port}/{POSTGRES_DATABASE}");
        std::mem::forget(container);
        PostgresFixture { dsn }
    };

    let _ = FIXTURE.set(fixture);
    FIXTURE.get().expect("PostgreSQL fixture should be initialized")
}
