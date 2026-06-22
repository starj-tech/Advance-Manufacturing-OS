-- V13: extend `actuator_commands.command_kind` CHECK to admit the
-- new 'write-tag' variant added to `aether_actuators::CommandKind`.
--
-- 0009 introduced the table with five Phase-1 kinds; 0010 added
-- 'scan' for V11; this migration adds 'write-tag' for V13's
-- controller (PLC/PAC/CNC/DCS) write surface.
--
-- Drop+re-add because Postgres can't ALTER CHECK in place. Safe
-- on a fresh deployment (the table itself just landed in 0009
-- and Phase-1+2 outbox entries haven't shipped to production).

ALTER TABLE actuator_commands
    DROP CONSTRAINT actuator_commands_command_kind_check;

ALTER TABLE actuator_commands
    ADD CONSTRAINT actuator_commands_command_kind_check
    CHECK (command_kind IN (
        'capture', 'move-joint', 'move-linear',
        'dispatch-job', 'halt',
        'scan',
        'write-tag'
    ));
