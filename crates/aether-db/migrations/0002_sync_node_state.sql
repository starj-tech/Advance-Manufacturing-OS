-- HLC generator persistence.
--
-- One row per local node. Persisting (wall_ms, logical) across restarts
-- protects against system-clock rollback: when the app reboots, the
-- generator must not emit a wall_ms below the last seen value, even if
-- the OS clock did go backwards (NTP correction, manual time-zone
-- change, suspend-resume drift).
--
-- The CHECK (id = 1) constraint enforces a singleton — every client
-- has exactly one HLC node identity for its lifetime; provisioning a
-- new node_id is a separate flow that resets the table.

CREATE TABLE IF NOT EXISTS sync_node_state (
    id        INTEGER PRIMARY KEY CHECK (id = 1),
    node_id   TEXT NOT NULL,
    wall_ms   INTEGER NOT NULL,
    logical   INTEGER NOT NULL
);
