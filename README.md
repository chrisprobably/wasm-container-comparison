# wasm-container-comparison

The same small booking API built two ways, so they can be compared on cost, startup time and latency:

| | `aws/` | `cloudflare/` |
|---|---|---|
| Runtime | Node 24 in a Docker container | Rust compiled to WebAssembly |
| Framework | Express 5 + Drizzle ORM | [`worker`](https://crates.io/crates/worker) crate + sea-query |
| Hosting | EC2 Auto Scaling Group (t3.medium, 2–4 instances) behind an ALB | Cloudflare Workers with Smart Placement |
| Database | RDS Postgres 17 (`db.t4g.micro`) in us-east-2 | The same RDS instance, through Hyperdrive |

Both versions use the same schema (`db/schema.sql`) and return identical responses, so the same scripts can test and benchmark either one.

## API

| Method | Path | Notes |
|---|---|---|
| `GET` | `/health` | Returns `ok` |
| `POST` | `/calendars` | Body: `{"name": "..."}` |
| `GET` | `/calendars/:id/bookings?from=&to=` | Bookings overlapping the time range |
| `POST` | `/calendars/:id/bookings` | Body: `{"title", "start", "end"}` (ISO 8601). `400` if end is not after start, `409` if the slot overlaps an existing booking |
| `GET` | `/bookings/:id` | |
| `DELETE` | `/bookings/:id` | |

## Layout

```
aws/
  app/          Node API and Dockerfile
  infra/        Terraform: VPC, ALB, Auto Scaling Group, RDS, ECR
  deploy.sh     Build and push the image, then terraform apply
cloudflare/
  worker/       Rust Worker (src/lib.rs) and wrangler.toml
  infra/        Terraform: Hyperdrive config
  deploy.sh     Create Hyperdrive, then wrangler deploy
bench/
  smoke.sh      Functional checks against either API
  latency.sh    First-request latency (new connection per request)
db/
  schema.sql    Shared schema
```

## Running the Worker locally

Needs [Rust](https://rustup.rs), Node.js and a local Postgres. The Worker compiles to WebAssembly, so Rust also needs the Wasm build target:

```bash
rustup target add wasm32-unknown-unknown   # one-time setup

createdb calendar
psql calendar -f db/schema.sql

cd cloudflare/worker
npm install
# Wrangler requires a password in the URL. A local Postgres that trusts
# localhost connections (the Homebrew default) ignores it, so any value works.
export CLOUDFLARE_HYPERDRIVE_LOCAL_CONNECTION_STRING_DB="postgres://$USER:password@localhost:5432/calendar"
npm run dev   # http://localhost:8787

# In another terminal
bench/smoke.sh http://localhost:8787
```

Local runs only check that the code behaves correctly. Latency numbers from a local run mean nothing.

## Deploying

Deploy AWS first, because the Cloudflare side reads the database host and password from the AWS Terraform outputs.

```bash
# Needs AWS credentials, Terraform and Docker
aws/deploy.sh

# Needs CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID
cloudflare/deploy.sh
```

`cloudflare/deploy.sh` writes your Hyperdrive ID into `cloudflare/worker/wrangler.toml`. Don't commit that change.

## Benchmarking

```bash
bench/smoke.sh <base-url>
bench/latency.sh "<base-url>/calendars/1/bookings?from=2026-11-01T00:00:00Z&to=2026-12-01T00:00:00Z" 50
```

Run `latency.sh` from where your users are, not from the cloud region. Smart Placement only moves the Worker near the database after it has seen some traffic, so send some requests first and note whether a measurement was taken before or after that happened.

## Caveats

- **Cost comparisons cover compute only.** Both versions use the same RDS instance, so the database costs the same either way.
- **The database is publicly reachable.** `allow_cloudflare_db_access` (default `true`) opens Postgres to [Cloudflare's IP ranges](https://www.cloudflare.com/ips/) so Hyperdrive can connect. The RDS instance has `publicly_accessible = true`, `deletion_protection = false` and no final snapshot. This setup is for benchmarking, not production.
- **The AWS side is a deliberately typical setup:** a single-stage `node:24` image and instances in public subnets. It isn't tuned to be as small or fast as possible.

## Tearing down

The ALB, EC2 instances and RDS cost money even when idle. Destroy the Cloudflare side first, because it depends on the AWS outputs.

```bash
terraform -chdir=cloudflare/infra destroy
terraform -chdir=aws/infra destroy
```

To delete the Worker itself, run `npx wrangler delete` in `cloudflare/worker`.

## License

[MIT](LICENSE)
