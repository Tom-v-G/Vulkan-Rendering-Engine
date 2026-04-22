#![allow(
    dead_code,
    unused_variables,
    clippy::too_many_arguments,
    clippy::unnecessary_wraps
)]

use std::time::Instant;

use anyhow::{anyhow, Result};

use winit::dpi::LogicalSize;
use winit::event::ElementState;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget};
use winit::window::WindowBuilder;

use vulkanalia::prelude::v1_0::*;

mod app;
mod app_data;
mod buffers;
mod camera;
mod commands;
mod constants;
mod descriptors;
mod device;
mod images;
mod input;
mod instance;
mod load_models;
mod pipeline;
mod swapchain;
mod textures;
mod utils;
mod vertex;

use crate::app::App;
use crate::constants::*;
use crate::input::{handle_keyboard_input, Action, InputMap, InputState};

fn main() -> Result<()> {
    pretty_env_logger::init();

    // Window
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll); // Note: ControlFlow::WaitUntil might give better frame pacing
    let window = WindowBuilder::new()
        .with_title("Garbo Engine")
        .with_inner_size(LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT))
        .build(&event_loop)?;

    // App
    let mut app = unsafe { App::create(&window)? };
    let mut minimized = false;

    let inputmap = InputMap::new();
    let mut input_state = InputState::new();

    let mut last_frame = Instant::now();

    event_loop.run(move |event, elwt| {
        match event {
            // Request a redraw when all events were processed.
            Event::AboutToWait => {
                let now = Instant::now();
                let dt = (now - last_frame).as_secs_f32();
                last_frame = now;

                update(&mut app, &input_state, dt, &inputmap);

                if app.shutdown_triggered {
                    exit_program(elwt, &mut app);
                }
                window.request_redraw();
            }
            Event::WindowEvent { event, .. } => match event {
                // Render a frame if our Vulkan app is not being destroyed.
                WindowEvent::RedrawRequested if !elwt.exiting() && !minimized => {
                    unsafe { app.render(&window) }.unwrap()
                }
                // Mark window as resized
                WindowEvent::Resized(size) => {
                    if size.width == 0 || size.height == 0 {
                        minimized = true;
                    } else {
                        minimized = false;
                        app.resized = true;
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    handle_keyboard_input(event, &mut app, &mut input_state)
                }
                // Destroy our Vulkan app.
                WindowEvent::CloseRequested => {
                    exit_program(elwt, &mut app);
                }
                _ => {}
            },
            _ => {}
        }
    })?;

    Ok(())
}

fn exit_program(elwt: &EventLoopWindowTarget<()>, app: &mut App) {
    println!("Closing window");
    elwt.exit();
    // destroying = true;
    // *control_flow = ControlFlow::Exit;
    unsafe {
        app.device.device_wait_idle().unwrap();
    }
    unsafe {
        app.destroy();
    }
}

fn update(app: &mut App, input: &InputState, dt: f32, input_map: &InputMap) {
    const SPEED: f32 = 5.0; // units per second
    let velocity = SPEED * dt;

    for key in &input.pressed_keys {
        if let Some(action) = input_map.get_action(key) {
            match action {
                Action::MoveForward => app.camera.move_forward(velocity),
                Action::MoveBackward => app.camera.move_backward(velocity),
                Action::MoveLeft => app.camera.move_left(velocity),
                Action::MoveRight => app.camera.move_right(velocity),
                Action::MoveUp => app.camera.move_up(velocity),
                Action::MoveDown => app.camera.move_down(velocity),
                Action::Quit => app.shutdown_triggered = true,
                _ => {}
            }
        }
    }
}
