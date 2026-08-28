variable "project" {
  description = "GCP project in which to create the fixtures."
  type        = string
  nullable    = false

  validation {
    condition     = trimspace(var.project) != ""
    error_message = "project must not be blank."
  }
}

variable "zone" {
  description = "GCP zone in which to create the zonal fixtures."
  type        = string
  nullable    = false

  validation {
    condition     = trimspace(var.zone) != ""
    error_message = "zone must not be blank."
  }
}
