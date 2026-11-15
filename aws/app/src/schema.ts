// Drizzle definitions for the tables in db/schema.sql.
import { bigint, pgTable, text, timestamp } from "drizzle-orm/pg-core";

export const calendars = pgTable("calendars", {
  id: bigint("id", { mode: "number" }).primaryKey().generatedAlwaysAsIdentity(),
  name: text("name").notNull(),
  createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
});

export const bookings = pgTable("bookings", {
  id: bigint("id", { mode: "number" }).primaryKey().generatedAlwaysAsIdentity(),
  calendarId: bigint("calendar_id", { mode: "number" })
    .notNull()
    .references(() => calendars.id, { onDelete: "cascade" }),
  title: text("title").notNull(),
  startAt: timestamp("start_at", { withTimezone: true }).notNull(),
  endAt: timestamp("end_at", { withTimezone: true }).notNull(),
  createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
});
