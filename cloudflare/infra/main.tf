terraform {
  required_version = ">= 1.6"

  required_providers {
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "~> 5.28"
    }
  }
}

# Reads CLOUDFLARE_API_TOKEN from the environment.
provider "cloudflare" {}

variable "account_id" {
  description = "Cloudflare account ID."
  type        = string
}

variable "db_host" {
  type = string
}

variable "db_port" {
  type    = number
  default = 5432
}

variable "db_name" {
  type    = string
  default = "calendar"
}

variable "db_user" {
  type    = string
  default = "calendar_admin"
}

variable "db_password" {
  type      = string
  sensitive = true
}

# Hyperdrive pools connections near the database and caches read queries.
resource "cloudflare_hyperdrive_config" "calendar" {
  account_id = var.account_id
  name       = "calendar-bench"

  origin = {
    scheme   = "postgres"
    host     = var.db_host
    port     = var.db_port
    database = var.db_name
    user     = var.db_user
    password = var.db_password
  }
}

output "hyperdrive_id" {
  value = cloudflare_hyperdrive_config.calendar.id
}
