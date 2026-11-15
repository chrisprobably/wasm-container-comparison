#!/usr/bin/env bash
# Creates the AWS infrastructure and deploys the Node API image.
# Needs: AWS credentials, Terraform and Docker.
set -euo pipefail
cd "$(dirname "$0")"

terraform -chdir=infra init -input=false

# Create the image repository first so there is somewhere to push to.
terraform -chdir=infra apply -target=aws_ecr_repository.api
REPO=$(terraform -chdir=infra output -raw ecr_repository_url)
REGION=$(terraform -chdir=infra output -raw region)

aws ecr get-login-password --region "$REGION" | docker login --username AWS --password-stdin "${REPO%%/*}"
docker build --platform linux/amd64 -f app/Dockerfile -t "$REPO:latest" ..
docker push "$REPO:latest"

terraform -chdir=infra apply
echo "API: $(terraform -chdir=infra output -raw api_url)"
