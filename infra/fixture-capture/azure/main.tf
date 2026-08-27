locals {
  purpose = "thalassaops-sprint-10-fixture-capture"
}

provider "azurerm" {
  features {}

  subscription_id = var.subscription_id
  tenant_id       = var.tenant_id
}

resource "azurerm_resource_group" "fixture" {
  location = var.location
  name     = "thalassaops-s10-fixture-rg"

  tags = {
    purpose = local.purpose
  }
}

resource "azurerm_virtual_network" "fixture" {
  address_space       = ["10.42.0.0/16"]
  location            = var.location
  name                = "thalassaops-s10-fixture-vnet"
  resource_group_name = azurerm_resource_group.fixture.name

  tags = {
    purpose = local.purpose
  }
}

resource "azurerm_subnet" "fixture" {
  address_prefixes     = ["10.42.1.0/24"]
  name                 = "thalassaops-s10-fixture-subnet"
  resource_group_name  = azurerm_resource_group.fixture.name
  virtual_network_name = azurerm_virtual_network.fixture.name
}

resource "azurerm_kubernetes_cluster" "fixture" {
  dns_prefix          = "thalassaops-s10-fixture"
  location            = var.location
  name                = "thalassaops-s10-fixture-aks"
  resource_group_name = azurerm_resource_group.fixture.name

  default_node_pool {
    name           = "system"
    node_count     = 1
    vm_size        = var.aks_node_size
    vnet_subnet_id = azurerm_subnet.fixture.id
  }

  identity {
    type = "SystemAssigned"
  }

  linux_profile {
    admin_username = "fixtureadmin"

    ssh_key {
      key_data = var.ssh_public_key
    }
  }

  network_profile {
    load_balancer_sku = "standard"
    network_plugin    = "azure"
  }

  tags = {
    purpose = local.purpose
  }
}

resource "azurerm_network_interface" "fixture" {
  location            = var.location
  name                = "thalassaops-s10-fixture-nic"
  resource_group_name = azurerm_resource_group.fixture.name

  ip_configuration {
    name                          = "internal"
    private_ip_address_allocation = "Dynamic"
    subnet_id                     = azurerm_subnet.fixture.id
  }

  tags = {
    purpose = local.purpose
  }
}

resource "azurerm_linux_virtual_machine" "fixture" {
  admin_username                  = "fixtureadmin"
  disable_password_authentication = true
  location                        = var.location
  name                            = "thalassaops-s10-fixture-vm"
  network_interface_ids           = [azurerm_network_interface.fixture.id]
  resource_group_name             = azurerm_resource_group.fixture.name
  size                            = var.vm_size

  admin_ssh_key {
    public_key = var.ssh_public_key
    username   = "fixtureadmin"
  }

  os_disk {
    caching              = "ReadWrite"
    storage_account_type = "Standard_LRS"
  }

  source_image_reference {
    offer     = "0001-com-ubuntu-server-jammy"
    publisher = "Canonical"
    sku       = "22_04-lts-gen2"
    version   = "latest"
  }

  tags = {
    purpose = local.purpose
  }
}
