use crate::traits::PacketSink;

// The packets of a hidden inventory open, over the sink that sends the item stack requests: the Interact packet that
// asks the server to open the player inventory, and the ContainerClose that ends it. The game gives it through
// `HiddenInventory`; the tests give a fake that records the order
pub trait InventoryLink: PacketSink {
    // sends Interact(OpenInventory) for the local player; false when it was not sent
    fn open_inventory(&mut self) -> bool;

    // sends the client's ContainerClose for the inventory the server opened as `container_id`; false when it was not
    // sent
    fn close_inventory(&mut self, container_id: i8) -> bool;
}
