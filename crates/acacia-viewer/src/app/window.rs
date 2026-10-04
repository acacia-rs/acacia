//! The window's events, handed to [`App`].

use std::sync::Arc;

use acacia_render::Renderer;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
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
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } if !self.grabbed => self.grab(true),
            WindowEvent::MouseWheel { delta, .. } => self.input.scroll(match delta {
                MouseScrollDelta::LineDelta(_, y) => y,
                MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
            }),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key(code, event.state == ElementState::Pressed);
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let (DeviceEvent::MouseMotion { delta: (dx, dy) }, true) = (event, self.grabbed) {
            self.input.mouse(&mut self.camera, dx, dy);
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
