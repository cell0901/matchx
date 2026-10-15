# Exchange

A multithreaded exchange built in Rust, with a dedicated orderbook thread per market and separate services for the API, real-time feeds, and persistence.

The engine keeps matching and balance updates in memory, Redis carries commands and events between services, and Postgres stores executed trades and order state.

## Architecture
![architecture](./assets/architecture_rs.png)

## Services

`http:` is the REST API. It handles authentication, validates requests, and bridges request/reply messages through Redis. It runs on `localhost:8080`.

`engine:` is the multi-threaded matching service. Each market owns a dedicated orderbook thread; the service also handles balances, Redis command consumption, and event publishing.

`ws:` is the WebSocket gateway for trade and depth updates. It runs on `localhost:8081`.

`db-filler:` consumes Redis Stream events and persists trades and order state to Postgres.

## Local setup

Start Redis and Postgres:

```bash
docker compose up -d
```

Create `.env` files from the examples in each service directory and set the required values. The local Docker defaults are:

```env
REDIS_URL=redis://localhost:6379
DATABASE_URL=postgresql://postgres:postgrespass@localhost:5432/postgres
```

Run migrations before starting the services:

```bash
cd http/migration
DATABASE_URL=postgresql://postgres:postgrespass@localhost:5432/postgres cargo run -- up
```

Start each service in a separate terminal:

```bash
cd engine && cargo run
cd http && cargo run
cd ws && cargo run
cd db-filler && cargo run
```

## Data flow

```text
HTTP API ── Redis list / stream ──> Engine ── Redis Pub/Sub ──> HTTP reply / WebSocket gateway
                                      │
                                      └── Redis Stream: engine_events_stream ──> db-filler ──> Postgres
```

`engine_events_stream` is consumed with a Redis consumer group. The database writer acknowledges an event only after its Postgres write succeeds.

## Numeric values

The engine uses fixed-point `u64` values internally with a scale factor of `100_000_000`. Client requests use decimal strings such as `"1.25"`; API responses convert scaled price and quantity values back to decimal strings. This avoids floating-point rounding errors during matching and balance calculations.

Market orders may omit `price`; limit orders require it.

## Useful endpoints

- `POST /signup`, `POST /signin`
- `POST /balance/onramp`, `POST /balance/deposit`, `GET /balance`
- `POST /order`, `DELETE /cancel-order`, `GET /open-orders`
- `GET /depth`

Protected endpoints require the authentication middleware configured by the HTTP service.

## Engine throughput benchmark

Run from `engine/` with `cargo bench --bench engine_throughput`. No Redis or Postgres is needed.

The benchmark sends 1,000 matched limit-order pairs per active market and reports completed orders per second for one versus four active market threads. It pre-funds users before timing and counts an order only after the balance actor confirms settlement. Each pair produces one fill, so fills per second is half the reported orders per second for this workload. Redis, WebSocket, database publishing, and HTTP are excluded; this is actor-level engine throughput, not end-to-end exchange throughput.

In a local run, the benchmark measured approximately **37,000 completed orders/sec with one active market** and **256,000 completed orders/sec across four active markets**. Results vary with hardware and system load.

## HTTP load test

The k6 order-flow test measures the client-response path: HTTP → Redis → engine → HTTP. It reports accepted order throughput, client-observed latency, and failures. Database persistence and WebSocket delivery are outside its scope.

In a local two-minute run across four markets, k6 offered **200 matched pairs/s (~400 orders/s)**. All **48,002 orders** received the expected responses, with **zero failed HTTP requests** and **zero dropped iterations**. `/order` latency was **11.3 ms at p95** and **14.92 ms at p99**.

Before timing begins, the script creates and funds test users. It starts 200 iterations per second, cycling through the four markets. Each iteration submits a limit sell order, waits for its response, then submits a matching buy order. The run checks order acceptance, HTTP failures, dropped iterations, and response latency; p95 of 11.3 ms means 95% of `/order` requests completed within 11.3 ms at this offered load.
