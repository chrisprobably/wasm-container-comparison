use chrono::{DateTime, SecondsFormat, Utc};
use sea_query::{Expr, ExprTrait, Iden, Order, PostgresQueryBuilder, Query};
use sea_query_postgres::PostgresBinder;
use serde::{Deserialize, Serialize};
use tokio_postgres::{error::SqlState, Client, Row};
use worker::{postgres_tls::PassthroughTls, *};

// --- Tables (see db/schema.sql) ---

#[derive(Iden)]
enum Calendars {
    Table,
    Id,
    Name,
    CreatedAt,
}

#[derive(Iden)]
enum Bookings {
    Table,
    Id,
    CalendarId,
    Title,
    StartAt,
    EndAt,
}

fn booking_columns() -> [Bookings; 5] {
    [Bookings::Id, Bookings::CalendarId, Bookings::Title, Bookings::StartAt, Bookings::EndAt]
}

// --- Request and response bodies ---

#[derive(Deserialize)]
struct NewCalendar {
    name: Option<String>,
}

#[derive(Deserialize)]
struct NewBooking {
    title: Option<String>,
    start: Option<String>,
    end: Option<String>,
}

#[derive(Serialize)]
struct CalendarJson {
    id: i64,
    name: String,
    created_at: String,
}

#[derive(Serialize)]
struct BookingJson {
    id: i64,
    calendar_id: i64,
    title: String,
    start: String,
    end: String,
}

impl From<&Row> for CalendarJson {
    fn from(row: &Row) -> Self {
        CalendarJson { id: row.get("id"), name: row.get("name"), created_at: iso(row.get("created_at")) }
    }
}

impl From<&Row> for BookingJson {
    fn from(row: &Row) -> Self {
        BookingJson {
            id: row.get("id"),
            calendar_id: row.get("calendar_id"),
            title: row.get("title"),
            start: iso(row.get("start_at")),
            end: iso(row.get("end_at")),
        }
    }
}

// --- Routes ---

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .get("/health", |_, _| Response::ok("ok"))
        .post_async("/calendars", |req, ctx| async move {
            respond(create_calendar(req, ctx.env).await)
        })
        .get_async("/calendars/:id/bookings", |req, ctx| async move {
            let id = param(&ctx, "id");
            respond(list_bookings(req, ctx.env, id).await)
        })
        .post_async("/calendars/:id/bookings", |req, ctx| async move {
            let id = param(&ctx, "id");
            respond(create_booking(req, ctx.env, id).await)
        })
        .get_async("/bookings/:id", |_, ctx| async move {
            let id = param(&ctx, "id");
            respond(get_booking(ctx.env, id).await)
        })
        .delete_async("/bookings/:id", |_, ctx| async move {
            let id = param(&ctx, "id");
            respond(delete_booking(ctx.env, id).await)
        })
        .run(req, env)
        .await
}

// --- Handlers ---

async fn create_calendar(mut req: Request, env: Env) -> ApiResult {
    let body: NewCalendar = req.json().await.map_err(|_| bad_request("invalid JSON body"))?;
    let name = body.name.filter(|n| !n.is_empty()).ok_or(bad_request("name is required"))?;

    let (sql, values) = Query::insert()
        .into_table(Calendars::Table)
        .columns([Calendars::Name])
        .values_panic([name.into()])
        .returning(Query::returning().columns([Calendars::Id, Calendars::Name, Calendars::CreatedAt]))
        .build_postgres(PostgresQueryBuilder);

    let db = connect(&env).await?;
    let row = db.query_one(&sql, &values.as_params()).await?;
    json(201, &CalendarJson::from(&row))
}

async fn list_bookings(req: Request, env: Env, id: String) -> ApiResult {
    let id = parse_id(&id)?;
    let url = req.url()?;
    let query = |key: &str| url.query_pairs().find(|(k, _)| k == key).map(|(_, v)| v.into_owned());
    let (from, to) = match (query("from"), query("to")) {
        (Some(from), Some(to)) => (parse_time(&from)?, parse_time(&to)?),
        _ => return Err(bad_request("from and to query parameters are required")),
    };

    let db = connect(&env).await?;
    if !calendar_exists(&db, id).await? {
        return Err(not_found("calendar not found"));
    }

    let (sql, values) = Query::select()
        .columns(booking_columns())
        .from(Bookings::Table)
        .and_where(Expr::col(Bookings::CalendarId).eq(id))
        .and_where(Expr::col(Bookings::StartAt).lt(to))
        .and_where(Expr::col(Bookings::EndAt).gt(from))
        .order_by(Bookings::StartAt, Order::Asc)
        .build_postgres(PostgresQueryBuilder);

    let rows = db.query(&sql, &values.as_params()).await?;
    json(200, &rows.iter().map(BookingJson::from).collect::<Vec<_>>())
}

async fn create_booking(mut req: Request, env: Env, id: String) -> ApiResult {
    let calendar_id = parse_id(&id)?;
    let body: NewBooking = req.json().await.map_err(|_| bad_request("invalid JSON body"))?;
    let (title, start, end) = match (body.title, body.start, body.end) {
        (Some(title), Some(start), Some(end)) => (title, parse_time(&start)?, parse_time(&end)?),
        _ => return Err(bad_request("title, start and end are required")),
    };
    if end <= start {
        return Err(bad_request("end must be after start"));
    }

    let db = connect(&env).await?;

    let (sql, values) = Query::select()
        .column(Bookings::Id)
        .from(Bookings::Table)
        .and_where(Expr::col(Bookings::CalendarId).eq(calendar_id))
        .and_where(Expr::col(Bookings::StartAt).lt(end))
        .and_where(Expr::col(Bookings::EndAt).gt(start))
        .limit(1)
        .build_postgres(PostgresQueryBuilder);
    if db.query_opt(&sql, &values.as_params()).await?.is_some() {
        return Err(ApiError::Status(409, "time slot already booked"));
    }

    let (sql, values) = Query::insert()
        .into_table(Bookings::Table)
        .columns([Bookings::CalendarId, Bookings::Title, Bookings::StartAt, Bookings::EndAt])
        .values_panic([calendar_id.into(), title.into(), start.into(), end.into()])
        .returning(Query::returning().columns(booking_columns()))
        .build_postgres(PostgresQueryBuilder);

    let row = db.query_one(&sql, &values.as_params()).await?;
    json(201, &BookingJson::from(&row))
}

async fn get_booking(env: Env, id: String) -> ApiResult {
    let id = parse_id(&id)?;
    let (sql, values) = Query::select()
        .columns(booking_columns())
        .from(Bookings::Table)
        .and_where(Expr::col(Bookings::Id).eq(id))
        .build_postgres(PostgresQueryBuilder);

    let db = connect(&env).await?;
    let row = db
        .query_opt(&sql, &values.as_params())
        .await?
        .ok_or(not_found("booking not found"))?;
    json(200, &BookingJson::from(&row))
}

async fn delete_booking(env: Env, id: String) -> ApiResult {
    let id = parse_id(&id)?;
    let (sql, values) = Query::delete()
        .from_table(Bookings::Table)
        .and_where(Expr::col(Bookings::Id).eq(id))
        .returning_col(Bookings::Id)
        .build_postgres(PostgresQueryBuilder);

    let db = connect(&env).await?;
    if db.query_opt(&sql, &values.as_params()).await?.is_none() {
        return Err(not_found("booking not found"));
    }
    Ok(Response::empty()?.with_status(204))
}

async fn calendar_exists(db: &Client, id: i64) -> std::result::Result<bool, ApiError> {
    let (sql, values) = Query::select()
        .column(Calendars::Id)
        .from(Calendars::Table)
        .and_where(Expr::col(Calendars::Id).eq(id))
        .build_postgres(PostgresQueryBuilder);
    Ok(db.query_opt(&sql, &values.as_params()).await?.is_some())
}

// --- Database ---

// Hyperdrive keeps the real connections to Postgres pooled near the database,
// so opening a connection per request is the intended pattern.
async fn connect(env: &Env) -> std::result::Result<Client, ApiError> {
    let hyperdrive = env.hyperdrive("DB")?;
    let socket = Socket::builder()
        .secure_transport(SecureTransport::StartTls)
        .connect(hyperdrive.host(), hyperdrive.port())?;
    let config = hyperdrive.connection_string().parse::<tokio_postgres::Config>()?;
    let (client, connection) = config.connect_raw(socket, PassthroughTls).await?;

    wasm_bindgen_futures::spawn_local(async move {
        if let Err(error) = connection.await {
            console_error!("database connection error: {error}");
        }
    });
    Ok(client)
}

// --- Parsing, responses and errors ---

enum ApiError {
    Status(u16, &'static str),
    Db(tokio_postgres::Error),
    Worker(worker::Error),
}

impl From<tokio_postgres::Error> for ApiError {
    fn from(error: tokio_postgres::Error) -> Self {
        ApiError::Db(error)
    }
}

impl From<worker::Error> for ApiError {
    fn from(error: worker::Error) -> Self {
        ApiError::Worker(error)
    }
}

type ApiResult = std::result::Result<Response, ApiError>;

fn bad_request(message: &'static str) -> ApiError {
    ApiError::Status(400, message)
}

fn not_found(message: &'static str) -> ApiError {
    ApiError::Status(404, message)
}

fn param(ctx: &RouteContext<()>, name: &str) -> String {
    ctx.param(name).cloned().unwrap_or_default()
}

fn parse_id(value: &str) -> std::result::Result<i64, ApiError> {
    value.parse().map_err(|_| bad_request("invalid id"))
}

fn parse_time(value: &str) -> std::result::Result<DateTime<Utc>, ApiError> {
    DateTime::parse_from_rfc3339(value)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| bad_request("invalid timestamp"))
}

// Same format as JavaScript's Date.toISOString(), used by the AWS API.
fn iso(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn json<T: Serialize>(status: u16, body: &T) -> ApiResult {
    Ok(Response::from_json(body)?.with_status(status))
}

fn error_json(status: u16, message: &str) -> Result<Response> {
    Ok(Response::from_json(&serde_json::json!({ "error": message }))?.with_status(status))
}

// Map errors to the same status codes the AWS API returns.
fn respond(result: ApiResult) -> Result<Response> {
    match result {
        Ok(response) => Ok(response),
        Err(ApiError::Status(status, message)) => error_json(status, message),
        Err(ApiError::Db(error)) if error.code() == Some(&SqlState::FOREIGN_KEY_VIOLATION) => {
            error_json(404, "calendar not found")
        }
        Err(ApiError::Db(error)) => {
            console_error!("database error: {error}");
            error_json(500, "internal error")
        }
        Err(ApiError::Worker(error)) => {
            console_error!("worker error: {error}");
            error_json(500, "internal error")
        }
    }
}
