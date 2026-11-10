namespace    = "alaydriem"
name         = "hotbar-manager"
display_name = "Hotbar Manager"
version      = "26.63.4"
author       = "Alaydriem"
config_path  = "config.hcl"

claim "Input" {
  mode   = "Shared"
  reason = "Reads mouse clicks on its buttons in inventory and container screens"
}

keybind "rotate_up" {
  label       = "Rotate rows up"
  default     = "Alt+D"
  capability  = "Input"
  scope       = "Global"
  config_path = "keys.rotate_up"
}

keybind "rotate_down" {
  label       = "Rotate rows down"
  default     = "Alt+A"
  capability  = "Input"
  scope       = "Global"
  config_path = "keys.rotate_down"
}

keybind "sort" {
  label       = "Sort open container"
  default     = "Alt+X"
  capability  = "Input"
  scope       = "Global"
  config_path = "keys.sort"
}

section "keys" {
  title = "Keys"

  field "keys.rotate_up" {
    label   = "Rotate rows up"
    help    = "Moves every hotbar row up one step"
    kind    = "chord"
    default = "Alt+D"
    apply   = "Live"
  }

  field "keys.rotate_down" {
    label   = "Rotate rows down"
    help    = "Moves every hotbar row down one step"
    kind    = "chord"
    default = "Alt+A"
    apply   = "Live"
  }

  field "keys.sort" {
    label   = "Sort open container"
    help    = "Sorts the container that is open"
    kind    = "chord"
    default = "Alt+X"
    apply   = "Live"
  }
}
