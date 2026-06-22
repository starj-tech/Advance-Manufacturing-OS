-- V11: extend `actuator_commands.command_kind` CHECK to admit the
-- new 'scan' variant added to `aether_actuators::CommandKind`.
--
-- The original CHECK in `0009_actuator_commands.sql` hard-coded the
-- five Phase-1 kinds (capture, move-joint, move-linear, dispatch-
-- job, halt). Phase-2 V11 (Auto-ID) adds 'scan'. Postgres can't
-- ALTER CHECK in place, so drop+re-add is the migration path.
--
-- The drop is safe because the constraint is enforced row-by-row
-- (not used as an index); no rows exist yet for any tenant (the
-- table itself was just introduced in 0009 and Phase-1 outbox
-- entries haven't shipped to production). The re-add re-validates
-- whatever data exists, which on a fresh deployment is empty.

ALTER TABLE actuator_commands
    DROP CONSTRAINT actuator_commands_command_kind_check;

ALTER TABLE actuator_commands
    ADD CONSTRAINT actuator_commands_command_kind_check
    CHECK (command_kind IN (
        'capture', 'move-joint', 'move-linear',
        'dispatch-job', 'halt',
        'scan'
    ));
