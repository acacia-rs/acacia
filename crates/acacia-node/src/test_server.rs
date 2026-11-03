use acacia_proto::packets::{InventoryTransaction, Text, TextCategory, TextContent, TextContentChat, TextType};
use acacia_proto::types::WindowType;
use acacia_proto::Packet;
use acacia_testserver::{FakeServer, InventoryServer, Script};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use tokio::runtime::Runtime;

/// acacia-testserver's `FakeServer` on 127.0.0.1: replays a recorded BDS spawn to the first client that joins.
#[napi]
pub struct TestServer {
    server: Option<FakeServer>,
    runtime: Option<Runtime>,
    inventory: Option<InventoryServer>,
}

#[napi(object)]
pub struct ContainerItem {
    pub slot: u32,
    pub name: String,
    pub count: u32,
}

#[napi]
impl TestServer {
    #[napi(factory)]
    pub fn start() -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build()?;
        let server = runtime.block_on(FakeServer::start(Script::bds_spawn()))?;
        Ok(Self { server: Some(server), runtime: Some(runtime), inventory: None })
    }

    #[napi(getter)]
    pub fn port(&self) -> Result<u32> {
        Ok(self.server()?.addr().port().into())
    }

    /// Sends a player's chat line to the client.
    #[napi]
    pub fn chat(&self, sender: String, message: String) -> Result<()> {
        self.server()?.send(&Text {
            needs_translation: false,
            category: TextCategory::Authored,
            r#type: TextType::Chat,
            content: TextContent::Chat(TextContentChat { source_name: sender, message }),
            xuid: String::new(),
            platform_chat_id: String::new(),
            has_filtered_message: false,
            filtered_message: None,
        });
        Ok(())
    }

    /// The chat lines the client has sent so far, in order.
    #[napi]
    pub fn received_chat(&self) -> Result<Vec<String>> {
        let texts = self.server()?.received().into_iter().filter_map(|r| r.packet.decode::<Text>().ok());
        Ok(texts.filter_map(|t| if let TextContent::Chat(c) = t.content { Some(c.message) } else { None }).collect())
    }

    /// The feet position the script spawns the player at, `[x, y, z]`.
    #[napi]
    pub fn spawn_feet(&self) -> Result<Vec<f64>> {
        let feet = self.server()?.spawn_feet().ok_or_else(|| Error::from_reason("no StartGame sent yet"))?;
        Ok(feet.map(f64::from).to_vec())
    }

    /// Lays a stone floor `half` cells around the spawn, one below the feet, once the client has its chunks
    /// (blocks until then, at most 10 s).
    #[napi]
    pub fn lay_floor(&self, half: i32) -> Result<()> {
        let server = self.server()?;
        let [x, y, z] = server.spawn_feet().ok_or_else(|| Error::from_reason("no StartGame sent yet"))?;
        let feet = [x.floor() as i32, y.floor() as i32, z.floor() as i32];
        let columns: Vec<_> = [-half, half].iter().flat_map(|dx| [-half, half].map(|dz| ((feet[0] + dx) >> 4, (feet[2] + dz) >> 4))).collect();
        self.runtime()?.block_on(server.columns_sent(&columns))?;
        server.lay_floor(feet, half, acacia_testserver::bds_block_id("minecraft:stone"));
        Ok(())
    }

    /// Shows another player with its feet at `(x, y, z)`; `id` is both its runtime and unique id.
    #[napi]
    pub fn add_player(&self, name: String, id: u32, x: f64, y: f64, z: f64) -> Result<()> {
        self.server()?.add_player(&name, id.into(), id.into(), [x as f32, y as f32, z as f32]);
        Ok(())
    }

    /// Shows an entity of `kind` with its feet at `(x, y, z)`; `id` is both its runtime and unique id.
    #[napi]
    pub fn add_entity(&self, kind: String, id: u32, x: f64, y: f64, z: f64) -> Result<()> {
        self.server()?.add_entity(&kind, id.into(), id.into(), [x as f32, y as f32, z as f32]);
        Ok(())
    }

    #[napi]
    pub fn remove_entity(&self, id: u32) -> Result<()> {
        self.server()?.remove_entity(id.into());
        Ok(())
    }

    /// Puts `count` of item `name` in main-inventory slot `slot` (0-8: the hotbar).
    #[napi]
    pub fn give(&self, slot: u32, name: String, count: u32) -> Result<()> {
        self.server()?.give(slot, &name, count as u16, 1000 + slot as i32);
        Ok(())
    }

    /// Sets one cell to block `name`'s default state, as BDS's block update would.
    #[napi]
    pub fn set_block(&self, x: i32, y: i32, z: i32, name: String) -> Result<()> {
        self.server()?.set_blocks(&[[x, y, z]], acacia_testserver::bds_block_id(&name));
        Ok(())
    }

    /// Starts accepting the client's item stack requests and opening its inventory screen and the blocks
    /// `addContainer` registers (acacia-testserver's `serve_inventory`).
    #[napi]
    pub fn serve_inventory(&mut self) -> Result<()> {
        let inventory = {
            let _guard = self.runtime()?.enter();
            self.server()?.serve_inventory()
        };
        self.inventory = Some(inventory);
        Ok(())
    }

    /// Makes the block at `(x, y, z)` open as a `chest`, `furnace` or `craftingTable` window holding `items`.
    #[napi]
    pub fn add_container(&self, x: i32, y: i32, z: i32, kind: String, items: Vec<ContainerItem>) -> Result<()> {
        let kind = match kind.as_str() {
            "chest" => WindowType::Container,
            "furnace" => WindowType::Furnace,
            "craftingTable" => WindowType::Workbench,
            other => return Err(Error::from_reason(format!("unknown container kind {other}"))),
        };
        let inventory = self.inventory.as_ref().ok_or_else(|| Error::from_reason("call serveInventory first"))?;
        let items: Vec<_> = items.iter().map(|i| (i.slot as usize, i.name.as_str(), i.count as u16)).collect();
        inventory.add_container(self.server()?, [x, y, z], kind, &items);
        Ok(())
    }

    /// How many `InventoryTransaction` packets (item use on blocks and entities) the client has sent.
    #[napi]
    pub fn inventory_transactions(&self) -> Result<u32> {
        Ok(self.server()?.received().iter().filter(|r| r.packet.id == InventoryTransaction::ID).count() as u32)
    }

    #[napi]
    pub fn kick(&self, message: String) -> Result<()> {
        self.server()?.kick(message);
        Ok(())
    }

    #[napi]
    pub fn stop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            let _guard = runtime.enter();
            self.server.take();
            drop(_guard);
            runtime.shutdown_background();
        }
    }

    fn server(&self) -> Result<&FakeServer> {
        self.server.as_ref().ok_or_else(|| Error::from_reason("the test server has stopped"))
    }

    fn runtime(&self) -> Result<&Runtime> {
        self.runtime.as_ref().ok_or_else(|| Error::from_reason("the test server has stopped"))
    }
}
