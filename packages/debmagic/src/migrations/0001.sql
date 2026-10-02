CREATE TABLE environments (
    id TEXT PRIMARY KEY,
    driver TEXT NOT NULL,
    package_name TEXT NOT NULL,
    package_identifier TEXT NOT NULL,
    source_dir TEXT NOT NULL,
    root_dir TEXT NOT NULL,
    distro_family TEXT NOT NULL,
    distro_codename TEXT NOT NULL,
    distro_version TEXT NOT NULL,
    distro_is_devel INTEGER NOT NULL,
    persistent INTEGER NOT NULL,
    purpose TEXT NOT NULL,
    owner_pid INTEGER,
    owner_pid_start INTEGER,
    destroying INTEGER NOT NULL DEFAULT 0,
    driver_metadata TEXT NOT NULL DEFAULT '{}'
);
CREATE TABLE attachments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    environment_id TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    pid INTEGER NOT NULL,
    pid_start INTEGER
);
CREATE TABLE invocations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    source_dir TEXT NOT NULL,
    package_name TEXT NOT NULL,
    package_version TEXT NOT NULL,
    distro_family TEXT NOT NULL,
    distro_codename TEXT NOT NULL,
    distro_version TEXT NOT NULL,
    distro_is_devel INTEGER NOT NULL,
    driver TEXT,
    success INTEGER NOT NULL,
    changes_path TEXT,
    environment_id TEXT,
    created_at INTEGER NOT NULL
);
