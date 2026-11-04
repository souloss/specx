# Security policy

Please report vulnerabilities privately through the repository's configured security reporting channel. Do not include credentials, tokens, connection strings, database dumps, or other private data in a public issue.

Include the affected specX commit, deployment mode, impacted component, reproduction steps, and the smallest useful evidence. The service host should be run behind a trusted TLS reverse proxy for public access, with a long `SPECX_PASSWORD` and `SPECX_SESSION_SECRET` supplied through the deployment environment.

The API derives tenant, workspace, and actor context from the authenticated session. Changes that cross that boundary need focused tests covering unauthorized, forbidden, and cross-workspace requests.
