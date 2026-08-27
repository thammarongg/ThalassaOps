locals {
  purpose = "thalassaops-sprint-10-fixture-capture"
}

provider "aws" {
  profile = var.profile
  region  = var.region

  default_tags {
    tags = {
      purpose = local.purpose
    }
  }
}

data "aws_availability_zones" "available" {
  state = "available"
}

data "aws_ssm_parameter" "al2023_arm64" {
  name = "/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-6.1-arm64"
}

resource "aws_vpc" "fixture" {
  cidr_block           = "10.42.0.0/16"
  enable_dns_hostnames = true
  enable_dns_support   = true

  tags = {
    Name = "thalassaops-s10-fixture-vpc"
  }
}

resource "aws_internet_gateway" "fixture" {
  vpc_id = aws_vpc.fixture.id

  tags = {
    Name = "thalassaops-s10-fixture-igw"
  }
}

resource "aws_subnet" "fixture_a" {
  availability_zone = data.aws_availability_zones.available.names[0]
  cidr_block        = "10.42.1.0/24"
  vpc_id            = aws_vpc.fixture.id

  tags = {
    Name = "thalassaops-s10-fixture-a"
  }
}

resource "aws_subnet" "fixture_b" {
  availability_zone = data.aws_availability_zones.available.names[1]
  cidr_block        = "10.42.2.0/24"
  vpc_id            = aws_vpc.fixture.id

  tags = {
    Name = "thalassaops-s10-fixture-b"
  }
}

resource "aws_route_table" "fixture" {
  vpc_id = aws_vpc.fixture.id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.fixture.id
  }

  tags = {
    Name = "thalassaops-s10-fixture-routes"
  }
}

resource "aws_route_table_association" "fixture_a" {
  route_table_id = aws_route_table.fixture.id
  subnet_id      = aws_subnet.fixture_a.id
}

resource "aws_route_table_association" "fixture_b" {
  route_table_id = aws_route_table.fixture.id
  subnet_id      = aws_subnet.fixture_b.id
}

resource "aws_iam_role" "eks_cluster" {
  name = "thalassaops-s10-fixture-eks-cluster"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "eks.amazonaws.com"
        }
      }
    ]
  })

  tags = {
    Name = "thalassaops-s10-fixture-eks-cluster"
  }
}

resource "aws_iam_role_policy_attachment" "eks_cluster" {
  policy_arn = "arn:aws:iam::aws:policy/AmazonEKSClusterPolicy"
  role       = aws_iam_role.eks_cluster.name
}

resource "aws_eks_cluster" "fixture" {
  name     = "thalassaops-s10-fixture-eks"
  role_arn = aws_iam_role.eks_cluster.arn

  vpc_config {
    endpoint_private_access = false
    endpoint_public_access  = true
    subnet_ids              = [aws_subnet.fixture_a.id, aws_subnet.fixture_b.id]
  }

  tags = {
    Name = "thalassaops-s10-fixture-eks"
  }

  depends_on = [aws_iam_role_policy_attachment.eks_cluster]
}

resource "aws_security_group" "instance" {
  description = "Egress-only security group for the fixture EC2 instance."
  name        = "thalassaops-s10-fixture-instance"
  vpc_id      = aws_vpc.fixture.id

  tags = {
    Name = "thalassaops-s10-fixture-instance"
  }
}

resource "aws_vpc_security_group_egress_rule" "instance" {
  cidr_ipv4         = "0.0.0.0/0"
  ip_protocol       = "-1"
  security_group_id = aws_security_group.instance.id
}

resource "aws_instance" "fixture" {
  ami                         = data.aws_ssm_parameter.al2023_arm64.value
  associate_public_ip_address = false
  instance_type               = "t4g.nano"
  subnet_id                   = aws_subnet.fixture_a.id
  vpc_security_group_ids      = [aws_security_group.instance.id]

  metadata_options {
    http_endpoint               = "enabled"
    http_put_response_hop_limit = 1
    http_tokens                 = "required"
  }

  tags = {
    Name = "thalassaops-s10-fixture-ec2"
  }
}
