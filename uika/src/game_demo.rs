// Gem Collector game demo — top-down pawn, collect gems, score + countdown.
//
// Setup requirements (in UE Editor):
// 1. Set GameMode Override = GemCollectorGameMode in World Settings
//    (DefaultPawnClass and HUDClass are configured automatically via CDO)

use uika_macros::{uclass, uclass_impl};
use uika_runtime::{
    ulog, Checked, DynamicCall, OwnedStruct, UObjectRef, UikaResult,
    LOG_DISPLAY, LOG_WARNING,
};
use uika_bindings::core_ue::{FLinearColor, FRotator, FRotatorExt, FTransform, Object};
use uika_bindings::engine::{
    Actor, ActorExt, ActorComponent, CameraActor,
    DefaultPawn,
    GameModeBase, GameModeBaseExt, GameplayStatics, GameplayStaticsExt,
    HUD, HUDExt, Pawn,
    DirectionalLight, SceneComponent, SkyLight,
    StaticMesh, StaticMeshActor, StaticMeshComponent, StaticMeshComponentExt,
    World,
};
use uika_bindings::manual::{
    vector::OwnedFVectorExt,
    world_ext::{self, WorldSpawnExt},
};
use uika_runtime::{LinearColor, Transform};
use glam::{DQuat, DVec3};

// ---------------------------------------------------------------------------
// Helper: set a component to Movable mobility via DynamicCall
// ---------------------------------------------------------------------------

fn try_set_movable(component: &UObjectRef<impl uika_runtime::UeClass>) {
    match DynamicCall::new(component, "SetMobility") {
        Err(e) => {
            ulog!(LOG_WARNING, "[GemCollector] SetMobility: find_function failed: {:?}", e);
        }
        Ok(mut call) => {
            if let Err(e) = call.set("NewMobility", 2u8) {
                ulog!(LOG_WARNING, "[GemCollector] SetMobility: set param failed: {:?}", e);
                return;
            }
            if let Err(e) = call.call() {
                ulog!(LOG_WARNING, "[GemCollector] SetMobility: call failed: {:?}", e);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// GemCollectorGameMode — sets DefaultPawnClass to GemCollectorPawn via CDO
// ---------------------------------------------------------------------------

#[uclass(parent = GameModeBase)]
pub struct GemCollectorGameMode {}

#[uclass_impl]
impl GemCollectorGameMode {
    #[ufunction(Override)]
    fn receive_begin_play(&mut self) {
        ulog!(LOG_DISPLAY, "[GemCollector] GameMode::ReceiveBeginPlay!");
        if let Err(e) = self.setup_game() {
            ulog!(LOG_WARNING, "[GemCollector] setup_game failed: {:?}", e);
        }
    }
}

impl GemCollectorGameMode {
    /// Called from ReceiveBeginPlay. Spawns floor, lights, pawn, and possesses.
    /// Camera is set up by the pawn on its first tick (after possess completes).
    fn setup_game(&self) -> UikaResult<()> {
        let gm_actor: UObjectRef<Actor> = unsafe { UObjectRef::from_raw(self.__obj) };
        let world_h = uika_runtime::world::get_world_raw(gm_actor.checked()?.raw())?;
        let world: UObjectRef<World> = unsafe { UObjectRef::from_raw(world_h) };
        let world_ctx: UObjectRef<Object> = unsafe { UObjectRef::from_raw(self.__obj) };

        Self::spawn_floor(&world)?;
        Self::spawn_lights(&world)?;

        // Spawn our pawn manually (UE's auto-spawn fires too late)
        let spawn_transform = FTransform::from_transform(Transform::new(
            DQuat::IDENTITY,
            DVec3::new(0.0, 0.0, 100.0),
            DVec3::ONE,
        ));
        let pawn: UObjectRef<GemCollectorPawn> = world.spawn_actor(&spawn_transform)?;
        let pawn_raw = pawn.raw();
        ulog!(LOG_DISPLAY, "[GemCollector] Spawned pawn at z=100");

        // Possess via PlayerController
        let pc = <Checked<GameplayStatics> as GameplayStaticsExt>::get_player_controller(world_ctx, 0);
        let pc_raw = pc.raw();
        let controller: UObjectRef<Actor> = unsafe { UObjectRef::from_raw(pc_raw) };
        // Use DynamicCall for Possess since we need Controller, not Actor
        let mut call = DynamicCall::new(&controller, "Possess")?;
        call.set("InPawn", pawn_raw)?;
        call.call()?;
        ulog!(LOG_DISPLAY, "[GemCollector] Possessed pawn");

        // Manually create HUD — CDO HUDClass may not propagate after hot-reload
        let hud_class_h = <GemCollectorHUD as uika_runtime::UeClass>::static_class();
        match DynamicCall::new(&controller, "ClientSetHUD") {
            Ok(mut call) => {
                match call.set("NewHUDClass", uika_ffi::UObjectHandle(hud_class_h.0)) {
                    Ok(()) => match call.call() {
                        Ok(_) => ulog!(LOG_DISPLAY, "[GemCollector] HUD created via ClientSetHUD"),
                        Err(e) => ulog!(LOG_WARNING, "[GemCollector] ClientSetHUD call failed: {:?}", e),
                    },
                    Err(e) => ulog!(LOG_WARNING, "[GemCollector] ClientSetHUD set param failed: {:?}", e),
                }
            }
            Err(e) => ulog!(LOG_WARNING, "[GemCollector] ClientSetHUD not found: {:?}", e),
        }

        ulog!(LOG_DISPLAY, "[GemCollector] Game setup complete!");
        Ok(())
    }

    fn spawn_floor(world: &UObjectRef<World>) -> UikaResult<()> {
        let mesh: UObjectRef<StaticMesh> = world_ext::load_object("/Engine/BasicShapes/Plane")?;

        let transform = FTransform::from_transform(Transform::new(
            DQuat::IDENTITY,
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(50.0, 50.0, 1.0),
        ));
        let floor: UObjectRef<StaticMeshActor> = world.spawn_actor(&transform)?;
        let floor_actor: UObjectRef<Actor> = floor.cast::<Actor>()?;

        let smc_class = <StaticMeshComponent as uika_runtime::UeClass>::static_class();
        let smc_class_ref: UObjectRef<ActorComponent> = unsafe {
            UObjectRef::from_raw(uika_ffi::UObjectHandle(smc_class.0))
        };
        let component = floor_actor.checked()?.get_component_by_class(smc_class_ref);
        let mesh_comp: UObjectRef<StaticMeshComponent> = component.cast::<StaticMeshComponent>()?;
        mesh_comp.checked()?.set_static_mesh(mesh);
        try_set_movable(&mesh_comp);

        ulog!(LOG_DISPLAY, "[GemCollector] Floor spawned");
        Ok(())
    }

    fn spawn_lights(world: &UObjectRef<World>) -> UikaResult<()> {
        let dl_transform = FTransform::from_transform(Transform::new(
            DQuat::from_rotation_y(-std::f64::consts::FRAC_PI_4),
            DVec3::new(0.0, 0.0, 500.0),
            DVec3::ONE,
        ));
        let dl: UObjectRef<DirectionalLight> = world.spawn_actor(&dl_transform)?;
        let dl_actor: UObjectRef<Actor> = dl.cast::<Actor>()?;
        let dl_root = dl_actor.checked()?.k2_get_root_component();
        try_set_movable(&dl_root);
        ulog!(LOG_DISPLAY, "[GemCollector] Directional light spawned");

        let sl_transform = FTransform::from_transform(Transform::new(
            DQuat::IDENTITY,
            DVec3::new(0.0, 0.0, 500.0),
            DVec3::ONE,
        ));
        let sl: UObjectRef<SkyLight> = world.spawn_actor(&sl_transform)?;
        let sl_actor: UObjectRef<Actor> = sl.cast::<Actor>()?;
        let sl_root = sl_actor.checked()?.k2_get_root_component();
        try_set_movable(&sl_root);
        ulog!(LOG_DISPLAY, "[GemCollector] Sky light spawned");

        Ok(())
    }
}

/// Set CDO defaults that reference other reified classes.
pub fn post_register_setup() {
    ulog!(LOG_DISPLAY, "[GemCollector] post_register_setup() called");
    let gm_class = <GemCollectorGameMode as uika_runtime::UeClass>::static_class();
    if gm_class.is_null() {
        ulog!(LOG_WARNING, "[GemCollector] post_register_setup: gm_class is null!");
        return;
    }
    let api = uika_runtime::api();
    unsafe {
        let cdo = ((*api.reify).get_cdo)(gm_class);
        if cdo.is_null() {
            ulog!(LOG_WARNING, "[GemCollector] post_register_setup: CDO is null!");
            return;
        }
        let gm_ref: UObjectRef<GameModeBase> = UObjectRef::from_raw(cdo);

        match gm_ref.checked() {
            Ok(gm) => {
                // DefaultPawnClass = GemCollectorPawn (for editor display; we spawn manually)
                let pawn_class = <GemCollectorPawn as uika_runtime::UeClass>::static_class();
                let pawn_ref: UObjectRef<Pawn> = UObjectRef::from_raw(
                    uika_ffi::UObjectHandle(pawn_class.0),
                );
                gm.set_default_pawn_class(pawn_ref);
                ulog!(LOG_DISPLAY, "[GemCollector] CDO DefaultPawnClass = GemCollectorPawn");

                // HUDClass = GemCollectorHUD
                let hud_class = <GemCollectorHUD as uika_runtime::UeClass>::static_class();
                let hud_ref: UObjectRef<HUD> = UObjectRef::from_raw(
                    uika_ffi::UObjectHandle(hud_class.0),
                );
                gm.set_hud_class(hud_ref);
                ulog!(LOG_DISPLAY, "[GemCollector] CDO HUDClass = GemCollectorHUD");
            }
            Err(e) => ulog!(LOG_WARNING, "[GemCollector] CDO property setup failed: {:?}", e),
        }
    }
}

// ---------------------------------------------------------------------------
// GemCollectorHUD — draws score and time remaining on screen
// ---------------------------------------------------------------------------

#[uclass(parent = HUD)]
pub struct GemCollectorHUD {}

#[uclass_impl]
impl GemCollectorHUD {
    #[ufunction(Override)]
    fn receive_begin_play(&mut self) {
        ulog!(LOG_DISPLAY, "[GemCollector] HUD::ReceiveBeginPlay! HUD is alive.");
    }

    #[ufunction(Override)]
    fn receive_draw_hud(&mut self, size_x: i32, size_y: i32) {
        if let Err(e) = self.draw_game_hud(size_x, size_y) {
            ulog!(LOG_WARNING, "[GemCollector] HUD draw error: {:?}", e);
        }
    }
}

impl GemCollectorHUD {
    fn self_as_hud(&self) -> UObjectRef<HUD> {
        unsafe { UObjectRef::from_raw(self.__obj) }
    }

    fn draw_game_hud(&self, _size_x: i32, _size_y: i32) -> UikaResult<()> {
        let hud = self.self_as_hud().checked()?;

        let pawn = hud.get_owning_pawn();
        if !pawn.is_valid() {
            // No pawn yet — draw a waiting message
            let white = FLinearColor::from_linear_color(LinearColor::WHITE);
            hud.draw_text("Waiting for pawn...", &white, 20.0, 20.0, None, Some(2.0), None);
            return Ok(());
        }
        let score: i32 = match DynamicCall::new(&pawn, "GetScore").and_then(|c| c.call()) {
            Ok(r) => r.get("ReturnValue").unwrap_or(0),
            Err(_) => 0,
        };
        let time_remaining: f32 = match DynamicCall::new(&pawn, "GetTimeRemaining").and_then(|c| c.call()) {
            Ok(r) => r.get("ReturnValue").unwrap_or(0.0),
            Err(_) => 0.0,
        };

        let white = FLinearColor::from_linear_color(LinearColor::WHITE);
        let yellow = FLinearColor::from_linear_color(LinearColor::new(1.0, 1.0, 0.0, 1.0));
        let red = FLinearColor::from_linear_color(LinearColor::RED);

        // Background bar
        let black_bg = FLinearColor::from_linear_color(LinearColor::new(0.0, 0.0, 0.0, 0.6));
        hud.draw_rect(&black_bg, 10.0, 10.0, 280.0, 70.0);

        // Score text
        let score_text = format!("Score: {}", score);
        hud.draw_text(&score_text, &white, 20.0, 18.0, None, Some(2.0), None);

        // Time remaining (yellow if > 10s, red if <= 10s)
        let time_color = if time_remaining > 10.0 { &yellow } else { &red };
        let time_text = format!("Time: {:.0}s", time_remaining);
        hud.draw_text(&time_text, time_color, 20.0, 48.0, None, Some(2.0), None);

        // Game over overlay
        if time_remaining <= 0.0 {
            let overlay = FLinearColor::from_linear_color(LinearColor::new(0.0, 0.0, 0.0, 0.7));
            hud.draw_rect(&overlay, 0.0, 0.0, _size_x as f32, _size_y as f32);
            let game_over_text = format!("GAME OVER — Final Score: {}", score);
            let x = (_size_x as f32 / 2.0) - 200.0;
            let y = _size_y as f32 / 2.0 - 20.0;
            hud.draw_text(&game_over_text, &red, x, y, None, Some(3.0), None);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CollectibleGem — passive actor, collected by proximity
// ---------------------------------------------------------------------------

#[uclass(parent = Actor)]
pub struct CollectibleGem {
    #[component(root)]
    root_scene: SceneComponent,

    #[component(attach = "root_scene")]
    mesh: StaticMeshComponent,

    #[uproperty(BlueprintReadWrite, default = 10)]
    point_value: i32,
}

#[uclass_impl]
impl CollectibleGem {}

// ---------------------------------------------------------------------------
// GemCollectorPawn — player-controlled top-down pawn
// ---------------------------------------------------------------------------

#[uclass(parent = DefaultPawn)]
pub struct GemCollectorPawn {
    #[uproperty(BlueprintReadWrite)]
    score: i32,

    #[uproperty(BlueprintReadWrite, default = 400.0)]
    move_speed: f32,

    #[uproperty(BlueprintReadWrite, default = 60.0)]
    time_remaining: f32,

    // Rust-private fields
    gem_spawn_timer: f32,
    game_over: bool,
    camera_setup_done: bool,
}

#[uclass_impl]
impl GemCollectorPawn {
    #[ufunction(Override)]
    fn receive_begin_play(&mut self) {
        ulog!(LOG_DISPLAY, "[GemCollector] Pawn::ReceiveBeginPlay!");
        if let Err(e) = self.init_game() {
            ulog!(LOG_WARNING, "[GemCollector] init_game failed: {:?}", e);
        }
    }

    #[ufunction(Override)]
    fn receive_tick(&mut self, delta_seconds: f32) {
        // Deferred camera setup (first tick after UE possesses the pawn)
        if !self.camera_setup_done() {
            if let Err(e) = self.setup_camera() {
                ulog!(LOG_WARNING, "[GemCollector] camera setup failed: {:?}", e);
            }
        }

        if self.game_over() {
            return;
        }
        if let Err(e) = self.tick_game(delta_seconds) {
            ulog!(LOG_WARNING, "[GemCollector] tick error: {:?}", e);
        }
    }

    #[ufunction(BlueprintCallable)]
    fn get_score(&self) -> i32 {
        self.score()
    }

    #[ufunction(BlueprintCallable)]
    fn get_time_remaining(&self) -> f32 {
        self.time_remaining()
    }
}

impl GemCollectorPawn {
    fn self_as_actor(&self) -> UObjectRef<Actor> {
        unsafe { UObjectRef::from_raw(self.__obj) }
    }

    fn get_world(&self) -> UikaResult<UObjectRef<World>> {
        let h = self.self_as_actor().checked()?.raw();
        let world_h = uika_runtime::world::get_world_raw(h)?;
        Ok(unsafe { UObjectRef::from_raw(world_h) })
    }

    fn init_game(&mut self) -> UikaResult<()> {
        self.set_time_remaining(60.0);
        self.set_score(0);
        self.set_move_speed(400.0);
        self.set_game_over(false);
        self.set_gem_spawn_timer(0.0);
        self.set_camera_setup_done(false);

        let world = self.get_world()?;
        let actor_ref = self.self_as_actor();
        let pawn_pos = actor_ref.checked()?.k2_get_actor_location().to_dvec3();
        ulog!(LOG_DISPLAY, "[GemCollector] Pawn at {:?}", pawn_pos);

        // Spawn 10 gems in a circular pattern around the pawn
        for i in 0..10 {
            let angle = (i as f64) * std::f64::consts::TAU / 10.0;
            let radius = 500.0 + (i as f64) * 100.0;
            self.spawn_gem_at(
                &world,
                pawn_pos.x + angle.cos() * radius,
                pawn_pos.y + angle.sin() * radius,
            )?;
        }

        // Overlap delegate for gem collection
        let binding = actor_ref.checked()?.on_actor_begin_overlap().add(move |_self_actor, other| {
            let handle = other.raw();
            if other.cast::<CollectibleGem>().is_ok() {
                let a: UObjectRef<Actor> = unsafe { UObjectRef::from_raw(handle) };
                if let Ok(c) = a.checked() {
                    c.k2_destroy_actor();
                }
            }
        })?;
        std::mem::forget(binding);

        ulog!(LOG_DISPLAY, "[GemCollector] Game started! 60 seconds, collect gems.");
        Ok(())
    }

    /// Camera setup — deferred to first tick so the pawn is already possessed.
    fn setup_camera(&mut self) -> UikaResult<()> {
        self.set_camera_setup_done(true);
        ulog!(LOG_DISPLAY, "[GemCollector] setup_camera starting...");

        let actor_ref = self.self_as_actor();
        let pawn_pos = actor_ref.checked()?.k2_get_actor_location().to_dvec3();
        let world = self.get_world()?;

        // Spawn camera above pawn
        let cam_transform = FTransform::from_transform(Transform::new(
            DQuat::IDENTITY,
            DVec3::new(pawn_pos.x, pawn_pos.y, pawn_pos.z + 800.0),
            DVec3::ONE,
        ));
        let camera: UObjectRef<CameraActor> = world.spawn_actor(&cam_transform)?;
        let cam_raw = camera.raw();

        // Rotate to look straight down
        let rot = OwnedStruct::<FRotator>::new();
        rot.as_ref().set_pitch(-90.0);
        rot.as_ref().set_yaw(0.0);
        rot.as_ref().set_roll(0.0);
        let cam_actor: UObjectRef<Actor> = unsafe { UObjectRef::from_raw(cam_raw) };
        cam_actor.checked()?.k2_set_actor_rotation(&rot, false);

        // Get our PlayerController and set view target
        let world_ctx: UObjectRef<Object> = unsafe { UObjectRef::from_raw(self.__obj) };
        let pc = <Checked<GameplayStatics> as GameplayStaticsExt>::get_player_controller(world_ctx, 0);
        let mut call = DynamicCall::new(&pc, "SetViewTargetWithBlend")?;
        call.set("NewViewTarget", cam_raw)?;
        call.set("BlendTime", 0.0f32)?;
        call.call()?;

        ulog!(LOG_DISPLAY, "[GemCollector] Camera set up at {:?}, looking down", pawn_pos);
        Ok(())
    }

    fn tick_game(&mut self, dt: f32) -> UikaResult<()> {
        // Countdown
        let prev_time = self.time_remaining();
        let time = (prev_time - dt).max(0.0);
        self.set_time_remaining(time);

        // Log every 10 seconds for diagnostics
        let prev_ten = (prev_time / 10.0) as i32;
        let curr_ten = (time / 10.0) as i32;
        if curr_ten < prev_ten {
            ulog!(LOG_DISPLAY, "[GemCollector] Time: {:.0}s remaining, score: {}", time, self.score());
        }

        if time <= 0.0 {
            self.set_game_over(true);
            ulog!(LOG_DISPLAY, "[GemCollector] Game Over! Final score: {}", self.score());
            return Ok(());
        }

        // Gem pickup by proximity
        self.check_gem_pickup()?;

        // Gem respawn timer (every 5 seconds)
        let timer = self.gem_spawn_timer() + dt;
        self.set_gem_spawn_timer(timer);
        if timer >= 5.0 {
            self.set_gem_spawn_timer(0.0);
            let pawn_pos = self.self_as_actor().checked()?.k2_get_actor_location().to_dvec3();
            let t = time as f64;
            let angle = t * 1.618;
            let radius = 400.0 + (t * 7.0) % 600.0;
            if let Err(e) = self.spawn_gem_at(
                &self.get_world()?,
                pawn_pos.x + angle.cos() * radius,
                pawn_pos.y + angle.sin() * radius,
            ) {
                ulog!(LOG_WARNING, "[GemCollector] gem spawn failed: {:?}", e);
            }
        }

        Ok(())
    }

    fn check_gem_pickup(&mut self) -> UikaResult<()> {
        let pawn_pos = self.self_as_actor().checked()?.k2_get_actor_location().to_dvec3();
        let world = self.get_world()?;
        let gems: Vec<UObjectRef<CollectibleGem>> = world.get_all_actors_of_class()?;

        for gem_ref in gems {
            if let Ok(gem_actor) = gem_ref.cast::<Actor>() {
                if let Ok(checked) = gem_actor.checked() {
                    let gem_pos = checked.k2_get_actor_location().to_dvec3();
                    if (pawn_pos - gem_pos).length() < 150.0 {
                        self.set_score(self.score() + 10);
                        checked.k2_destroy_actor();
                        ulog!(LOG_DISPLAY, "[GemCollector] +10! Score: {}", self.score());
                    }
                }
            }
        }

        Ok(())
    }

    fn spawn_gem_at(&self, world: &UObjectRef<World>, x: f64, y: f64) -> UikaResult<()> {
        let t = FTransform::from_transform(Transform::new(
            DQuat::IDENTITY,
            DVec3::new(x, y, 50.0),
            DVec3::ONE,
        ));
        let gem: UObjectRef<CollectibleGem> = world.spawn_actor(&t)?;
        let gem_raw = gem.raw();

        // Access the declared mesh component directly via #[component] accessor
        let gem_typed = CollectibleGem::from_obj(gem)?;
        let mesh_comp = gem_typed.mesh()?;
        try_set_movable(&mesh_comp); // Must set Movable BEFORE assigning mesh
        let mesh: UObjectRef<StaticMesh> = world_ext::load_object("/Engine/BasicShapes/Sphere")?;
        mesh_comp.checked()?.set_static_mesh(mesh);

        // Scale down — default sphere is 100cm diameter
        let gem_actor: UObjectRef<Actor> = unsafe { UObjectRef::from_raw(gem_raw) };
        let scale = uika_bindings::core_ue::FVector::from_dvec3(DVec3::new(0.3, 0.3, 0.3));
        gem_actor.checked()?.set_actor_scale3_d(&scale);

        Ok(())
    }
}
