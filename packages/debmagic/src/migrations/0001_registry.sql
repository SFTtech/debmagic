-- SQLite has no enums: each enum column checks the values of the Rust
-- conversion named above it.

create table environments (
    -- Environment::id
    id text primary key,
    -- DriverType::as_str / DriverType::from_name
    driver text not null check (driver in ('docker', 'bare', 'lxd', 'incus')),
    package_name text not null,
    package_identifier text not null,
    source_dir text not null,
    root_dir text not null,
    -- Distro::as_str / Distro::parse, free-form for custom distros
    distro_family text not null,
    distro_codename text not null,
    distro_version text not null,
    distro_is_devel integer not null check (distro_is_devel in (0, 1)),
    -- Persistence::as_str / Persistence::from_name
    persistence text not null check (persistence in ('no', 'on-failure', 'always')),
    -- EnvironmentPurpose::as_str / EnvironmentPurpose::from_name
    purpose text not null check (purpose in ('build', 'test')),
    -- the command holding the Environment, see Process
    owner_pid integer,
    owner_pid_start integer,
    destroying integer not null default 0 check (destroying in (0, 1)),
    -- JSON object from Driver::driver_metadata
    driver_metadata text not null default '{}'
) strict;

-- `debmagic env shell` sessions, see Process
create table attachments (
    id integer primary key autoincrement,
    environment_id text not null references environments(id) on delete cascade,
    pid integer not null,
    pid_start integer
) strict;

create table invocations (
    id integer primary key autoincrement,
    -- InvocationKind::as_str / InvocationKind::from_name
    kind text not null check (kind in ('binary_build', 'source_build', 'test_run')),
    source_dir text not null,
    package_name text not null,
    package_version text not null,
    distro_family text not null,
    distro_codename text not null,
    distro_version text not null,
    distro_is_devel integer not null check (distro_is_devel in (0, 1)),
    driver text check (driver in ('docker', 'bare', 'lxd', 'incus')),
    success integer not null check (success in (0, 1)),
    changes_path text,
    environment_id text references environments(id) on delete set null,
    -- unix time
    created_at integer not null
) strict;
