# Exchange

A Rust exchange with separate HTTP, matching-engine, WebSocket, and database-writer services. The matching engine runs a dedicated synchronous orderbook thread for each market.

The engine keeps matching and balance updates in memory, Redis carries commands and events between services, and Postgres stores executed trades and order state.

## Services

| Service | Purpose |
| --- | --- |
| `http` | REST API, authentication, request validation, and Redis request/reply bridge. Runs on `localhost:8080`. |
| `engine` | Multi-threaded, per-market orderbook matching, balance handling, Redis command consumers, and event publishing. |
| `ws` | WebSocket gateway for trade and depth updates. Runs on `localhost:8081`. |
| `db-filler` | Redis Streams consumer that persists trade and order events to Postgres. |

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

## TODO

- Add engine-only benchmarks for order matching, settlement, and depth updates.
- Add end-to-end benchmarks covering HTTP, Redis, engine, and WebSocket publication.
