#!/usr/bin/env bash
# Creates the Hyperdrive config, then builds and deploys the Rust Worker.
#
# Needs: CLOUDFLARE_API_TOKEN, CLOUDFLARE_ACCOUNT_ID, Rust with the
# wasm32-unknown-unknown target, Terraform, and Node.js.
#
# Database details come from the AWS Terraform outputs if they exist,
# otherwise from DB_HOST and DB_PASSWORD.
set -euo pipefail
cd "$(dirname "$0")"

: "${CLOUDFLARE_ACCOUNT_ID:?set CLOUDFLARE_ACCOUNT_ID}"
: "${CLOUDFLARE_API_TOKEN:?set CLOUDFLARE_API_TOKEN}"

if [[ -z "${DB_HOST:-}" ]]; then
  DB_HOST=$(terraform -chdir=../aws/infra output -raw db_host)
  DB_PASSWORD=$(terraform -chdir=../aws/infra output -raw db_password)
fi
: "${DB_PASSWORD:?set DB_PASSWORD}"

export TF_VAR_account_id="$CLOUDFLARE_ACCOUNT_ID"
export TF_VAR_db_host="$DB_HOST"
export TF_VAR_db_password="$DB_PASSWORD"

terraform -chdir=infra init -input=false
terraform -chdir=infra apply
HYPERDRIVE_ID=$(terraform -chdir=infra output -raw hyperdrive_id)

cd worker
sed -i.bak -E "s/^id = \".*\"/id = \"${HYPERDRIVE_ID}\"/" wrangler.toml && rm wrangler.toml.bak
npm install --no-audit --no-fund
npx wrangler deploy
