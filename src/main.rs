#![allow(
    dead_code,
    unused_variables,
    clippy::too_many_arguments,
    clippy::unnecessary_wraps
)]

use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use vulkanalia::prelude::v1_0::*;

mod app;
mod app_data;
mod buffers;
mod camera;
mod chunk;
mod chunk_rendering;
mod chunkmesher;
mod commands;
mod constants;
mod descriptors;
mod device;
// mod gamestate;
mod chunk_manager;
mod gui;
mod images;
mod input;
mod instance;
mod load_models;
mod metrics;
mod pipeline;
mod swapchain;
mod textures;
mod utils;
mod vertex;
mod voxel;

use crate::app::RenderApp;
use crate::chunk::Chunk;
use crate::constants::*;
// use crate::gamestate::GameState;
use crate::chunk_manager::ChunkManager;
use crate::input::{handle_keyboard_input, handle_mouse_input, Action, InputMap, InputState};
use crate::metrics::EngineMetrics;
use crate::utils::*;

struct RenderState {
    window: Window,
    render_app: RenderApp,
}

struct RuntimeState {
    minimized: bool,
    inputmap: InputMap,
    inputstate: InputState,
    metrics: EngineMetrics,
    chunk_manager: ChunkManager,
}

struct WindowApp {
    renderstate: Option<RenderState>,
    runtime: RuntimeState,
}

impl ApplicationHandler for WindowApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderstate.is_some() {
            return;
        }

        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title(ENGINE_TITLE)
                    .with_inner_size(LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT)),
            )
            .unwrap();

        window.focus_window();
        // Lock cursor to screen
        // window.set_cursor_grab(winit::window::CursorGrabMode::Confined);
        // .or_else(|_e| window.set_cursor_grab(winit::window::CursorGrabMode::Locked))
        // .unwrap();

        // set cursor invisible
        window.set_cursor_visible(false);

        let render_app =
            unsafe { RenderApp::create(&window).expect("Failed to create render app.") };

        self.renderstate = Some(RenderState { window, render_app })
    }

    // TODO Rewrite this code to implement the old event loop functions.
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.renderstate.is_none() {
            return;
        }

        let renderstate = self.renderstate.as_mut().unwrap();

        let egui_response = renderstate
            .render_app
            .gui
            .egui_winit
            .on_window_event(&renderstate.window, &event);

        match event {
            // Handle shutdown
            WindowEvent::CloseRequested => {
                exit_program(&event_loop, &mut renderstate.render_app);
                event_loop.exit();
            }
            // Redraw the application.
            WindowEvent::RedrawRequested if !self.runtime.minimized => unsafe {
                renderstate
                    .render_app
                    .render(&renderstate.window, &self.runtime)
            }
            .unwrap(),
            // Mark window as resized
            WindowEvent::Resized(size) => {
                if size.width == 0 || size.height == 0 {
                    self.runtime.minimized = true;
                } else {
                    self.runtime.minimized = false;
                    renderstate.render_app.resized = true;
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if !egui_response.consumed {
                    handle_keyboard_input(
                        event,
                        &mut self.runtime.inputstate,
                        &self.runtime.inputmap,
                    );
                }
            }
            _ => (),
        }
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        match event {
            DeviceEvent::MouseMotion { delta } => {
                handle_mouse_input(delta, &mut self.runtime.inputstate)
            }
            _ => {}
        }
    }

    // Process all events
    // Request a redraw when all events were processed.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderstate.is_none() {
            return;
        }

        // Handle update
        let renderstate = self.renderstate.as_mut().unwrap();
        update(&mut renderstate.render_app, &mut self.runtime);

        // Handle shutdown
        if renderstate.render_app.shutdown_triggered {
            exit_program(&event_loop, &mut renderstate.render_app);
            event_loop.exit();
        }

        renderstate.window.request_redraw();
    }
}

fn main() -> Result<()> {
    pretty_env_logger::init();

    // Window
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll); // Note: ControlFlow::WaitUntil might give better frame pacing

    let chunk_manager = ChunkManager::new();

    // Temporary: create chunk meshdata to display.
    let red_voxel = Voxel::new(255, 0, 80);
    let green_voxel = Voxel::new(40, 255, 0);
    let blue_voxel = Voxel::new(0, 70, 255);

    const VOXEL_COUNT: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;
    let voxel_options = [red_voxel, green_voxel, blue_voxel];
    let voxels: [Voxel; VOXEL_COUNT] =
        std::array::from_fn(|_| voxel_options.choose(&mut rand::rng()).unwrap().clone());
    let active_voxels: [u64; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE / 64] =
            // std::array::from_fn(|_| rand::random());
        [u64::MAX; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE / 64];

    let chunk = Chunk::create(voxels, active_voxels, (0, 0, 0));

    let visible_chunks = chunk.mesh([None; 6]);

    let runtime = RuntimeState {
        minimized: false,
        inputmap: InputMap::new(),
        inputstate: InputState::new(),
        metrics: EngineMetrics::new(),
    };

    let mut window_app = WindowApp {
        renderstate: None,
        runtime,
    };
    let _ = event_loop.run_app(&mut window_app);

    Ok(())
}

fn exit_program(eventloop: &ActiveEventLoop, render_app: &mut RenderApp) {
    println!("Closing window");
    eventloop.exit();
    // destroying = true;
    // *control_flow = ControlFlow::Exit;
    unsafe {
        render_app.device.device_wait_idle().unwrap();
    }
    unsafe {
        render_app.destroy();
    }
}

fn update(app: &mut RenderApp, runtime: &mut RuntimeState) {
    let input_state = &mut runtime.inputstate;
    let input_map = &runtime.inputmap;

    let dt = runtime.metrics.dt();
    let velocity = MOVEMENT_SPEED * dt;

    // Control interactions
    for key in &input_state.continuous_pressed_keys {
        if let Some(action) = input_map.get_action(&key) {
            if !app.menu_mode {
                match action {
                    Action::MoveForward => app.camera.move_forward(velocity),
                    Action::MoveBackward => app.camera.move_backward(velocity),
                    Action::MoveLeft => app.camera.move_left(velocity),
                    Action::MoveRight => app.camera.move_right(velocity),
                    Action::MoveUp => app.camera.move_up(velocity),
                    Action::MoveDown => app.camera.move_down(velocity),

                    _ => {}
                }
            }
        }
    }
    let pressed: Vec<_> = input_state.single_pressed_keys.iter().copied().collect();

    for key in pressed {
        if let Some(action) = input_map.get_action(&key) {
            match action {
                Action::SetMenuMode => app.menu_mode = !app.menu_mode,
                Action::Quit => app.shutdown_triggered = true,
                _ => {}
            }
        }
        input_state.single_pressed_keys.remove(&key);
    }

    if !app.menu_mode {
        app.camera.update_camera_look(input_state.mouse_delta);
    }
    input_state.mouse_delta = (0., 0.);

    // Game updates

    // Metric collection
    runtime.metrics.update();
}
