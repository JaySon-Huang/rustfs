// Copyright 2026 RustFS Team
//
// Each integration test binary includes only one fixture module; suppress
// dead-code warnings for the unused siblings in shared support code.
#![allow(dead_code, unused_imports)]
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

pub mod amqp;
pub mod mysql;
pub mod postgres;

pub use amqp::shared_amqp_fixture;
pub use mysql::shared_mysql_fixture;
pub use postgres::shared_postgres_fixture;
