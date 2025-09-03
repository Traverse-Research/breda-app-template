//! # App-template
//!
//! This crate serves as an application template for other apps. Like `inline-example`,
//! it features an inline raytracing example, but with the following differences:
//!
//! * It uses the breda streaming system
//! * It uses a pipeline defined in [`./breda.yaml`]
//! * It sets up a render graph for rendering
//!
//! Copy and paste this create to `apps/<your_app_name>` and make sure to rename any references to
//! `app-template` or `app_template`.

use std::sync::Arc;

// Re-export or Android
#[cfg(target_os = "android")]
pub use android_activity::AndroidApp;
use anyhow::{Context, Result};
use breda::{
    egui::WindowSettings,
    render_graph::{
        ExecutedRenderGraphSignalFenceWith, RasterPass, RenderGraph, RenderGraphPersistentStore,
    },
    render_loop::v2::{
        event::RenderLoopEvent,
        opts::{BredaOpts, WindowOpts},
    },
    renderer::{
        create_buffer_with_data, AccelerationStructureBuildLocation, BufferCreateDesc, BuildFlags,
        Device, GeometryFlags, IndexBufferFormat, InstanceFlags, LoadOp, QueueSubmitInfo,
        RaytracingInstanceDesc, StoreOp, TriangleGeometryCreateDesc, VertexFormat,
    },
    shader_database::{AssetsShaderDatabase, ShaderDatabase},
    shader_database_api::ShaderDatabaseAsset,
    streaming_system::StreamingSystem,
    workspace_recipe::WorkspaceBuildRecipe,
};
use clap::Parser;

/// This app serves as an application template for other apps
#[derive(Default, Parser)]
pub struct CommandlineOpts {
    #[clap(flatten)]
    pub breda: BredaOpts,
    #[clap(flatten)]
    pub window: WindowOpts,
}

fn init_streaming_system(
    streaming_system: &StreamingSystem,
    device: &Arc<dyn Device>,
) -> Result<()> {
    let mut ctx = streaming_system.create_context(device);
    let recipe = WorkspaceBuildRecipe {
        root_crate_names: vec![env!("CARGO_PKG_NAME").to_string()],

        shader_compile_targets: vec![device.preferred_compile_target()],
        ..WorkspaceBuildRecipe::default_for_current_target_platform()
    };
    streaming_system.build_workspace(recipe)?;

    let _shader_db =
        ctx.load_versioned::<_, ShaderDatabaseAsset>(&streaming_system.get_shader_db_cid()?);

    let shader_db_poller = ctx.poller();
    loop {
        streaming_system.update();

        if shader_db_poller.is_ready() {
            break;
        }
    }
    Ok(())
}

pub fn internal_main(
    opts: &CommandlineOpts,
    #[cfg(target_os = "android")] android_app: AndroidApp,
) -> Result<()> {
    let mut breda = breda::Breda::new(
        "App Template",
        opts.breda.into(),
        #[cfg(target_os = "android")]
        android_app,
    )?;

    let device_arc = breda
        .devices()
        .find(|d| d.capabilities().supports_inline_ray_tracing)
        .context("No device found that supports inline raytracing")?
        .clone();
    let streaming_system = breda.streaming_system();

    breda.render_loop().run_closure(
        opts.window.into_desc(
            "App Template",
            breda::renderer::SwapchainColorMode::ForceSrgb8Bit,
            true,
        ),
        &mut breda_app_support::EguiInputStateHandler::new_auto_size(),
        move |mut event_receiver, event_sender| -> Result<()> {
            let device = device_arc.as_ref();
            let queue = device.get_gfx_queue();

            init_streaming_system(&streaming_system, &device_arc)?;

            let mut render_graph_persistent_store = RenderGraphPersistentStore::new(device);

            let positions = vec![
                [100.0f32, 100.1f32, 100.0f32],
                [200.0f32, 100.2f32, 3.1f32],
                [302.0f32, 403.0f32, 3.2f32],
            ];
            let position_buffer = create_buffer_with_data(
                device,
                "inline position buffer",
                &BufferCreateDesc::gpu_only_storage(),
                &positions,
            );

            let indices = vec![0, 1, 2];
            let index_buffer = create_buffer_with_data(
                device,
                "inline index buffer",
                &BufferCreateDesc::gpu_only_storage(),
                &indices,
            );

            let vertex_format = VertexFormat::R32g32b32Sfloat;
            let geometry = device.create_tri_geometry(
                "inline tri geom",
                &position_buffer,
                Some(&index_buffer),
                None,
                &TriangleGeometryCreateDesc {
                    vertex_format,
                    vertex_offset_in_bytes: 0,
                    vertex_count: positions.len(),
                    vertex_stride_in_bytes: vertex_format.size_in_bytes(),
                    index_format: Some(IndexBufferFormat::Uint32),
                    index_offset_in_bytes: 0,
                    index_count: indices.len(),
                    transform_offset_in_bytes: 0,
                    geometry_flags: GeometryFlags::empty(),
                    build_location: AccelerationStructureBuildLocation::Device,
                },
            );

            let mat4x3 = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

            let mut cmd = queue.lock().create_command_buffer();

            // Note: keep the blas alive, otherwise the buffer and handle will be freed when it goes out of scope
            let (acceleration_structure, _blas) = {
                let blas_request = device.create_blas_build_request(
                    AccelerationStructureBuildLocation::Device,
                    BuildFlags::FAST_TRACE,
                    &[geometry],
                    "blas",
                );
                let blas_handle = unsafe {
                    blas_request
                        .blas
                        .blas_handle(AccelerationStructureBuildLocation::Device)
                };
                let instance = RaytracingInstanceDesc::new(
                    mat4x3,
                    0u32,
                    0xff,
                    0,
                    InstanceFlags::TRIANGLE_CULL_DISABLE,
                    blas_handle,
                );
                let tlas_request = device.create_tlas_build_request_from_instances(
                    AccelerationStructureBuildLocation::Device,
                    BuildFlags::FAST_BUILD,
                    &[instance],
                    "tlas",
                );

                let scratch = device.create_buffer(
                    "acceleration_structure_scratch",
                    u64::max(
                        blas_request
                            .build_info
                            .size_requirements()
                            .scratch_size_in_bytes,
                        tlas_request
                            .build_info
                            .size_requirements()
                            .scratch_size_in_bytes,
                    ) as usize,
                    &BufferCreateDesc::gpu_only_scratch_build(),
                );

                let mut as_enc = cmd.acceleration_structure_encoder();
                let blas = blas_request.blas.clone();
                as_enc.batch_build_bottom_level(&[blas_request], &scratch);
                as_enc.build_top_level(&tlas_request, &scratch);
                cmd.end_acceleration_structure(as_enc);

                (tlas_request.tlas, blas)
            };

            let mut egui_renderer = breda::egui::Renderer::new(device);

            let fence = queue.lock().submit(vec![cmd], QueueSubmitInfo::no_sync());
            fence.wait_for_idle();

            let mut input_processor = breda::input::InputProvider::default();

            while let Ok(RenderLoopEvent {
                swapchain,
                swapchain_sync,
                present_index,
                state,
            }) = event_receiver.receive(&device_arc, &queue)
            {
                let egui = state.apply(&mut input_processor);

                streaming_system.update();
                let mut streaming_context = streaming_system.create_context(&device_arc);
                let shader_db = streaming_context.load_versioned::<_, ShaderDatabaseAsset>(
                    &streaming_system.get_shader_db_cid()?,
                );

                let shader_db = streaming_system
                    .assets
                    .borrow::<AssetsShaderDatabase>(shader_db.handle())
                    .unwrap();

                let mut render_graph =
                    RenderGraph::new(render_graph_persistent_store, swapchain.size());

                let present_image = swapchain.present_image(present_index);
                let present_image_rg = render_graph.import_texture(&present_image);

                let tlas = render_graph.import_tlas(&acceleration_structure);
                let pipeline = shader_db.get_pipeline("app-template-raytracer");
                RasterPass::new("Main pass", &mut render_graph)
                    .render_target(&present_image_rg, LoadOp::Discard, StoreOp::Store)
                    .tlas(&tlas)
                    .draw(&pipeline, 6, 1);

                if let Some(ctx) = &egui {
                    ctx.window(
                        "Current GPU",
                        &mut true,
                        &WindowSettings::from_window_size([500.0, 140.0]),
                        |ui| {
                            let driver_info = device.driver_info();
                            ui.label(format!(
                                "{} {}",
                                driver_info.vendor, driver_info.device_name
                            ));
                            ui.label(format!(
                                "{} `{}` @ {}",
                                driver_info.driver_id,
                                driver_info.driver_name,
                                driver_info.version.map_or_else(
                                    || "Invalid driver version".to_string(),
                                    |v| v.to_string()
                                )
                            ));
                            if !driver_info.driver_extra_info.is_empty() {
                                ui.label(driver_info.driver_extra_info);
                            }
                            if let Some(shader_compiler_version) =
                                driver_info.shader_compiler_version
                            {
                                ui.small(format!(
                                    "Driver shader compiler: {shader_compiler_version}"
                                ));
                            }
                        },
                    );
                }

                let mut cmd = queue.lock().create_command_buffer();

                // compile and execute render graph
                let compiled_rg = render_graph.compile(&[&present_image_rg], None);
                let (executed_rg, signal_fence) = compiled_rg.execute(device, &mut cmd);

                render_graph_persistent_store = executed_rg.release_store();

                if let Some(mut ctx) = egui {
                    let (platform_output, render_input) = ctx.end_frame();
                    event_sender.send(platform_output);
                    egui_renderer.render(device, &mut cmd, None, &present_image, render_input);
                }

                let _fence = queue.lock().submit(
                    vec![cmd],
                    QueueSubmitInfo::swapchain_only_sync(swapchain_sync)
                        .with_render_graph_signal_fence(signal_fence),
                );
                let present_status = swapchain.present(&queue, present_index, Some(swapchain_sync));
                event_receiver.with_status(present_status);
            }

            Ok(())
        },
    )?
}
