CREATE TABLE host_links (
    host_id UUID PRIMARY KEY REFERENCES hosts(id) ON DELETE CASCADE,
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    host_name TEXT,
    client_id UUID NOT NULL,
    client_name TEXT,
    public_key_b64 TEXT NOT NULL,
    private_key_jwk JSONB NOT NULL,
    server_public_key_b64 TEXT NOT NULL,
    signing_session_id UUID,
    paired_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_host_links_owner_user_id ON host_links(owner_user_id);
