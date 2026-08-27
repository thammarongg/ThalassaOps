locals {
  purpose = "thalassaops-sprint-10-fixture-capture"
  region  = join("-", slice(split("-", var.zone), 0, 2))
}

provider "google" {
  project = var.project
  region  = local.region
  zone    = var.zone
}

data "google_compute_image" "debian" {
  family  = "debian-12"
  project = "debian-cloud"
}

resource "google_compute_network" "fixture" {
  auto_create_subnetworks = false
  description             = "purpose=thalassaops-sprint-10-fixture-capture"
  name                    = "thalassaops-s10-fixture-network"
}

resource "google_compute_subnetwork" "fixture" {
  description   = "purpose=thalassaops-sprint-10-fixture-capture"
  ip_cidr_range = "10.42.0.0/24"
  name          = "thalassaops-s10-fixture-subnet"
  network       = google_compute_network.fixture.id
  region        = local.region
}

resource "google_container_cluster" "fixture" {
  initial_node_count       = 1
  location                 = var.zone
  name                     = "thalassaops-s10-fixture-gke"
  network                  = google_compute_network.fixture.name
  remove_default_node_pool = true
  subnetwork               = google_compute_subnetwork.fixture.name

  ip_allocation_policy {}

  resource_labels = {
    purpose = local.purpose
  }

  deletion_protection = false
}

resource "google_compute_instance" "fixture" {
  machine_type = "e2-micro"
  name         = "thalassaops-s10-fixture-compute"
  zone         = var.zone

  boot_disk {
    initialize_params {
      image = data.google_compute_image.debian.self_link
      size  = 10
      type  = "pd-standard"
    }
  }

  network_interface {
    subnetwork = google_compute_subnetwork.fixture.id
  }

  labels = {
    purpose = local.purpose
  }
}
