variable "name" {
  description = "Prefix for resource names."
  type        = string
  default     = "calendar-bench"
}

variable "region" {
  type    = string
  default = "us-east-2"
}

variable "instance_type" {
  type    = string
  default = "t3.medium"
}

variable "min_size" {
  type    = number
  default = 2
}

variable "max_size" {
  type    = number
  default = 4
}

variable "certificate_arn" {
  description = "Optional ACM certificate ARN. When set, the ALB also listens on HTTPS (443)."
  type        = string
  default     = null
}

variable "db_instance_class" {
  type    = string
  default = "db.t4g.micro"
}

variable "allow_cloudflare_db_access" {
  description = "Allow Cloudflare's IP ranges to reach Postgres, so Hyperdrive can connect."
  type        = bool
  default     = true
}
