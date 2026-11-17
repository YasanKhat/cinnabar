//! Retained server debug geometry: shared meshes, stable instance slots and GPU visibility.

use bevy::prelude::IntoScheduleConfigs;
use bevy::render::render_phase::AddRenderCommand;
use render_scene::PrimitiveShapesScene;

mod gpu;
mod mesh;
mod pipeline;
#[cfg(test)]
mod tests;

#[cfg(test)]
use bevy::prelude::{IntoSystem, System, default};
use bevy::{
    asset::{load_internal_asset, uuid_handle},
    prelude::{App, Handle, Plugin, Shader, World},
    render::{
        GpuResourceAppExt, Render, RenderApp, RenderStartup, RenderSystems,
        extract_resource::ExtractResourcePlugin,
    },
};

/// Installs the network primitive renderer independently of actor rendering.
pub struct PrimitiveShapesRenderPlugin;

const SHADER: Handle<Shader> = uuid_handle!("361a6dc1-b0e8-41ee-a5ac-94b3e8d6cc48");
mod shader;
use shader::shader;
#[cfg(test)]
use shader::{TEXT_ACOS_CUBIC, TEXT_ACOS_LINEAR};

impl Plugin for PrimitiveShapesRenderPlugin {
    /// Keeps the domain store in the main world and uploads changes in the render world.
    fn build(&self, app: &mut App) {
        app.init_resource::<PrimitiveShapesScene>()
            .add_plugins(ExtractResourcePlugin::<PrimitiveShapesScene>::default());
        load_internal_asset!(app, SHADER, "primitive_shapes/shapes.wesl", shader);
        crate::pipeline_warmup::register::<pipeline::ShapePipeline>(app);
        let Some(render) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        crate::transparent_phase::install(render);
        crate::world_passes::install(render);
        render.init_resource::<pipeline::ShapePipeline>()
            .init_gpu_resource::<pipeline::ShapePipeline>()
            .add_render_command::<bevy::core_pipeline::core_3d::Transparent3d, pipeline::DrawShapes>()
            .add_systems(RenderStartup, gpu::init.after(bevy::render::init_gpu_resource::<pipeline::ShapePipeline>))
            .add_systems(Render, (
                gpu::prepare.in_set(RenderSystems::PrepareResources),
                pipeline::prepare_bind_groups.in_set(RenderSystems::PrepareBindGroups),
                pipeline::queue.in_set(crate::world_passes::WorldPassSystems).in_set(RenderSystems::Queue),
            ));
    }
}

/// Identifies the debug draw family for encoded-color transparency routing.
pub(crate) fn draw_function(world: &World) -> Option<bevy::render::render_phase::DrawFunctionId> {
    world.get_resource::<bevy::render::render_phase::DrawFunctions<bevy::core_pipeline::core_3d::Transparent3d>>()?.read().get_id::<pipeline::DrawShapes>()
}
