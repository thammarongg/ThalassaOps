output "aks_cluster_id" {
  description = "ID of the throwaway AKS cluster."
  value       = azurerm_kubernetes_cluster.fixture.id
}

output "aks_cluster_name" {
  description = "Name of the throwaway AKS cluster."
  value       = azurerm_kubernetes_cluster.fixture.name
}

output "resource_group_id" {
  description = "ID of the single resource group containing the fixtures."
  value       = azurerm_resource_group.fixture.id
}

output "resource_group_name" {
  description = "Name of the single resource group containing the fixtures."
  value       = azurerm_resource_group.fixture.name
}

output "virtual_machine_id" {
  description = "ID of the throwaway Standard_D2als_v6 virtual machine."
  value       = azurerm_linux_virtual_machine.fixture.id
}

output "virtual_machine_name" {
  description = "Name of the throwaway virtual machine."
  value       = azurerm_linux_virtual_machine.fixture.name
}
