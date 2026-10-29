use acacia_proto::packets::{Text, TextCategory, TextContent, TextContentChat, TextType};
use acacia_testserver::{FakeServer, Script};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use tokio::runtime::Runtime;

/// acacia-testserver's `FakeServer` on 127.0.0.1: replays a recorded BDS spawn to the first client that joins.
#[napi]
pub struct TestServer {
    server: Option<FakeServer>,
    runtime: Option<Runtime>,
}

#[napi]
impl TestServer {
    #[napi(factory)]
    pub fn start() -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build()?;
        let server = runtime.block_on(FakeServer::start(Script::bds_spawn()))?;
        Ok(Self { server: Some(server), runtime: Some(runtime) })
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
}
