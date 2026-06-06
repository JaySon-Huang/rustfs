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
    mysql::Mysql,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use tokio::sync::Mutex;

const MYSQL_ROOT_PASSWORD: &str = "rustfs";
const MYSQL_DATABASE: &str = "rustfs_events";
const MYSQL_DSN_ENV: &str = "RUSTFS_TEST_MYSQL_DSN";

pub struct MysqlFixture {
    pub dsn: String,
    _container: Option<ContainerAsync<Mysql>>,
}

static FIXTURE: OnceLock<MysqlFixture> = OnceLock::new();
static INIT: Mutex<()> = Mutex::const_new(());

pub async fn shared_mysql_fixture() -> &'static MysqlFixture {
    if let Some(fixture) = FIXTURE.get() {
        return fixture;
    }

    let _guard = INIT.lock().await;
    if let Some(fixture) = FIXTURE.get() {
        return fixture;
    }

    let fixture = if let Ok(dsn) = std::env::var(MYSQL_DSN_ENV) {
        MysqlFixture { dsn, _container: None }
    } else {
        let container = Mysql::default()
            .with_env_var("MYSQL_ROOT_PASSWORD", MYSQL_ROOT_PASSWORD)
            .with_env_var("MYSQL_DATABASE", MYSQL_DATABASE)
            .start()
            .await
            .expect("start MySQL test container");
        let port = container.get_host_port_ipv4(3306).await.expect("resolve MySQL host port");
        let dsn = format!("root:{MYSQL_ROOT_PASSWORD}@tcp(127.0.0.1:{port})/{MYSQL_DATABASE}");
        MysqlFixture {
            dsn,
            _container: Some(container),
        }
    };

    let _ = FIXTURE.set(fixture);
    FIXTURE.get().expect("MySQL fixture should be initialized")
}
