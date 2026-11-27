namespace    = "alaydriem"
name         = "hotbar-manager"
entry        = "alaydriem_hotbar_manager.dll"
api_version  = 3

uses {
  feature "core" {
    event "client.tick" {
      version  = 1
      required = true
    }
    event "registry.provider_ready" {
      version  = 1
      required = true
    }
    event "registry.provider_gone" {
      version  = 1
      required = true
    }
    event "mods.loaded" {
      version  = 1
      required = true
    }
    area "client.read" {
      version  = 1
      required = true
    }
  }
  feature "buttons" {
    event "ui.screen_render" {
      version  = 1
      required = true
    }
    event "input.ui_pointer" {
      version  = 1
      required = true
    }
    area "ui.screen" {
      version  = 1
      required = true
    }
    area "ui.draw" {
      version  = 1
      required = true
    }
    area "ui.textures" {
      version  = 1
      required = true
    }
    event "ui.textures_reloaded" {
      version  = 1
      required = true
    }
  }
  feature "screen_actions" {
    event "ui.screen_render" {
      version  = 1
      required = true
    }
    event "ui.screen_changed" {
      version  = 1
      required = true
    }
    area "ui.screen" {
      version  = 2
      required = true
    }
    area "inventory.read" {
      version  = 3
      required = true
    }
    area "client.top_screen" {
      version  = 2
      required = true
    }
  }
  feature "rotation" {
    event "inventory.request_done" {
      version  = 1
      required = true
    }
    area "inventory.read" {
      version  = 3
      required = true
    }
    area "inventory.request" {
      version  = 1
      required = true
    }
    area "client.top_screen" {
      version  = 2
      required = true
    }
  }
}

listing {
  title       = "Hotbar Manager"
  summary     = "Rotate the hotbar rows and sort containers."
  description = <<-EOT
    Rotate the inventory rows through the hotbar and sort the open container, with vanilla-looking buttons.
  EOT
  authors     = ["Charles R. Portwood II"]
  license     = "BSD-3-Clause"
  source      = "https://github.com/alaydriem/amml-hotbar-manager"
  homepage    = "https://github.com/alaydriem/amml-hotbar-manager#readme"
  tags        = ["hotbar", "inventory", "sort", "client", "utility"]
  icon        = "icon.png"
}
