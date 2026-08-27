variable "subscription_id" {
  description = "Azure subscription in which to create the fixtures."
  type        = string
  nullable    = false

  validation {
    condition     = trimspace(var.subscription_id) != ""
    error_message = "subscription_id must not be blank."
  }
}

variable "tenant_id" {
  description = "Azure tenant used for authentication."
  type        = string
  nullable    = false

  validation {
    condition     = trimspace(var.tenant_id) != ""
    error_message = "tenant_id must not be blank."
  }
}

variable "location" {
  description = "Azure region in which to create the fixtures."
  type        = string
  nullable    = false

  validation {
    condition     = trimspace(var.location) != ""
    error_message = "location must not be blank."
  }
}

variable "ssh_public_key" {
  description = "SSH public key used for the Linux AKS node and VM."
  type        = string
  nullable    = false

  validation {
    condition     = trimspace(var.ssh_public_key) != ""
    error_message = "ssh_public_key must not be blank."
  }
}

variable "aks_node_size" {
  description = "AKS system node pool VM size. Subscriptions with restricted quota may not permit B-series; check with 'az vm list-skus'."
  type        = string
  default     = "Standard_DC2s_v3"
}

variable "vm_size" {
  description = "Standalone virtual machine size. Subscriptions with restricted quota may not permit B-series; check with 'az vm list-skus'."
  type        = string
  default     = "Standard_DC1s_v3"
}
