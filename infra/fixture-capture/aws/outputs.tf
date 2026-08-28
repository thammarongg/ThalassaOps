output "eks_cluster_arn" {
  description = "ARN of the throwaway EKS cluster."
  value       = aws_eks_cluster.fixture.arn
}

output "eks_cluster_id" {
  description = "ID (name) of the throwaway EKS cluster."
  value       = aws_eks_cluster.fixture.id
}

output "eks_cluster_name" {
  description = "Name of the throwaway EKS cluster."
  value       = aws_eks_cluster.fixture.name
}

output "ec2_instance_id" {
  description = "ID of the throwaway EC2 instance."
  value       = aws_instance.fixture.id
}

output "ec2_instance_name" {
  description = "Name tag of the throwaway EC2 instance."
  value       = aws_instance.fixture.tags["Name"]
}

output "iam_role_name" {
  description = "Name of the EKS control-plane IAM role."
  value       = aws_iam_role.eks_cluster.name
}

output "subnet_ids" {
  description = "IDs of the two EKS subnets."
  value       = [aws_subnet.fixture_a.id, aws_subnet.fixture_b.id]
}

output "vpc_id" {
  description = "ID of the dedicated fixture VPC."
  value       = aws_vpc.fixture.id
}
