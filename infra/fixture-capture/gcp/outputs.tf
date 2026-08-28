output "gke_cluster_id" {
  description = "ID of the throwaway GKE cluster."
  value       = google_container_cluster.fixture.id
}

output "gke_cluster_name" {
  description = "Name of the throwaway GKE cluster."
  value       = google_container_cluster.fixture.name
}

output "compute_instance_id" {
  description = "ID of the throwaway Compute Engine instance."
  value       = google_compute_instance.fixture.id
}

output "compute_instance_name" {
  description = "Name of the throwaway Compute Engine instance."
  value       = google_compute_instance.fixture.name
}

output "network_id" {
  description = "ID of the dedicated fixture VPC network."
  value       = google_compute_network.fixture.id
}

output "subnetwork_id" {
  description = "ID of the dedicated fixture subnet."
  value       = google_compute_subnetwork.fixture.id
}
