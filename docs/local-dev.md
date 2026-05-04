# Local Development Reference

## Port Reference

| Port | Service |
|---|---|
| `localhost:8000` | DynamoDB Local (also used by `cargo test`) |
| `localhost:3000` | local-apigw proxy (internal) |
| `localhost:9000` | Caddy HTTP |
| `localhost:9001` | Caddy HTTPS (required for Safari WebAuthn) |

## Certificate Renewal

Caddy's locally-issued cert expires after a long break. To regenerate and re-trust:

```bash
docker compose down
docker volume rm mendicant_caddy_data mendicant_caddy_config
docker compose up -d

# Install the CA cert directly (caddy trust is disabled in Caddyfile):
docker compose cp caddy:/data/caddy/pki/authorities/local/root.crt ./caddy-root.crt

# Safari / Chrome (macOS keychain):
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain ./caddy-root.crt

# Firefox (has its own cert store — ignores macOS keychain):
# Settings → Privacy & Security → View Certificates → Authorities → Import
# Select caddy-root.crt, tick "Trust this CA to identify websites", OK.

rm ./caddy-root.crt
```

DynamoDB is in-memory so no data is lost. After trusting, reload `./scripts/setup-dynamodb-local.sh` then reload the browser.

## Environment Variables

| Variable | Effect |
|---|---|
| `DYNAMODB_ENDPOINT_URL=http://localhost:8000` | Points SDK at DynamoDB Local |
| `JWT_SIGNING_KEY_PATH=/path/to/dev-key.pem` | Uses local RSA key instead of KMS |

When absent, the code uses real AWS services. Both are set in `docker-compose.yml` for local development.
