# Modbus TCP Telemetry Logger & Industrial Gateway

[![Rust](https://img.shields.io/badge/Language-Rust_2021-orange.svg)](https://www.rust-lang.org/) [![Framework](https://img.shields.io/badge/Web-Axum-blue.svg)](https://github.com/tokio-rs/axum) [![Async](https://img.shields.io/badge/Runtime-Tokio-lightgrey.svg)](https://tokio.rs/) [![Database](https://img.shields.io/badge/Database-MySQL-blue.svg)](https://www.mysql.com/)

An async Modbus TCP telemetry logger, data hub, and REST/WebSocket API gateway built for IIoT, industrial automation, and SCADA integration. Written in **Rust**.

> 🌍 **Multi-language ready.** The backend and API responses are being wired for i18n (see [Localization](#-localization) below) so operator UIs can ship in the client's native language, not just English/Ukrainian.

---

## Why this matters (TL;DR for non-engineers)

- **No data loss during reconfiguration** — hardware/tag configs hot-reload without stopping the service.
- **Self-healing polling** — link errors and timeouts are detected and recovered automatically; timeout tuning cut timeout counts by an order of magnitude in production use.
- **Low DB load** — deduplication + batched writes mean the database only stores what actually changed.
- **Instant dashboards** — recent history (up to ~22 days per tag) is served from RAM, not the database, so charts and reports load immediately.
- **Role-based access** — Admin / SuperUser / User / Service levels with JWT auth, so operators and integrators get exactly the access they need.

---

## Architecture & Code Structure

[#architecture--code-structure](#architecture--code-structure)

The application runs on the `tokio` async runtime. Subsystems are decoupled and communicate through typed async `mpsc` channels rather than calling each other directly.

### Central message router (`/messages`)

All subsystems route their operations through a single event bus using one message type, `MainMsg` (`main_msg.rs`):

1. **`Request`** — one-shot request/response pipeline. REST API endpoints send a request together with a `oneshot::channel` for the reply. The router forwards it to the database or the in-memory cache, which handles it and replies directly to the HTTP handler.
2. **`Command`** — modification commands (e.g. adding a Modbus device or register). After validation and persistence to MySQL, the router emits a `ConfigEventType` signal to notify polling workers.
3. **`Event`** — real-time events (poll status, device disconnects, TCP context resets). These are broadcast live to connected WebSocket clients and logged to disk.
4. **`ConfigEventType`** — hot-reload signal. When device configuration changes, the router targets the specific polling actors and forces them to reload their config on the fly, without stopping neighboring workers or restarting the whole application.

---

## Key Features & Modules

[#key-features--modules](#key-features--modules)

### 1. Adaptive Modbus polling (`/reader` & `/modbus_device`)

- **`node_master`** — top-level actor managing polling nodes. Receives signals from the database router and dynamically spawns new polling tasks or forwards signals to existing ones.
- **`node_loop`** — per-line manager actor (`reader_loop/read_loop.rs`). Manages the `ReadMaster` lifecycle, handles timeouts, and processes worker update/removal signals.
- **`ReadMaster`** (`reader_loop/read_master_struct.rs`) — the polling core, built on `tokio-modbus`.
  * **"Most-lagging-first" scheduler** — devices with the oldest timestamp are polled first.
  * **Adaptive timeout & self-recovery** — automatically handles flaky links and `HeaderMismatch` frame errors from `tokio-modbus`. On error, the device's timeout is dynamically increased by **+50 ms** (not persisted to the database) and the TCP context is force-reset to restore a clean Modbus connection state. In production this cut timeout counts by roughly an order of magnitude.
  * **Live config reload** — re-reads device configuration from the DB via a `oneshot` request without stopping the service. Trade-off: this resets the current polling queue — a deliberate simplicity-over-completeness choice.
- **Decoding plugins** (`/decoding_plugins`) — fast enum-based data converters (`plugin_loader.rs`, `value_interface.rs`) supporting decimal-point shifting (`ComaShift`) and bit masking within a word (`BitInWord`). Custom plugins can be added for vendor-specific decoding schemes (e.g. `SatecDoubleRegistersInt32`). An enum was chosen over `dyn Trait` for simplicity; the trade-off is a recompile when adding a new plugin — a rare event in practice.

### 2. Smart telemetry storage & caching (`/db`)

- **Automatic initialization** (`states.rs`, `db_init`) — creates the database schema, required tables, and a default admin account on first run. Connection config is stored encrypted in `configs/db.toml`. The app only needs a DB login/password; it provisions everything else itself.
- **Async DB router** (`worker/db_master.rs`) — handles API requests and spawns DB tasks, replying via `oneshot` channels. Tasks are timeout-guarded; values have their own dedicated sub-task.
- **Deduplication & caching** (`/hasher`) — managed by the `HashMaster` actor (`hash_master.rs`) and the `ValueHasher` module:
  * **Deduplication** — suppresses writing unchanged values to the database (sends a heartbeat record once every 5 minutes if a value is stable). On disconnect, waits 2 minutes before marking data loss, avoiding DB spam.
  * **In-memory ring buffer** — holds up to **131,072 records per tag** in RAM (~22 days of history at a 15-second poll interval). Chart/report queries are served instantly from RAM, falling back to MySQL (`db_measure_unit.rs`) only outside the buffer window. Buffer size is a compile-time setting.
  * **Batched writes** (`db_measure_unit.rs`) — groups measurements into batched SQL inserts (batch size ≥ 20) to reduce storage wear and boost write throughput.

### 3. Access control & logical grouping

- **Physical & logical hierarchy**:
  * **Physical layer**: Modbus gateways (`node_router`) → Devices (`devices_router`) → Registers (`values_router`).
  * **Logical layer**: Shops/Areas (`user_group_router`) → Units/Devices (`user_subgroup_router`), linked via `assign_router`, enabling convenient operator dashboards. Useful when, say, one Modbus RTU line spans several shop floors and devices need to be grouped logically rather than physically. Subgroups hold values — devices spanning multiple areas, or multiple devices belonging to one area, can both be grouped by value.
- **RBAC** (`auth`) — supports `Admin`, `SuperUser`, `User`, and `Service` roles with JWT auth (15 min access / 30 day refresh tokens).
- **Real-time WebSockets** (`live_router`) — streams live tag values and equipment status. Connections auto-disconnect every 15 minutes to refresh the token, which also helps clean up stale/"hung" sockets. Data is subscription-based.

### 4. Node protection & hardware binding (`/minimal_copy_safe`)

- **Basic license protection** — on startup, the app checks `configs/license.key` against the local Windows drive identifier. If the identifier can't be retrieved, the check doesn't block startup.

### 5. File logging system (`/logger`)

- **Non-blocking log writes** — to avoid contending with the database under load, application logs are written asynchronously to the filesystem.
- **Structured storage** — logs are organized chronologically under `logs/`, split by month and daily files (`logs/YYYY-MM/DD.log`).

---

## 🌍 Localization

[#-localization](#-localization)

> **Status: in progress.** The core is already structured for this — the work is threading shared strings/config through `Arc` and refactoring outward-facing text into a translation layer. Estimated a couple of evenings of focused refactoring.

Planned approach:

- **Backend/API messages** — error messages, event descriptions, and validation responses moved behind a locale-aware lookup, selected per request (e.g. `Accept-Language` header or user profile setting), so integrators consuming the REST/WebSocket API get responses in their own language.
- **Shared state via `Arc`** — locale tables and translation maps wrapped in `Arc<...>` and shared across polling/DB/WebSocket actors without per-message cloning overhead, consistent with the existing actor/channel architecture.
- **UI layer** — the browser-served UI reads the same locale tables, so operator dashboards can ship in the client's language rather than only English/Ukrainian — useful when deploying to plants with non-English-speaking floor staff.
- **No breaking changes intended** — default locale stays backward-compatible with existing deployments; new locales are additive.

If you need a specific language prioritized for a project, mention it — it's straightforward to add once the translation layer lands.

---

## 🛠 Tech Stack

[#-tech-stack](#-tech-stack)

- **Core**: Rust, async runtime `tokio`
- **Networking & Web**: `axum`, `WebSocket`, `jsonwebtoken`, `tower-http`
- **Industrial protocols**: `tokio-modbus`
- **Database**: MySQL (`sqlx`)
- **Security**: JWT (JSON Web Tokens), hardware-ID license binding

---

## Quick Start

[#quick-start](#quick-start)

**1. Clone the repository**

```
git clone https://github.com/Seraphim1990/ModbusTCPLogger.git
cd ModbusTCPLogger
```

**2. Prepare the database**

Create a database and user in MySQL (or provide root access for automatic setup on first run).

**3. Run the application**

```
cargo run --release
```

On first launch, the app will prompt for DB connection details, generate `configs/db.toml`, create the table structure, and check the local license file.

Server settings are configured in `configs/server.toml`. The UI is served over the browser.

---

## Need adaptation for specific hardware or a custom SCADA interface?

- Custom Modbus RTU/TCP and non-standard protocol development & integration
- Integration with existing SCADA and industrial systems
- SCADA / industrial automation GUI development in Qt/QML

**Contact:**

- **GitHub**: [@Seraphim1990](https://github.com/Seraphim1990)
- **E-mail**: zhenia.koshelnik@gmail.com
