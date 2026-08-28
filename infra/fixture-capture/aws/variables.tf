variable "profile" {
  description = "AWS CLI profile Terraform uses for this throwaway fixture."
  type        = string
  nullable    = false

  validation {
    condition     = trimspace(var.profile) != ""
    error_message = "profile must not be blank."
  }
}

variable "region" {
  description = "AWS region in which to create the fixtures."
  type        = string
  nullable    = false

  validation {
    condition     = trimspace(var.region) != ""
    error_message = "region must not be blank."
  }
}
