-- Log in with EVE: the characters that logged in, their SSO tokens and their
-- sessions. The refresh token is a long-lived credential; it never leaves the
-- API and is never logged.
create table sso_characters (
    id bigint primary key,
    name text not null,
    -- Changes when the character changes hands; a new owner drops old sessions.
    owner_hash text not null,
    scopes text[] not null,
    access_token text not null,
    refresh_token text not null,
    access_expires_at timestamptz not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

-- A session is addressed by a random token in an HttpOnly cookie; only its
-- SHA-256 is stored, so a database dump yields no usable cookie.
create table sessions (
    token_hash text primary key,
    character_id bigint not null references sso_characters (id) on delete cascade,
    expires_at timestamptz not null,
    created_at timestamptz not null default now()
);

create index sessions_character_id on sessions (character_id);

-- ESI answers kept until they expire (ESI asks clients not to refetch
-- earlier), with their ETag for a cheap 304 afterwards.
create table esi_cache (
    key text primary key,
    body text not null,
    etag text,
    expires_at timestamptz not null
);
