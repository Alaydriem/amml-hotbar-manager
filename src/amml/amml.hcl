namespace    = "alaydriem"
name         = "hotbar-manager"
entry        = "alaydriem_hotbar_manager.dll"
api_version  = 2
amml_version = "26.62.0"
entries = [
  "ClientInstance.update",
  # the item moves: the answers to item stack requests, the packet path the requests are sent through, and the
  # player-slot prediction (entry names as stratum's amml.hcl)
  "LegacyClientNetworkHandler.vtable",
  "ClientNetworkHandler.handle.ItemStackResponsePacket",
  "MinecraftPackets.createPacket",
  "Packet.read",
  "ReadOnlyBinaryStream.vtable",
  "ReadOnlyBinaryStream.dtor",
  "ClientInstance.getPacketSender",
  "PacketSender.send",
  "ClientInstance.getLocalPlayer",
  "Player.playerInventory",
  "FillingContainer.swapSlots",
  # optional, the player-slot prediction: no recipe yet; while they are missing, player moves wait for the server
  "PlayerInventory.mInventory",
  "FillingContainer.mItems",
  # the screen detector: the root control name and the UI profile (recipes in the hotbar worktree)
  "ScreenView.mVisualTree",
  "VisualTree.mRootControl",
  "UIControl.mName",
  "ClientInstance.getOptions",
  "IOptionRegistry.getUIProfile",
  # the slot reader: `ItemStackBase.getMaxStackSize` has a recipe; ItemStack.vtable has one too; ItemStack.mNetIdVariant and CompoundTag.write have none yet (the reader is off without the net id entry)
  "ItemStackBase.getMaxStackSize",
  "ItemStack.mNetIdVariant",
  "ItemStack.vtable",
  "CompoundTag.write",
  # the buttons: the UI button funnel and the view render (both hooked), the draw calls and the GUI scale, the panel
  # walk and the view's pointer (recipes in the hotbar worktree)
  "ScreenView.handleButtonEvent",
  "ScreenView.render",
  "MinecraftUIRenderContext.mScreenContext",
  "MinecraftUIRenderContext.drawImage",
  "MinecraftUIRenderContext.flushImages",
  "MinecraftUIRenderContext.getTexture",
  "ScreenContext.guiData",
  "GuiData.getGuiScale",
  "UIControl.mChildren",
  "UIControl.mCachedPosition",
  "UIControl.mCachedPositionDirty",
  "UIControl.mCachedPositionDirtyMask",
  "UIControl.mIsVisibleInTree",
  "UIControl.mIsVisibleInTreeMask",
  "UIControl.mSize",
  "UIControl._setCachedPosition",
  "ScreenView.mPointerButtonIds",
  "ScreenView.mPointerLocationPrevious",
  # optional, the input mode of the view: no recipe yet; while it is missing, a pointer press over a button is taken
  # in every input mode
  "ScreenView.mInputMode",
  # optional, the image queue of the render context: no recipe yet; while it is missing, the buttons draw without
  # checking that the game left no image queued
  "MinecraftUIRenderContext.mQueuedImages",
  # the button actions read the open screen through the SDK wrappers: the view's controller (ScreenView.mPointerLocationPrevious
  # is listed above), the container test, the slot's visual stack and the item name offset; all have recipes
  "ScreenController.isContainerScreen",
  "ContainerScreenController.getVisualItemStackImpl",
  "Item.mFullName",
  # the vanilla container screen handlers that sort, dump and drop call (all have recipes); one missing entry turns off
  # the screen actions
  "ContainerScreenController._handleAutoPlace",
  "ContainerScreenController._handleTakeAll",
  "ContainerScreenController._handlePlaceAll",
  "ContainerScreenController._handleDropItem",
  "ContainerScreenController._handleDropSelectedItem",
  "ContainerScreenController.mContainerManagerController",
  "ContainerManagerController.handleSwap",
  # the top screen name, read on every tick: it tells a close or a switch at once (has a recipe)
  "ClientInstance.getTopScreenName",
  # the row rotation: the hidden inventory open. The server's ContainerOpen and ContainerClose answers are hidden from
  # the game (slots of the LegacyClientNetworkHandler vtable, listed above; both have recipes) and the player's runtime
  # id names the Interact target (has a recipe). The player slots need PlayerInventory.mInventory and
  # FillingContainer.mItems, listed above (no recipes yet): without them the rotation is off, with one warning
  "ClientNetworkHandler.handle.ContainerOpenPacket",
  "ClientNetworkHandler.handle.ContainerClosePacket",
  "Actor.getRuntimeID",
]

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
