-- Persistence values in environments.persistent: 0 = no, 1 = always, 2 = on-failure.
-- Version 1 stored a boolean in this column (0 = no, 1 = always). Those rows already match.
UPDATE environments SET persistent = persistent;
