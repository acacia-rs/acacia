//! The window's events, handed to [`App`].

use std::sync::Arc;

use acacia_render::Renderer;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

use super::App;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("Acacia").with_inner_size(winit::dpi::LogicalSize::new(1280, 720));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let size = window.inner_size();
        match Renderer::new(window.clone(), (size.width, size.height)) {
            Ok(mut r) => {
                if !self.settings.vsync {
                    r.set_vsync(false);
                }
                if let Some(sky) = &self.sky {
                    r.set_sky_textures(sky);
                }
                if let Some(strip) = acacia_render::load_crack_stages(self.looks.get(self.settings.look).files()) {
                    r.set_crack_stages(&strip);
                }
                self.renderer = Some(r);
            }
            Err(e) => {
                tracing::error!(%e, "renderer");
                event_loop.exit();
            }
        }
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if self.drive_shot() {
                    event_loop.exit();
                }
                self.frame();
                if self.failed.is_some() {
                    event_loop.exit();
                }
            }
            WindowEvent::Focused(false) => self.grab(false),
            WindowEvent::MouseInput { state, button, .. } => self.button(button, state == ElementState::Pressed),
            WindowEvent::MouseWheel { delta, .. } => self.scroll(match delta {
                MouseScrollDelta::LineDelta(_, y) => y,
                MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
            }),
            WindowEvent::KeyboardInput { event, .. } => {
                let chat_was_open = self.ui.chat.is_open();
                let pressed = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key(code, pressed);
                }
                if let (true, Some(text)) = (pressed, &event.text) {
                    self.text(text, chat_was_open);
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            self.mouse_motion(dx, dy);
        }
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.net.shutdown();
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}
