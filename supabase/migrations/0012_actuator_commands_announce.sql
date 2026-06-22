-- V14: extend `actuator_commands.command_kind` CHECK to admit
-- the new 'announce' variant added to
-- `aether_actuators::CommandKind` for HMI surfaces.
--
-- 0009 introduced the table with five Phase-1 kinds; 0010 added
-- 'scan' for V11; 0011 added 'write-tag' for V13; this migration
-- adds 'announce' for V14's HMI write surface.
--
-- Drop+re-add because Postgres can't ALTER CHECK in place. Safe
-- on a fresh deployment.

ALTER TABLE actuator_commands
    DROP CONSTRAINT actuator_commands_command_kind_check;

ALTER TABLE actuator_commands
    ADD CONSTRAINT actuator_commands_command_kind_check
    CHECK (command_kind IN (
        'capture', 'move-joint', 'move-linear',
        'dispatch-job', 'halt',
        'scan',
        'write-tag',
        'announce'
    ));
