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

#![allow(unsafe_code)]

//! Registers integration-test containers for explicit removal on process exit.
//!
//! Shared fixtures keep containers alive in `OnceLock` until the test binary
//! exits. `ContainerAsync` drop can run after the tokio runtime shuts down, so
//! we remove containers from an `atexit` handler using the container CLI.

use std::{
    process::{Command, Stdio},
    sync::{Mutex, Once},
};

static CONTAINER_IDS: Mutex<Vec<String>> = Mutex::new(Vec::new());
static INSTALL_ATEXIT: Once = Once::new();

pub fn register_container(id: String) {
    CONTAINER_IDS.lock().expect("lock integration-test container ids").push(id);

    INSTALL_ATEXIT.call_once(|| {
        // SAFETY: `atexit_remove_containers` is a C ABI callback that only
        // reads process-global state and performs best-effort container removal.
        unsafe {
            libc::atexit(atexit_remove_containers);
        }
    });
}

extern "C" fn atexit_remove_containers() {
    let ids = match CONTAINER_IDS.lock() {
        Ok(mut guard) => std::mem::take(&mut *guard),
        Err(poisoned) => std::mem::take(&mut *poisoned.into_inner()),
    };

    for id in &ids {
        remove_container_sync(id);
    }
}

fn remove_container_sync(id: &str) {
    for runtime in ["podman", "docker"] {
        if Command::new(runtime)
            .args(["rm", "-f", id])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
        {
            return;
        }
    }
}
