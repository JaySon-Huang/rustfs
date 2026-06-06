// Copyright 2024 RustFS Team
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

//! Integration tests for the AMQP notification target.
//!
//! Requires Docker or Podman. These tests are gated behind the `integration-tests`
//! feature and are not run by default CI. To run locally:
//!
//! ```bash
//! cargo test -p rustfs-targets --test amqp_integration --features integration-tests
//! ```
//!
//! Override the broker URL with `RUSTFS_TEST_AMQP_URL` to use an external
//! RabbitMQ-compatible broker instead of starting a container.

mod support;

use lapin::{
    BasicProperties, Connection, ConnectionProperties,
    options::{BasicAckOptions, BasicGetOptions, QueueBindOptions, QueueDeclareOptions, QueueDeleteOptions},
    types::FieldTable,
};
use rustfs_s3_types::EventName;
use rustfs_targets::Target;
use rustfs_targets::check_amqp_broker_available;
use rustfs_targets::target::EntityTarget;
use rustfs_targets::target::TargetType;
use rustfs_targets::target::amqp::{AMQPArgs, AMQPTarget};
use serde_json::Value;
use std::sync::Arc;
use support::shared_amqp_fixture;
use uuid::Uuid;

fn test_args(url: &str, routing_key: &str) -> AMQPArgs {
    AMQPArgs {
        enable: true,
        url: url.parse().expect("valid AMQP URL"),
        exchange: "amq.topic".to_string(),
        routing_key: routing_key.to_string(),
        mandatory: true,
        persistent: true,
        username: String::new(),
        password: String::new(),
        tls_ca: String::new(),
        tls_client_cert: String::new(),
        tls_client_key: String::new(),
        queue_dir: String::new(),
        queue_limit: 100_000,
        target_type: TargetType::NotifyEvent,
    }
}

fn entity_for(bucket: &str, object: &str) -> Arc<EntityTarget<serde_json::Value>> {
    Arc::new(EntityTarget {
        bucket_name: bucket.to_string(),
        object_name: object.to_string(),
        event_name: EventName::ObjectCreatedPut,
        data: serde_json::json!({"bucket": bucket, "object": object}),
    })
}

async fn bind_queue(url: &str, queue: &str, routing_key: &str) -> lapin::Channel {
    let conn = Connection::connect(url, ConnectionProperties::default())
        .await
        .expect("connect to AMQP broker");
    let channel = conn.create_channel().await.expect("create channel");
    channel
        .queue_declare(
            queue.into(),
            QueueDeclareOptions {
                durable: false,
                exclusive: true,
                auto_delete: true,
                ..QueueDeclareOptions::default()
            },
            FieldTable::default(),
        )
        .await
        .expect("declare queue");
    channel
        .queue_bind(
            queue.into(),
            "amq.topic".into(),
            routing_key.into(),
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await
        .expect("bind queue");
    channel
}

async fn read_one(channel: &lapin::Channel, queue: &str) -> (Value, BasicProperties) {
    let msg = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Some(msg) = channel
                .basic_get(queue.into(), BasicGetOptions::default())
                .await
                .expect("basic_get")
            {
                msg.ack(BasicAckOptions::default()).await.expect("ack message");
                break msg;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("message should arrive");

    let properties = msg.properties.clone();
    let payload = serde_json::from_slice(&msg.data).expect("message payload should be JSON");
    (payload, properties)
}

#[tokio::test]
async fn test_check_amqp_broker_available() {
    let url = shared_amqp_fixture().await.url.clone();
    check_amqp_broker_available(&test_args(&url, "rustfs.check"))
        .await
        .expect("broker check should succeed");
}

#[tokio::test]
async fn test_direct_publish_delivers_json_payload() {
    let url = shared_amqp_fixture().await.url.clone();
    let routing_key = format!("rustfs.test.{}", Uuid::new_v4().simple());
    let queue = format!("rustfs-test-{}", Uuid::new_v4().simple());
    let channel = bind_queue(&url, &queue, &routing_key).await;
    let target = AMQPTarget::new("direct".to_string(), test_args(&url, &routing_key)).expect("construct AMQP target");

    target
        .save(entity_for("bucket1", "object-A"))
        .await
        .expect("publish should succeed");

    let (payload, properties) = read_one(&channel, &queue).await;
    assert_eq!(payload["Key"], "bucket1/object-A");
    assert_eq!(payload["Records"][0]["data"]["bucket"], "bucket1");
    assert_eq!(properties.content_type().as_ref().map(|s| s.as_str()), Some("application/json"));
    assert_eq!(*properties.delivery_mode(), Some(2));

    channel
        .queue_delete(queue.into(), QueueDeleteOptions::default())
        .await
        .expect("delete queue");
}

#[tokio::test]
async fn test_publish_reconnects_after_close() {
    let url = shared_amqp_fixture().await.url.clone();
    let routing_key = format!("rustfs.reconnect.{}", Uuid::new_v4().simple());
    let queue = format!("rustfs-test-{}", Uuid::new_v4().simple());
    let channel = bind_queue(&url, &queue, &routing_key).await;
    let target = AMQPTarget::new("reconnect".to_string(), test_args(&url, &routing_key)).expect("construct AMQP target");

    target
        .save(entity_for("bucket1", "object-before-close"))
        .await
        .expect("initial publish should succeed");
    let (payload, _) = read_one(&channel, &queue).await;
    assert_eq!(payload["Key"], "bucket1/object-before-close");

    target.close().await.expect("close cached AMQP connection");

    target
        .save(entity_for("bucket1", "object-after-close"))
        .await
        .expect("publish should reconnect after close");
    let (payload, _) = read_one(&channel, &queue).await;
    assert_eq!(payload["Key"], "bucket1/object-after-close");

    channel
        .queue_delete(queue.into(), QueueDeleteOptions::default())
        .await
        .expect("delete queue");
}

#[tokio::test]
async fn test_queue_replay_delivers_and_removes_stored_payload() {
    let url = shared_amqp_fixture().await.url.clone();
    let routing_key = format!("rustfs.replay.{}", Uuid::new_v4().simple());
    let queue = format!("rustfs-test-{}", Uuid::new_v4().simple());
    let channel = bind_queue(&url, &queue, &routing_key).await;
    let queue_dir = std::env::temp_dir().join(format!("rustfs-amqp-integration-{}", Uuid::new_v4()));
    let mut args = test_args(&url, &routing_key);
    args.queue_dir = queue_dir.to_string_lossy().to_string();
    let target = AMQPTarget::new("queued".to_string(), args.clone()).expect("construct AMQP target");

    target
        .save(entity_for("bucket1", "object-B"))
        .await
        .expect("store-backed save should queue");
    assert_eq!(target.delivery_snapshot().queue_length, 1);

    let key = target.store().expect("store configured").list()[0].clone();
    target.send_from_store(key).await.expect("replay should publish and delete");

    let (payload, properties) = read_one(&channel, &queue).await;
    assert_eq!(payload["Key"], "bucket1/object-B");
    assert_eq!(properties.content_type().as_ref().map(|s| s.as_str()), Some("application/json"));
    assert_eq!(*properties.delivery_mode(), Some(2));
    assert_eq!(target.delivery_snapshot().queue_length, 0);

    channel
        .queue_delete(queue.into(), QueueDeleteOptions::default())
        .await
        .expect("delete queue");
    let _ = std::fs::remove_dir_all(args.queue_dir);
}
