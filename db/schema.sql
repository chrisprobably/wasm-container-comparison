-- Shared schema for both APIs. Safe to run more than once.

CREATE TABLE IF NOT EXISTS calendars (
  id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  name       text        NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS bookings (
  id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  calendar_id bigint      NOT NULL REFERENCES calendars (id) ON DELETE CASCADE,
  title       text        NOT NULL,
  start_at    timestamptz NOT NULL,
  end_at      timestamptz NOT NULL,
  created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS bookings_calendar_start ON bookings (calendar_id, start_at);
