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

use testcontainers_modules::{rabbitmq::RabbitMq, testcontainers::runners::AsyncRunner};
use tokio::sync::Mutex;

use super::cleanup;

const AMQP_URL_ENV: &str = "RUSTFS_TEST_AMQP_URL";

pub struct AmqpFixture {
    pub url: String,
}

static FIXTURE: OnceLock<AmqpFixture> = OnceLock::new();
static INIT: Mutex<()> = Mutex::const_new(());

pub async fn shared_amqp_fixture() -> &'static AmqpFixture {
    if let Some(fixture) = FIXTURE.get() {
        return fixture;
    }

    let _guard = INIT.lock().await;
    if let Some(fixture) = FIXTURE.get() {
        return fixture;
    }

    let fixture = if let Ok(url) = std::env::var(AMQP_URL_ENV) {
        AmqpFixture { url }
    } else {
        let container = RabbitMq::default().start().await.expect("start RabbitMQ test container");
        cleanup::register_container(container.id().to_string());
        let port = container.get_host_port_ipv4(5672).await.expect("resolve RabbitMQ host port");
        let url = format!("amqp://guest:guest@127.0.0.1:{port}/%2f");
        std::mem::forget(container);
        AmqpFixture { url }
    };

    let _ = FIXTURE.set(fixture);
    FIXTURE.get().expect("AMQP fixture should be initialized")
}
