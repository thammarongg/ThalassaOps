# Sprint 10 fixture capture

These are three independent Terraform roots for creating short-lived cloud
resources whose authenticated API responses become the Sprint 10 mapper
fixtures. They are deliberately separate so an unavailable provider
credential cannot block the other roots.

Every resource is tagged or labelled with
`purpose=thalassaops-sprint-10-fixture-capture`. The resources exist only to
capture fixtures and must be destroyed the same day. Never commit a `*.tfvars`
file, state file, credential, or other real account value.

Terraform 1.6 or newer is required. Each root uses local state by default;
keep the state file private and remove it after the corresponding destroy.
Run `terraform init` and `terraform validate` in each root before applying.

## AWS

Required variables:

- `profile` — the existing AWS CLI profile to use.
- `region` — the AWS region.

The root creates a dedicated VPC, internet gateway, two subnets in distinct
availability zones, the EKS control-plane role, an EKS cluster with no managed
node group, and one no-public-IP `t4g.nano` EC2 instance.

```sh
cd infra/fixture-capture/aws
terraform init
terraform validate
terraform apply \
  -var='profile=REPLACE_WITH_PROFILE' \
  -var='region=REPLACE_WITH_REGION'
terraform output
terraform destroy \
  -var='profile=REPLACE_WITH_PROFILE' \
  -var='region=REPLACE_WITH_REGION'
```

Approximate time is 15–25 minutes to create and 10–20 minutes to destroy.
Approximate cost while running is US$0.12–0.20 per hour, mostly the EKS
control-plane charge and the tiny EC2 instance plus disk; actual pricing
depends on region and current provider rates.

## GCP

Required variables:

- `project` — the GCP project.
- `zone` — the zonal location for GKE and Compute Engine.

The root creates a dedicated VPC and subnet, a zonal GKE cluster with its
default node pool removed, and one no-external-IP `e2-micro` instance.

```sh
cd infra/fixture-capture/gcp
terraform init
terraform validate
terraform apply \
  -var='project=REPLACE_WITH_PROJECT' \
  -var='zone=REPLACE_WITH_ZONE'
terraform output
terraform destroy \
  -var='project=REPLACE_WITH_PROJECT' \
  -var='zone=REPLACE_WITH_ZONE'
```

Approximate time is 10–20 minutes to create and 5–15 minutes to destroy.
Approximate cost while running is US$0.10–0.20 per hour, mostly GKE cluster
management and the instance disk; an eligible e2-micro free tier may reduce
the compute portion, and actual pricing depends on region and current rates.

## Azure

Required variables:

- `subscription_id` — the Azure subscription.
- `tenant_id` — the Azure tenant used for login.
- `location` — the Azure region.
- `ssh_public_key` — a public SSH key for the Linux AKS node and VM (the
  corresponding private key stays outside this repository).
- `resource_group_suffix` — optional text appended only to the resource-group
  name; leave it empty for the first run and use a fresh value for a quick
  recapture after destroy.

The root creates one resource group, a VNet and subnet, one AKS cluster with a
single `Standard_D2als_v6` node, and one no-public-IP `Standard_D2als_v6`
virtual machine. In this subscription the DASv5 family has zero quota and the
original B-series v1 sizes are refused, while `standardDalv6Family` has quota
available.

```sh
cd infra/fixture-capture/azure
terraform init
terraform validate
terraform apply \
  -var='subscription_id=REPLACE_WITH_SUBSCRIPTION_ID' \
  -var='tenant_id=REPLACE_WITH_TENANT_ID' \
  -var='location=REPLACE_WITH_LOCATION' \
  -var='ssh_public_key=REPLACE_WITH_PUBLIC_KEY'
terraform output
terraform destroy \
  -var='subscription_id=REPLACE_WITH_SUBSCRIPTION_ID' \
  -var='tenant_id=REPLACE_WITH_TENANT_ID' \
  -var='location=REPLACE_WITH_LOCATION' \
  -var='ssh_public_key=REPLACE_WITH_PUBLIC_KEY'
```

When recapturing soon after a destroy, pass a fresh suffix (for example
`-var='resource_group_suffix=-capture-20260828-01'`) to both the apply and
destroy commands. Azure Resource Manager can briefly retain the prior
resource-group name while its child resources settle, which can surface a
transient VNet 404; a fresh suffix avoids that same-name propagation race.
The suffix changes only the resource-group name, so the VNet, subnet, AKS,
NIC, and VM names remain stable.

Approximate time is 10–20 minutes to create and 5–10 minutes to destroy.
Approximate cost while running is US$0.08–0.20 per hour, mostly the AKS node,
the `Standard_D2als_v6` VM, and their disks; actual pricing depends on region,
subscription offer, and current provider rates.

Destroy each root immediately after the six response captures are complete:

```sh
terraform plan -destroy   # review the full deletion set first
terraform destroy          # do not use -auto-approve
```
