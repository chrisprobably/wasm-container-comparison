output "api_url" {
  value = "${var.certificate_arn == null ? "http" : "https"}://${aws_lb.app.dns_name}"
}

output "ecr_repository_url" {
  value = aws_ecr_repository.api.repository_url
}

output "region" {
  value = var.region
}

output "asg_name" {
  value = aws_autoscaling_group.app.name
}

output "target_group_arn" {
  value = aws_lb_target_group.app.arn
}

# Used by cloudflare/deploy.sh to configure Hyperdrive.
output "db_host" {
  value = aws_db_instance.main.address
}

output "db_password" {
  value     = random_password.db.result
  sensitive = true
}
