-- Token of the Relay connection that last marked this host online.
-- A replica sets status back to offline only when the token still matches, so
-- one pod's disconnect does not clear a newer session owned by another pod.
-- The WebSocket itself stays on the pod that accepted it. Load balancers must
-- use ClientIP session affinity so one executor's long-lived connection sticks
-- to a single Relay pod. Tunnel bytes are not forwarded across pods.
ALTER TABLE hosts
    ADD COLUMN relay_connection_token UUID;
