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

variable "resource_group_suffix" {
  description = "Optional suffix appended to the fixture resource group name; use a fresh value when recapturing soon after a destroy to avoid ARM name-propagation races."
  type        = string
  default     = ""
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
  description = "AKS system node pool VM size. In this subscription the DASv5 family has zero quota and the original B-series v1 sizes are refused; standardDalv6Family has quota available, so Standard_D2als_v6 is the working default."
  type        = string
  default     = "Standard_D2als_v6"
}

variable "vm_size" {
  description = "Standalone virtual machine size. In this subscription the DASv5 family has zero quota and the original B-series v1 sizes are refused; standardDalv6Family has quota available, so Standard_D2als_v6 is the working default."
  type        = string
  default     = "Standard_D2als_v6"
}
