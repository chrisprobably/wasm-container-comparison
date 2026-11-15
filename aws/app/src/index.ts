import { readFile } from "node:fs/promises";
import { and, asc, eq, gt, lt } from "drizzle-orm";
import { drizzle } from "drizzle-orm/node-postgres";
import express, { type NextFunction, type Request, type Response } from "express";
import pg from "pg";
import { bookings, calendars } from "./schema.js";

const pool = new pg.Pool({
  connectionString: process.env.DATABASE_URL,
  ssl: process.env.PGSSLROOTCERT
    ? { ca: await readFile(process.env.PGSSLROOTCERT, "utf8") }
    : undefined,
  max: 10,
});
const db = drizzle({ client: pool });

async function applySchema() {
  const schema = await readFile(
    process.env.SCHEMA_PATH ?? new URL("../db/schema.sql", import.meta.url),
    "utf8",
  );
  const client = await pool.connect();
  try {
    // Both instances boot at the same time, so take a lock before running DDL.
    await client.query("BEGIN");
    await client.query("SELECT pg_advisory_xact_lock(4242)");
    await client.query(schema);
    await client.query("COMMIT");
  } catch (err) {
    await client.query("ROLLBACK");
    throw err;
  } finally {
    client.release();
  }
}

// --- Input parsing and response shapes (same as the Worker) ---

class BadRequest extends Error {}

function parseId(value: string): number {
  const id = Number(value);
  if (!Number.isSafeInteger(id)) throw new BadRequest("invalid id");
  return id;
}

function parseTime(value: unknown): Date {
  const time = typeof value === "string" ? new Date(value) : new Date(NaN);
  if (Number.isNaN(time.getTime())) throw new BadRequest("invalid timestamp");
  return time;
}

type Booking = typeof bookings.$inferSelect;

function bookingJson(b: Pick<Booking, "id" | "calendarId" | "title" | "startAt" | "endAt">) {
  return {
    id: b.id,
    calendar_id: b.calendarId,
    title: b.title,
    start: b.startAt.toISOString(),
    end: b.endAt.toISOString(),
  };
}

const bookingColumns = {
  id: bookings.id,
  calendarId: bookings.calendarId,
  title: bookings.title,
  startAt: bookings.startAt,
  endAt: bookings.endAt,
};

// --- Routes ---

const app = express();
app.use(express.json());

app.get("/health", (_req, res) => {
  res.type("text/plain").send("ok");
});

app.post("/calendars", async (req, res) => {
  const name = req.body?.name;
  if (typeof name !== "string" || name === "") throw new BadRequest("name is required");

  const [calendar] = await db.insert(calendars).values({ name }).returning();
  res.status(201).json({
    id: calendar.id,
    name: calendar.name,
    created_at: calendar.createdAt.toISOString(),
  });
});

app.get("/calendars/:id/bookings", async (req, res) => {
  const id = parseId(req.params.id);
  if (req.query.from === undefined || req.query.to === undefined) {
    throw new BadRequest("from and to query parameters are required");
  }
  const from = parseTime(req.query.from);
  const to = parseTime(req.query.to);

  const calendar = await db.select({ id: calendars.id }).from(calendars).where(eq(calendars.id, id));
  if (calendar.length === 0) return res.status(404).json({ error: "calendar not found" });

  const rows = await db
    .select(bookingColumns)
    .from(bookings)
    .where(and(eq(bookings.calendarId, id), lt(bookings.startAt, to), gt(bookings.endAt, from)))
    .orderBy(asc(bookings.startAt));
  res.json(rows.map(bookingJson));
});

app.post("/calendars/:id/bookings", async (req, res) => {
  const calendarId = parseId(req.params.id);
  const { title, start, end } = req.body ?? {};
  if (typeof title !== "string" || start === undefined || end === undefined) {
    throw new BadRequest("title, start and end are required");
  }
  const startAt = parseTime(start);
  const endAt = parseTime(end);
  if (endAt <= startAt) throw new BadRequest("end must be after start");

  const overlap = await db
    .select({ id: bookings.id })
    .from(bookings)
    .where(and(eq(bookings.calendarId, calendarId), lt(bookings.startAt, endAt), gt(bookings.endAt, startAt)))
    .limit(1);
  if (overlap.length > 0) return res.status(409).json({ error: "time slot already booked" });

  const [booking] = await db
    .insert(bookings)
    .values({ calendarId, title, startAt, endAt })
    .returning(bookingColumns);
  res.status(201).json(bookingJson(booking));
});

app.get("/bookings/:id", async (req, res) => {
  const id = parseId(req.params.id);
  const [booking] = await db.select(bookingColumns).from(bookings).where(eq(bookings.id, id));
  if (!booking) return res.status(404).json({ error: "booking not found" });
  res.json(bookingJson(booking));
});

app.delete("/bookings/:id", async (req, res) => {
  const id = parseId(req.params.id);
  const deleted = await db.delete(bookings).where(eq(bookings.id, id)).returning({ id: bookings.id });
  if (deleted.length === 0) return res.status(404).json({ error: "booking not found" });
  res.status(204).end();
});

// Express 5 passes errors thrown in async handlers to this handler.
app.use((err: unknown, _req: Request, res: Response, _next: NextFunction) => {
  if (err instanceof BadRequest) return res.status(400).json({ error: err.message });
  if ((err as { type?: string }).type === "entity.parse.failed") {
    return res.status(400).json({ error: "invalid JSON body" });
  }
  // Drizzle wraps driver errors; the Postgres error code is on `cause`.
  const code = (err as { cause?: { code?: string } }).cause?.code;
  if (code === "23503") return res.status(404).json({ error: "calendar not found" });
  console.error(err);
  res.status(500).json({ error: "internal error" });
});

await applySchema();
const port = Number(process.env.PORT ?? 3000);
app.listen(port, () => console.log(`calendar api listening on :${port}`));
