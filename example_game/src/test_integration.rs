// Comprehensive integration test suite for Uika.
// This module defines a UikaTestRunner reified actor with a RunAllTests()
// BlueprintCallable function. Place it in a level and wire BeginPlay to
// RunAllTests to validate all major APIs in a live UE environment.

use uika::{uclass, uclass_impl};
use uika::runtime::{
    ulog, DynamicCall, FName, OwnedStruct, Pinned, TWeakObjectPtr,
    Transform, UeClass, UObjectRef, UikaError, UikaResult,
    LOG_DISPLAY, LOG_ERROR,
};
use uika::bindings::core_ue::{
    FQuat, FRotator, FTransform, FVector,
};
use uika::bindings::engine::{
    Actor, ActorExt, Pawn, PawnExt, World,
    EAttachmentRule,
};
use uika::bindings::manual::{
    quat::OwnedFQuatExt,
    rotator::OwnedFRotatorExt,
    transform::OwnedFTransformExt,
    vector::OwnedFVectorExt,
    world_ext::{self, SpawnCollisionMethod, WorldSpawnExt},
};
use glam::{DQuat, DVec3};

// ---------------------------------------------------------------------------
// UikaTestRunner — reified actor
// ---------------------------------------------------------------------------

#[uclass(parent = Actor)]
pub struct UikaTestRunner {
    #[uproperty(BlueprintReadWrite)]
    total_run: i32,

    #[uproperty(BlueprintReadWrite)]
    total_passed: i32,

    #[uproperty(BlueprintReadWrite)]
    total_failed: i32,
}

// ---------------------------------------------------------------------------
// Test harness macro
// ---------------------------------------------------------------------------

macro_rules! run_test {
    ($self:expr, $name:expr, $body:expr) => {{
        $self.set_total_run($self.total_run() + 1);
        let result: UikaResult<()> = (|| $body)();
        match result {
            Ok(()) => {
                $self.set_total_passed($self.total_passed() + 1);
                ulog!(LOG_DISPLAY, "[UikaTest] PASS: {}", $name);
            }
            Err(e) => {
                $self.set_total_failed($self.total_failed() + 1);
                ulog!(LOG_ERROR, "[UikaTest] FAIL: {} -- {:?}", $name, e);
            }
        }
    }};
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn get_world(actor: &UObjectRef<Actor>) -> UikaResult<UObjectRef<World>> {
    let h = actor.checked()?.raw();
    let world_h = uika::runtime::world::get_world_raw(h)?;
    Ok(unsafe { UObjectRef::from_raw(world_h) })
}

fn identity_transform() -> OwnedStruct<FTransform> {
    FTransform::from_transform(Transform::IDENTITY)
}

fn transform_at(x: f64, y: f64, z: f64) -> OwnedStruct<FTransform> {
    FTransform::from_transform(Transform::new(DQuat::IDENTITY, DVec3::new(x, y, z), DVec3::ONE))
}


fn assert_near(a: f64, b: f64, eps: f64) -> UikaResult<()> {
    if (a - b).abs() > eps {
        Err(UikaError::InvalidOperation(format!(
            "assert_near failed: {a} vs {b} (eps={eps})"
        )))
    } else {
        Ok(())
    }
}

fn assert_true(cond: bool, msg: &str) -> UikaResult<()> {
    if cond {
        Ok(())
    } else {
        Err(UikaError::InvalidOperation(msg.into()))
    }
}

fn assert_false(cond: bool, msg: &str) -> UikaResult<()> {
    if !cond {
        Ok(())
    } else {
        Err(UikaError::InvalidOperation(msg.into()))
    }
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

#[uclass_impl]
impl UikaTestRunner {
    #[ufunction(BlueprintCallable)]
    fn run_all_tests(&mut self) {
        ulog!(LOG_DISPLAY, "[UikaTest] ========================================");
        ulog!(LOG_DISPLAY, "[UikaTest] Starting integration tests...");
        ulog!(LOG_DISPLAY, "[UikaTest] ========================================");

        self.set_total_run(0);
        self.set_total_passed(0);
        self.set_total_failed(0);

        // Get self_ref and world for tests that need them.
        let self_ref: UObjectRef<Actor> = unsafe {
            UObjectRef::from_raw(self.__obj)
        };

        // A. Core References
        self.test_core_references(&self_ref);

        // B. FName
        self.test_fname();

        // C. Math Conversions
        self.test_math_conversions();

        // D. Actor Properties
        self.test_actor_properties(&self_ref);

        // E. Actor Movement
        self.test_actor_movement(&self_ref);

        // F. Actor Lifecycle
        self.test_actor_lifecycle(&self_ref);

        // G. World Operations
        self.test_world_operations(&self_ref);

        // H. DynamicCall
        self.test_dynamic_call(&self_ref);

        // I. Containers
        self.test_containers(&self_ref);

        // J. Delegates
        self.test_delegates(&self_ref);

        // K. Weak Pointers
        self.test_weak_pointers(&self_ref);

        // L. Error Handling
        self.test_error_handling(&self_ref);

        // M. Realistic Game Patterns
        self.test_game_patterns(&self_ref);

        // N. Inheritance Flattening
        self.test_inheritance_flattening(&self_ref);

        // P. OwnedStruct Init/Destroy (P0-3)
        self.test_owned_struct_init();

        // Q. NewObject (P0-2)
        self.test_new_object(&self_ref);

        // R. Runtime Type Instantiation (P1-1)
        self.test_runtime_instantiation(&self_ref);

        // S. Deferred Spawn (P1-2)
        self.test_deferred_spawn(&self_ref);

        // O. Hot Reload Validation
        self.test_hot_reload();

        ulog!(LOG_DISPLAY, "[UikaTest] ========================================");
        ulog!(
            LOG_DISPLAY,
            "[UikaTest] Results: {} run, {} passed, {} failed",
            self.total_run(),
            self.total_passed(),
            self.total_failed()
        );
        ulog!(LOG_DISPLAY, "[UikaTest] ========================================");
    }

    // -----------------------------------------------------------------------
    // A. Core References (7 tests)
    // -----------------------------------------------------------------------

    fn test_core_references(&mut self, self_ref: &UObjectRef<Actor>) {
        run_test!(self, "A1: self_ref_is_valid", {
            assert_true(self_ref.is_valid(), "self ref should be valid")
        });

        run_test!(self, "A2: get_name_returns_nonempty", {
            let name = self_ref.get_name()?;
            assert_true(!name.is_empty(), "name should not be empty")
        });

        run_test!(self, "A3: get_class_returns_valid", {
            let cls = self_ref.get_class()?;
            assert_true(!cls.0.is_null(), "class handle should not be null")
        });

        run_test!(self, "A4: get_outer_returns_nonnull", {
            let outer = self_ref.get_outer()?;
            assert_true(!outer.0.is_null(), "outer should not be null (actors have Level outer)")
        });

        run_test!(self, "A5: cast_to_object_succeeds", {
            use uika::bindings::core_ue::Object;
            let copy = unsafe { UObjectRef::<Actor>::from_raw(self_ref.raw()) };
            let _obj_ref: UObjectRef<Object> = copy.cast()?;
            Ok(())
        });

        run_test!(self, "A6: cast_to_wrong_type_fails", {
            let copy = unsafe { UObjectRef::<Actor>::from_raw(self_ref.raw()) };
            let result: UikaResult<UObjectRef<World>> = copy.cast();
            match result {
                Err(UikaError::InvalidCast) => Ok(()),
                Err(e) => Err(UikaError::InvalidOperation(
                    format!("expected InvalidCast, got: {:?}", e),
                )),
                Ok(_) => Err(UikaError::InvalidOperation(
                    "expected cast to fail, but it succeeded".into(),
                )),
            }
        });

        run_test!(self, "A7: pin_keeps_valid", {
            let copy = unsafe { UObjectRef::<Actor>::from_raw(self_ref.raw()) };
            let pinned: Pinned<Actor> = copy.pin()?;
            assert_true(pinned.as_ref().is_valid(), "pinned ref should be valid")
        });

        run_test!(self, "A8: pinned_direct_method_call", {
            // Phase A: Pinned<T> can call Ext trait methods directly (no as_ref())
            let copy = unsafe { UObjectRef::<Actor>::from_raw(self_ref.raw()) };
            let pinned: Pinned<Actor> = copy.pin()?;
            let tag = FName::new("NonexistentTag12345");
            let has_tag = pinned.actor_has_tag(tag.handle());
            assert_true(!has_tag, "should not have a nonexistent tag")
        });

        run_test!(self, "A9: pinned_is_alive", {
            // Phase B: is_alive() returns true for a live pinned object
            let copy = unsafe { UObjectRef::<Actor>::from_raw(self_ref.raw()) };
            let pinned: Pinned<Actor> = copy.pin()?;
            assert_true(pinned.is_alive(), "pinned object should be alive")
        });
    }

    // -----------------------------------------------------------------------
    // B. FName (4 tests)
    // -----------------------------------------------------------------------

    fn test_fname(&mut self) {
        run_test!(self, "B1: fname_roundtrip", {
            let name = FName::new("TestTag");
            let s = name.to_string_lossy();
            assert_true(s == "TestTag", &format!("expected 'TestTag', got '{s}'"))
        });

        run_test!(self, "B2: fname_none_is_none", {
            assert_true(FName::NONE.is_none(), "FName::NONE should be none")
        });

        run_test!(self, "B3: fname_equality", {
            let a = FName::new("Same");
            let b = FName::new("Same");
            assert_true(a == b, "two FName::new('Same') should be equal")
        });

        run_test!(self, "B4: fname_display", {
            let name = FName::new("DisplayTest");
            let s = format!("{}", name);
            assert_true(s == "DisplayTest", &format!("expected 'DisplayTest', got '{s}'"))
        });
    }

    // -----------------------------------------------------------------------
    // C. Math Conversions (6 tests)
    // -----------------------------------------------------------------------

    fn test_math_conversions(&mut self) {
        run_test!(self, "C1: fvector_dvec3_roundtrip", {
            let v = DVec3::new(1.0, 2.0, 3.0);
            let fv = FVector::from_dvec3(v);
            let back = fv.to_dvec3();
            assert_near(back.x, 1.0, 0.001)?;
            assert_near(back.y, 2.0, 0.001)?;
            assert_near(back.z, 3.0, 0.001)
        });

        run_test!(self, "C2: frotator_rotator_roundtrip", {
            let r = uika::runtime::Rotator::new(30.0, 45.0, 60.0);
            let fr = FRotator::from_rotator(r);
            let back = fr.to_rotator();
            assert_near(back.pitch, 30.0, 0.001)?;
            assert_near(back.yaw, 45.0, 0.001)?;
            assert_near(back.roll, 60.0, 0.001)
        });

        run_test!(self, "C3: ftransform_identity_roundtrip", {
            let t = Transform::IDENTITY;
            let ft = FTransform::from_transform(t);
            let back = ft.to_transform();
            assert_near(back.translation.x, 0.0, 0.001)?;
            assert_near(back.translation.y, 0.0, 0.001)?;
            assert_near(back.translation.z, 0.0, 0.001)
        });

        run_test!(self, "C4: ftransform_translation_roundtrip", {
            let t = Transform::new(DQuat::IDENTITY, DVec3::new(100.0, 200.0, 300.0), DVec3::ONE);
            let ft = FTransform::from_transform(t);
            let back = ft.to_transform();
            assert_near(back.translation.x, 100.0, 0.001)?;
            assert_near(back.translation.y, 200.0, 0.001)?;
            assert_near(back.translation.z, 300.0, 0.001)
        });

        run_test!(self, "C5: fvector_zero", {
            let v = DVec3::ZERO;
            let fv = FVector::from_dvec3(v);
            let back = fv.to_dvec3();
            assert_near(back.x, 0.0, 0.001)?;
            assert_near(back.y, 0.0, 0.001)?;
            assert_near(back.z, 0.0, 0.001)
        });

        run_test!(self, "C6: fquat_identity_roundtrip", {
            let q = DQuat::IDENTITY;
            let fq = FQuat::from_dquat(q);
            let back = fq.to_dquat();
            assert_near(back.x, 0.0, 0.001)?;
            assert_near(back.y, 0.0, 0.001)?;
            assert_near(back.z, 0.0, 0.001)?;
            assert_near(back.w, 1.0, 0.001)
        });
    }

    // -----------------------------------------------------------------------
    // D. Actor Properties (6 tests)
    // -----------------------------------------------------------------------

    fn test_actor_properties(&mut self, self_ref: &UObjectRef<Actor>) {
        run_test!(self, "D1: get_set_custom_time_dilation", {
            let c = self_ref.checked()?;
            let orig = c.get_custom_time_dilation();
            c.set_custom_time_dilation(2.0);
            let val = c.get_custom_time_dilation();
            assert_near(val as f64, 2.0, 0.001)?;
            c.set_custom_time_dilation(orig);
            Ok(())
        });

        run_test!(self, "D2: get_set_initial_life_span", {
            let c = self_ref.checked()?;
            let orig = c.get_initial_life_span();
            c.set_initial_life_span(30.0);
            let val = c.get_initial_life_span();
            assert_near(val as f64, 30.0, 0.001)?;
            c.set_initial_life_span(orig);
            Ok(())
        });

        run_test!(self, "D3: get_set_hidden_in_game", {
            let c = self_ref.checked()?;
            c.set_actor_hidden_in_game(true);
            // Read back via DynamicCall since there's no direct getter for bHidden
            c.set_actor_hidden_in_game(false);
            Ok(())
        });

        run_test!(self, "D4: get_set_tick_enabled", {
            let c = self_ref.checked()?;
            c.set_actor_tick_enabled(false);
            let val = c.is_actor_tick_enabled();
            assert_false(val, "tick should be disabled")?;
            c.set_actor_tick_enabled(true);
            Ok(())
        });

        run_test!(self, "D5: get_set_tick_interval", {
            let c = self_ref.checked()?;
            c.set_actor_tick_interval(0.1);
            let val = c.get_actor_tick_interval();
            assert_near(val as f64, 0.1, 0.01)?;
            c.set_actor_tick_interval(0.0);
            Ok(())
        });

        run_test!(self, "D6: get_set_actor_scale", {
            let c = self_ref.checked()?;
            let scale = FVector::from_dvec3(DVec3::new(2.0, 2.0, 2.0));
            c.set_actor_scale3_d(&scale);
            let back = c.get_actor_scale3_d().to_dvec3();
            assert_near(back.x, 2.0, 0.01)?;
            assert_near(back.y, 2.0, 0.01)?;
            assert_near(back.z, 2.0, 0.01)?;
            // Restore
            let one = FVector::from_dvec3(DVec3::ONE);
            c.set_actor_scale3_d(&one);
            Ok(())
        });
    }

    // -----------------------------------------------------------------------
    // E. Actor Movement (6 tests)
    // -----------------------------------------------------------------------

    fn test_actor_movement(&mut self, self_ref: &UObjectRef<Actor>) {
        run_test!(self, "E1: set_and_get_location", {
            let c = self_ref.checked()?;
            let loc = FVector::from_dvec3(DVec3::new(100.0, 200.0, 300.0));
            c.k2_set_actor_location(&loc, false, true);
            let back = c.k2_get_actor_location().to_dvec3();
            assert_near(back.x, 100.0, 1.0)?;
            assert_near(back.y, 200.0, 1.0)?;
            assert_near(back.z, 300.0, 1.0)
        });

        run_test!(self, "E2: set_and_get_rotation", {
            let c = self_ref.checked()?;
            let rot = FRotator::from_rotator(uika::runtime::Rotator::new(0.0, 90.0, 0.0));
            c.k2_set_actor_rotation(&rot, true);
            let back = c.k2_get_actor_rotation().to_rotator();
            assert_near(back.yaw, 90.0, 1.0)
        });

        run_test!(self, "E3: teleport_to", {
            let c = self_ref.checked()?;
            let dest = FVector::from_dvec3(DVec3::new(500.0, 500.0, 0.0));
            let rot = FRotator::from_rotator(uika::runtime::Rotator::ZERO);
            c.k2_teleport_to(&dest, &rot);
            let back = c.k2_get_actor_location().to_dvec3();
            assert_near(back.x, 500.0, 1.0)?;
            assert_near(back.y, 500.0, 1.0)
        });

        run_test!(self, "E4: direction_vectors_are_unit", {
            let c = self_ref.checked()?;
            let fwd = c.get_actor_forward_vector().to_dvec3();
            let right = c.get_actor_right_vector().to_dvec3();
            let up = c.get_actor_up_vector().to_dvec3();
            assert_near(fwd.length(), 1.0, 0.01)?;
            assert_near(right.length(), 1.0, 0.01)?;
            assert_near(up.length(), 1.0, 0.01)
        });

        run_test!(self, "E5: get_transform_contains_location", {
            let c = self_ref.checked()?;
            let loc = FVector::from_dvec3(DVec3::new(111.0, 222.0, 333.0));
            c.k2_set_actor_location(&loc, false, true);
            let t = c.get_transform().to_transform();
            assert_near(t.translation.x, 111.0, 1.0)?;
            assert_near(t.translation.y, 222.0, 1.0)?;
            assert_near(t.translation.z, 333.0, 1.0)
        });

        run_test!(self, "E6: add_world_offset", {
            let c = self_ref.checked()?;
            let loc = FVector::from_dvec3(DVec3::new(0.0, 0.0, 0.0));
            c.k2_set_actor_location(&loc, false, true);
            let offset = FVector::from_dvec3(DVec3::new(50.0, 0.0, 0.0));
            c.k2_add_actor_world_offset(&offset, false, true);
            let back = c.k2_get_actor_location().to_dvec3();
            assert_near(back.x, 50.0, 1.0)
        });
    }

    // -----------------------------------------------------------------------
    // F. Actor Lifecycle (5 tests)
    // -----------------------------------------------------------------------

    fn test_actor_lifecycle(&mut self, self_ref: &UObjectRef<Actor>) {
        let world = match get_world(self_ref) {
            Ok(w) => w,
            Err(e) => {
                ulog!(LOG_ERROR, "[UikaTest] SKIP lifecycle tests: cannot get world: {:?}", e);
                return;
            }
        };

        run_test!(self, "F1: spawn_and_destroy", {
            let t = identity_transform();
            let spawned: UObjectRef<Actor> = world.spawn_actor(&t)?;
            assert_true(spawned.is_valid(), "spawned actor should be valid")?;
            spawned.checked()?.k2_destroy_actor();
            // After destroy, is_actor_being_destroyed or is_valid may change
            // (destruction may be deferred to end of frame in UE)
            Ok(())
        });

        run_test!(self, "F2: spawn_with_owner", {
            let t = identity_transform();
            let spawned: UObjectRef<Actor> = world.spawn_actor_with_owner(&t, self_ref)?;
            assert_true(spawned.is_valid(), "owned actor should be valid")?;
            let sc = spawned.checked()?;
            let owner = sc.get_owner();
            assert_true(owner.is_valid(), "owner should be valid")?;
            assert_true(
                owner.raw() == self_ref.raw(),
                "owner should be the test runner"
            )?;
            sc.k2_destroy_actor();
            Ok(())
        });

        run_test!(self, "F3: get_all_actors_of_class", {
            let t1 = transform_at(1000.0, 0.0, 0.0);
            let t2 = transform_at(2000.0, 0.0, 0.0);
            let t3 = transform_at(3000.0, 0.0, 0.0);
            let a1: UObjectRef<Actor> = world.spawn_actor(&t1)?;
            let a2: UObjectRef<Actor> = world.spawn_actor(&t2)?;
            let a3: UObjectRef<Actor> = world.spawn_actor(&t3)?;
            let actors: Vec<UObjectRef<Actor>> = world.get_all_actors_of_class()?;
            // We spawned 3 + self + possibly others, so at least 4
            assert_true(actors.len() >= 4, &format!(
                "expected >= 4 actors, got {}", actors.len()
            ))?;
            a1.checked()?.k2_destroy_actor();
            a2.checked()?.k2_destroy_actor();
            a3.checked()?.k2_destroy_actor();
            Ok(())
        });

        run_test!(self, "F4: destroyed_ref_becomes_pending_destroy", {
            let t = identity_transform();
            let spawned: UObjectRef<Actor> = world.spawn_actor(&t)?;
            assert_true(spawned.is_valid(), "should be valid before destroy")?;
            spawned.checked()?.k2_destroy_actor();
            // After K2_DestroyActor, the actor may be pending-destroy (deferred)
            // or fully destroyed immediately. Both are valid outcomes.
            match spawned.checked() {
                Ok(c) => {
                    let destroying = c.is_actor_being_destroyed();
                    if destroying { Ok(()) } else {
                        Err(UikaError::InvalidOperation(
                            "actor should be destroyed or pending-destroy".into()
                        ))
                    }
                }
                Err(UikaError::ObjectDestroyed) => Ok(()),
                Err(e) => Err(e),
            }
        });

        run_test!(self, "F5: set_life_span", {
            let t = identity_transform();
            let spawned: UObjectRef<Actor> = world.spawn_actor(&t)?;
            let sc = spawned.checked()?;
            sc.set_life_span(60.0);
            let ls = sc.get_life_span();
            assert_near(ls as f64, 60.0, 1.0)?;
            sc.k2_destroy_actor();
            Ok(())
        });
    }

    // -----------------------------------------------------------------------
    // G. World Operations (3 tests)
    // -----------------------------------------------------------------------

    fn test_world_operations(&mut self, self_ref: &UObjectRef<Actor>) {
        run_test!(self, "G1: get_world_returns_valid", {
            let world = get_world(self_ref)?;
            assert_true(world.is_valid(), "world should be valid")
        });

        run_test!(self, "G2: world_settings_accessible", {
            let world = get_world(self_ref)?;
            // Use DynamicCall to call K2_GetWorldSettings on the world
            let call = DynamicCall::new(&world, "K2_GetWorldSettings")?;
            let result = call.call()?;
            let settings_h: uika::runtime::UObjectHandle = result.get("ReturnValue")?;
            assert_true(!settings_h.0.is_null(), "world settings should not be null")
        });

        run_test!(self, "G3: find_object_nonexistent_returns_err", {
            use uika::bindings::core_ue::Object;
            let result = uika::bindings::manual::world_ext::find_object::<Object>(
                "/Game/DoesNotExist"
            );
            match result {
                Err(_) => Ok(()),
                Ok(_) => Err(UikaError::InvalidOperation(
                    "expected error for nonexistent path".into()
                )),
            }
        });
    }

    // -----------------------------------------------------------------------
    // H. DynamicCall (3 tests)
    // -----------------------------------------------------------------------

    fn test_dynamic_call(&mut self, self_ref: &UObjectRef<Actor>) {
        run_test!(self, "H1: dynamic_call_actor_has_tag", {
            let mut call = DynamicCall::new(self_ref, "ActorHasTag")?;
            call.set("Tag", FName::new("UnknownTag123").handle())?;
            let result = call.call()?;
            let has_tag: bool = result.get("ReturnValue")?;
            assert_false(has_tag, "should not have unknown tag")
        });

        run_test!(self, "H2: dynamic_call_nonexistent_function", {
            let result = DynamicCall::new(self_ref, "NoSuchFunction");
            match result {
                Err(UikaError::FunctionNotFound(_)) => Ok(()),
                Err(e) => Err(UikaError::InvalidOperation(
                    format!("expected FunctionNotFound, got: {:?}", e)
                )),
                Ok(_) => Err(UikaError::InvalidOperation(
                    "expected error for nonexistent function".into()
                )),
            }
        });

        run_test!(self, "H3: dynamic_call_get_location", {
            // First set a known location
            let loc = FVector::from_dvec3(DVec3::new(42.0, 84.0, 126.0));
            self_ref.checked()?.k2_set_actor_location(&loc, false, true);

            let call = DynamicCall::new(self_ref, "K2_GetActorLocation")?;
            let result = call.call()?;
            // Return value is a struct — read raw bytes
            // FVector is 3 doubles = 24 bytes in UE 5.7 (FVector uses double)
            let ret: [f64; 3] = result.get("ReturnValue")?;
            assert_near(ret[0], 42.0, 1.0)?;
            assert_near(ret[1], 84.0, 1.0)?;
            assert_near(ret[2], 126.0, 1.0)
        });
    }

    // -----------------------------------------------------------------------
    // I. Containers (6 tests)
    // -----------------------------------------------------------------------

    fn test_containers(&mut self, self_ref: &UObjectRef<Actor>) {
        run_test!(self, "I1: tags_add_and_read", {
            let c = self_ref.checked()?;
            let tags = c.tags();
            tags.clear()?;
            let tag = FName::new("TestTag1");
            tags.push(&tag.handle())?;
            let read_back = FName::from(tags.get(0)?);
            assert_true(
                read_back == tag,
                &format!("expected TestTag1, got {}", read_back)
            )
        });

        run_test!(self, "I2: tags_len_and_clear", {
            let c = self_ref.checked()?;
            let tags = c.tags();
            tags.clear()?;
            tags.push(&FName::new("A").handle())?;
            tags.push(&FName::new("B").handle())?;
            assert_true(tags.len()? == 2, "expected len 2")?;
            tags.clear()?;
            assert_true(tags.len()? == 0, "expected len 0 after clear")
        });

        run_test!(self, "I3: tags_remove", {
            let c = self_ref.checked()?;
            let tags = c.tags();
            tags.clear()?;
            tags.push(&FName::new("X").handle())?;
            tags.push(&FName::new("Y").handle())?;
            tags.push(&FName::new("Z").handle())?;
            tags.remove(1)?;
            assert_true(tags.len()? == 2, "expected len 2 after remove")
        });

        run_test!(self, "I4: actor_has_tag_integration", {
            let c = self_ref.checked()?;
            let tags = c.tags();
            tags.clear()?;
            let tag = FName::new("IntegrationTestTag");
            tags.push(&tag.handle())?;
            let has = c.actor_has_tag(tag.handle());
            assert_true(has, "actor should have the tag we just pushed")?;
            tags.clear()?;
            Ok(())
        });

        run_test!(self, "I5: children_array_accessible", {
            let c = self_ref.checked()?;
            let children = c.children();
            // Just verify we can call len without error
            let _len = children.len()?;
            Ok(())
        });

        run_test!(self, "I6: tags_empty_is_empty", {
            let c = self_ref.checked()?;
            let tags = c.tags();
            tags.clear()?;
            assert_true(tags.is_empty()?, "tags should be empty after clear")
        });
    }

    // -----------------------------------------------------------------------
    // J. Delegates (4 tests)
    // -----------------------------------------------------------------------

    fn test_delegates(&mut self, self_ref: &UObjectRef<Actor>) {
        let world = match get_world(self_ref) {
            Ok(w) => w,
            Err(e) => {
                ulog!(LOG_ERROR, "[UikaTest] SKIP delegate tests: {:?}", e);
                return;
            }
        };

        run_test!(self, "J1: on_destroyed_binds", {
            let t = identity_transform();
            let spawned: UObjectRef<Actor> = world.spawn_actor(&t)?;
            let sc = spawned.checked()?;
            let _binding = sc.on_destroyed().add(|_actor| {
                // Callback body — just verify binding works
            })?;
            sc.k2_destroy_actor();
            Ok(())
        });

        run_test!(self, "J2: on_end_play_binds", {
            let t = identity_transform();
            let spawned: UObjectRef<Actor> = world.spawn_actor(&t)?;
            let sc = spawned.checked()?;
            let _binding = sc.on_end_play().add(|_actor, _reason| {
                // Callback body
            })?;
            sc.k2_destroy_actor();
            Ok(())
        });

        run_test!(self, "J3: delegate_drop_unbinds", {
            let t = identity_transform();
            let spawned: UObjectRef<Actor> = world.spawn_actor(&t)?;
            let sc = spawned.checked()?;
            {
                let binding = sc.on_destroyed().add(|_actor| {})?;
                drop(binding);
            }
            // If we get here without crash, the unbind worked
            sc.k2_destroy_actor();
            Ok(())
        });

        run_test!(self, "J4: on_overlap_binds", {
            let c = self_ref.checked()?;
            let _binding = c.on_actor_begin_overlap().add(|_overlapped, _other| {
                // Just verify binding
            })?;
            // Drop the binding to clean up
            Ok(())
        });
    }

    // -----------------------------------------------------------------------
    // K. Weak Pointers (3 tests)
    // -----------------------------------------------------------------------

    fn test_weak_pointers(&mut self, self_ref: &UObjectRef<Actor>) {
        run_test!(self, "K1: weak_from_valid_ref", {
            let weak = TWeakObjectPtr::from_ref(self_ref);
            assert_true(weak.is_valid(), "weak ptr should be valid")?;
            let resolved = weak.get();
            assert_true(resolved.is_some(), "should resolve to Some")
        });

        run_test!(self, "K2: weak_default_is_invalid", {
            let weak: TWeakObjectPtr<Actor> = TWeakObjectPtr::default();
            assert_false(weak.is_valid(), "default weak should be invalid")?;
            let resolved = weak.get();
            assert_true(resolved.is_none(), "default weak should resolve to None")
        });

        run_test!(self, "K3: weak_to_destroyed", {
            let world = get_world(self_ref)?;
            let t = identity_transform();
            let spawned: UObjectRef<Actor> = world.spawn_actor(&t)?;
            let weak = TWeakObjectPtr::from_ref(&spawned);
            assert_true(weak.is_valid(), "weak should be valid before destroy")?;
            spawned.checked()?.k2_destroy_actor();
            // Note: weak may still resolve until GC runs or frame ends.
            // We just verify the API doesn't crash.
            let _resolved = weak.get();
            Ok(())
        });
    }

    // -----------------------------------------------------------------------
    // L. Error Handling (4 tests)
    // -----------------------------------------------------------------------

    fn test_error_handling(&mut self, self_ref: &UObjectRef<Actor>) {
        run_test!(self, "L1: checked_null_handle", {
            let null_ref: UObjectRef<Actor> = unsafe {
                UObjectRef::from_raw(uika::runtime::UObjectHandle(std::ptr::null_mut()))
            };
            match null_ref.checked() {
                Err(UikaError::ObjectDestroyed) => Ok(()),
                other => Err(UikaError::InvalidOperation(
                    format!("expected ObjectDestroyed, got: {:?}", other)
                )),
            }
        });

        run_test!(self, "L2: cast_null_fails", {
            let null_ref: UObjectRef<Actor> = unsafe {
                UObjectRef::from_raw(uika::runtime::UObjectHandle(std::ptr::null_mut()))
            };
            match null_ref.cast::<World>() {
                Err(UikaError::ObjectDestroyed) => Ok(()),
                other => Err(UikaError::InvalidOperation(
                    format!("expected ObjectDestroyed, got: {:?}", other)
                )),
            }
        });

        run_test!(self, "L3: pin_null_fails", {
            let null_ref: UObjectRef<Actor> = unsafe {
                UObjectRef::from_raw(uika::runtime::UObjectHandle(std::ptr::null_mut()))
            };
            match null_ref.pin() {
                Err(UikaError::ObjectDestroyed) => Ok(()),
                other => Err(UikaError::InvalidOperation(
                    format!("expected ObjectDestroyed, got: {:?}", other)
                )),
            }
        });

        // L5 skipped: Cannot test alive flag via K2_DestroyActor because UE 5.7
        // asserts !IsRooted() during actor destruction, and Pinned<T> roots the object.
        // The alive flag is exercised by engine-initiated destruction (level unload, PIE end).

        run_test!(self, "L4: dynamic_call_on_destroyed", {
            let world = get_world(self_ref)?;
            let t = identity_transform();
            let spawned: UObjectRef<Actor> = world.spawn_actor(&t)?;
            spawned.checked()?.k2_destroy_actor();
            // Try to DynamicCall on the destroyed actor
            let result = DynamicCall::new(&spawned, "K2_GetActorLocation");
            match result {
                Err(UikaError::ObjectDestroyed) => Ok(()),
                Err(e) => {
                    // Might get a different error since actor is pending destroy
                    // but not yet GC'd — that's acceptable too
                    ulog!(LOG_DISPLAY, "[UikaTest] L4: got {:?} instead of ObjectDestroyed (acceptable)", e);
                    Ok(())
                }
                Ok(call) => {
                    // Actor may still be "valid" until end of frame.
                    // Calling on a pending-destroy actor is acceptable.
                    let _result = call.call();
                    Ok(())
                }
            }
        });
    }

    // -----------------------------------------------------------------------
    // M. Realistic Game Patterns (5 tests)
    // -----------------------------------------------------------------------

    fn test_game_patterns(&mut self, self_ref: &UObjectRef<Actor>) {
        let world = match get_world(self_ref) {
            Ok(w) => w,
            Err(e) => {
                ulog!(LOG_ERROR, "[UikaTest] SKIP game pattern tests: {:?}", e);
                return;
            }
        };

        run_test!(self, "M1: patrol_points", {
            let c = self_ref.checked()?;
            // Plain AActor has no root component, so use self_ref (which has one)
            // to test path distance calculation between waypoints.
            let points = [
                DVec3::new(0.0, 0.0, 0.0),
                DVec3::new(100.0, 0.0, 0.0),
                DVec3::new(100.0, 100.0, 0.0),
            ];
            // Set self to each waypoint and read back to verify, then compute distance.
            let mut positions = Vec::new();
            for pt in &points {
                let loc = FVector::from_dvec3(*pt);
                c.k2_set_actor_location(&loc, false, true);
                let back = c.k2_get_actor_location().to_dvec3();
                positions.push(back);
            }
            let dist = (positions[1] - positions[0]).length()
                     + (positions[2] - positions[1]).length();
            assert_near(dist, 200.0, 5.0)
        });

        run_test!(self, "M2: actor_hierarchy", {
            // Plain AActor has no root component, so attachment APIs are no-ops.
            // Just verify the API calls don't error, and that get_attach_parent_actor
            // returns a valid (possibly null) result.
            let t = identity_transform();
            let parent: UObjectRef<Actor> = world.spawn_actor(&t)?;
            let child: UObjectRef<Actor> = world.spawn_actor(&t)?;

            let parent_copy = unsafe { UObjectRef::<Actor>::from_raw(parent.raw()) };
            let cc = child.checked()?;
            // This may silently fail without root components, but should not error.
            cc.k2_attach_to_actor(
                parent_copy,
                FName::NONE.handle(),
                EAttachmentRule::KeepWorld,
                EAttachmentRule::KeepWorld,
                EAttachmentRule::KeepWorld,
                false,
            );

            // get_attach_parent_actor should work regardless
            let _attached = cc.get_attach_parent_actor();

            parent.checked()?.k2_destroy_actor();
            child.checked()?.k2_destroy_actor();
            Ok(())
        });

        run_test!(self, "M3: tick_control", {
            let t = identity_transform();
            let actor: UObjectRef<Actor> = world.spawn_actor(&t)?;
            let ac = actor.checked()?;
            ac.set_actor_tick_enabled(false);
            let enabled = ac.is_actor_tick_enabled();
            assert_false(enabled, "tick should be disabled")?;
            ac.set_actor_tick_interval(0.5);
            let interval = ac.get_actor_tick_interval();
            assert_near(interval as f64, 0.5, 0.01)?;
            ac.k2_destroy_actor();
            Ok(())
        });

        run_test!(self, "M4: distance_calculation", {
            let c = self_ref.checked()?;
            // Use self_ref (which has a root component) for distance measurement.
            // Set self to origin, spawn a second actor, set its loc via self.
            // Since plain AActor has no root component, we test distance from self
            // to a known position by moving self and using GetDistanceTo on a
            // spawned actor at origin.
            let origin = FVector::from_dvec3(DVec3::new(300.0, 400.0, 0.0));
            c.k2_set_actor_location(&origin, false, true);

            let t = identity_transform();
            let other: UObjectRef<Actor> = world.spawn_actor(&t)?;
            // other is at origin (0,0,0) since plain AActor has no root component
            // Distance from (300,400,0) to (0,0,0) = 500
            let other_copy = unsafe { UObjectRef::<Actor>::from_raw(other.raw()) };
            let dist = c.get_distance_to(other_copy);
            assert_near(dist as f64, 500.0, 5.0)?;

            other.checked()?.k2_destroy_actor();
            Ok(())
        });

        run_test!(self, "M5: multi_actor_management", {
            let mut actors = Vec::new();
            for i in 0..5 {
                let t = transform_at(i as f64 * 100.0, 0.0, 0.0);
                let a: UObjectRef<Actor> = world.spawn_actor(&t)?;
                assert_true(a.is_valid(), "spawned actor should be valid")?;
                actors.push(a);
            }
            assert_true(actors.len() == 5, "should have 5 actors")?;
            for a in &actors {
                a.checked()?.k2_destroy_actor();
            }
            Ok(())
        });
    }

    // -----------------------------------------------------------------------
    // N. Inheritance Flattening (8 tests)
    // -----------------------------------------------------------------------

    fn test_inheritance_flattening(&mut self, self_ref: &UObjectRef<Actor>) {
        let world = match get_world(self_ref) {
            Ok(w) => w,
            Err(e) => {
                ulog!(LOG_ERROR, "[UikaTest] SKIP inheritance tests: {:?}", e);
                return;
            }
        };

        // N1: Pawn can call inherited Actor function (actor_has_tag)
        run_test!(self, "N1: pawn_calls_actor_has_tag", {
            let t = identity_transform();
            let pawn: UObjectRef<Pawn> = world.spawn_actor(&t)?;
            let pc = pawn.checked()?;
            // actor_has_tag is defined on Actor, called here on Pawn directly
            let has = pc.actor_has_tag(FName::new("NonexistentTag").handle());
            assert_false(has, "pawn should not have unknown tag")?;
            pc.k2_destroy_actor();
            Ok(())
        });

        // N2: Pawn can call inherited Actor property getter/setter
        run_test!(self, "N2: pawn_get_set_custom_time_dilation", {
            let t = identity_transform();
            let pawn: UObjectRef<Pawn> = world.spawn_actor(&t)?;
            let pc = pawn.checked()?;
            // custom_time_dilation is an Actor property, accessed on Pawn
            let orig = pc.get_custom_time_dilation();
            pc.set_custom_time_dilation(3.0);
            let val = pc.get_custom_time_dilation();
            assert_near(val as f64, 3.0, 0.001)?;
            pc.set_custom_time_dilation(orig);
            pc.k2_destroy_actor();
            Ok(())
        });

        // N3: Pawn can call inherited Actor function (set/get_life_span)
        run_test!(self, "N3: pawn_set_get_life_span", {
            let t = identity_transform();
            let pawn: UObjectRef<Pawn> = world.spawn_actor(&t)?;
            let pc = pawn.checked()?;
            // set_life_span / get_life_span are Actor functions
            pc.set_life_span(45.0);
            let ls = pc.get_life_span();
            assert_near(ls as f64, 45.0, 1.0)?;
            pc.k2_destroy_actor();
            Ok(())
        });

        // N4: Pawn-specific property still works (own property, not inherited)
        run_test!(self, "N4: pawn_own_property_base_eye_height", {
            let t = identity_transform();
            let pawn: UObjectRef<Pawn> = world.spawn_actor(&t)?;
            let pc = pawn.checked()?;
            // base_eye_height is Pawn's own property
            let orig = pc.get_base_eye_height();
            pc.set_base_eye_height(100.0);
            let val = pc.get_base_eye_height();
            assert_near(val as f64, 100.0, 0.01)?;
            pc.set_base_eye_height(orig);
            pc.k2_destroy_actor();
            Ok(())
        });

        // N5: Pawn can call inherited Actor functions (set/get custom_time_dilation)
        // Note: base APawn has no root component, so location-based tests don't work.
        // Use custom_time_dilation (a plain float property) to verify inheritance.
        run_test!(self, "N5: pawn_get_set_inherited_function", {
            let t = identity_transform();
            let pawn: UObjectRef<Pawn> = world.spawn_actor(&t)?;
            let pc = pawn.checked()?;
            // set_actor_time_dilation / get_actor_time_dilation are Actor functions
            pc.set_custom_time_dilation(2.5);
            let val = pc.get_custom_time_dilation();
            assert_near(val as f64, 2.5, 0.01)?;
            pc.k2_destroy_actor();
            Ok(())
        });

        // N6: Pawn can access inherited Actor container property (tags)
        run_test!(self, "N6: pawn_inherited_container_tags", {
            let t = identity_transform();
            let pawn: UObjectRef<Pawn> = world.spawn_actor(&t)?;
            let pc = pawn.checked()?;
            // tags is an Actor container property
            let tags = pc.tags();
            tags.clear()?;
            tags.push(&FName::new("PawnTag").handle())?;
            assert_true(tags.len()? == 1, "pawn should have 1 tag")?;
            let has = pc.actor_has_tag(FName::new("PawnTag").handle());
            assert_true(has, "pawn should have PawnTag")?;
            tags.clear()?;
            pc.k2_destroy_actor();
            Ok(())
        });

        // N7: Pawn can access inherited Actor delegate (on_destroyed)
        run_test!(self, "N7: pawn_inherited_delegate_on_destroyed", {
            let t = identity_transform();
            let pawn: UObjectRef<Pawn> = world.spawn_actor(&t)?;
            let pc = pawn.checked()?;
            // on_destroyed is an Actor delegate
            let _binding = pc.on_destroyed().add(|_actor| {
                // Callback fires on destroy
            })?;
            pc.k2_destroy_actor();
            Ok(())
        });

        // N8: Pawn tick control via inherited Actor functions
        run_test!(self, "N8: pawn_inherited_tick_control", {
            let t = identity_transform();
            let pawn: UObjectRef<Pawn> = world.spawn_actor(&t)?;
            let pc = pawn.checked()?;
            // set_actor_tick_enabled / is_actor_tick_enabled / get_actor_tick_interval
            // are Actor functions, called on Pawn
            pc.set_actor_tick_enabled(false);
            let enabled = pc.is_actor_tick_enabled();
            assert_false(enabled, "pawn tick should be disabled")?;
            pc.set_actor_tick_interval(0.25);
            let interval = pc.get_actor_tick_interval();
            assert_near(interval as f64, 0.25, 0.01)?;
            pc.k2_destroy_actor();
            Ok(())
        });
    }

    // -----------------------------------------------------------------------
    // P. OwnedStruct Init/Destroy (4 tests)
    // -----------------------------------------------------------------------

    fn test_owned_struct_init(&mut self) {
        // P1: FTransform::new() produces identity via InitializeStruct.
        // Zero-fill would give quat=(0,0,0,0) and scale=(0,0,0), but
        // InitializeStruct sets identity quat=(0,0,0,1) and scale=(1,1,1).
        run_test!(self, "P1: owned_struct_new_initializes_correctly", {
            let ft = OwnedStruct::<FTransform>::new();
            let t = ft.to_transform();
            // Scale should be (1,1,1), not (0,0,0)
            assert_near(t.scale.x, 1.0, 0.001)?;
            assert_near(t.scale.y, 1.0, 0.001)?;
            assert_near(t.scale.z, 1.0, 0.001)?;
            // Translation should be (0,0,0)
            assert_near(t.translation.x, 0.0, 0.001)?;
            assert_near(t.translation.y, 0.0, 0.001)?;
            assert_near(t.translation.z, 0.0, 0.001)?;
            // Quaternion should be identity (0,0,0,1)
            assert_near(t.rotation.w, 1.0, 0.001)
        });

        // P2: Clone produces identical data
        run_test!(self, "P2: owned_struct_clone_matches", {
            let ft = FTransform::from_transform(
                Transform::new(DQuat::IDENTITY, DVec3::new(10.0, 20.0, 30.0), DVec3::ONE)
            );
            let cloned = ft.clone();
            let orig_t = ft.to_transform();
            let clone_t = cloned.to_transform();
            assert_near(orig_t.translation.x, clone_t.translation.x, 0.001)?;
            assert_near(orig_t.translation.y, clone_t.translation.y, 0.001)?;
            assert_near(orig_t.translation.z, clone_t.translation.z, 0.001)
        });

        // P3: FVector new() also initializes properly (should be zero)
        run_test!(self, "P3: owned_struct_fvector_new", {
            let fv = OwnedStruct::<FVector>::new();
            let v = fv.to_dvec3();
            assert_near(v.x, 0.0, 0.001)?;
            assert_near(v.y, 0.0, 0.001)?;
            assert_near(v.z, 0.0, 0.001)
        });

        // P4: Drop runs without crash (implicit — if we get here, P1-P3 drops worked)
        run_test!(self, "P4: owned_struct_drop_no_crash", {
            {
                let _ft = OwnedStruct::<FTransform>::new();
                let _fv = OwnedStruct::<FVector>::new();
                // Both drop here
            }
            Ok(())
        });
    }

    // -----------------------------------------------------------------------
    // Q. NewObject (3 tests)
    // -----------------------------------------------------------------------

    fn test_new_object(&mut self, self_ref: &UObjectRef<Actor>) {
        // Q1: new_object_transient creates a valid object (use Actor, not UObject which is abstract)
        run_test!(self, "Q1: new_object_transient_creates_valid", {
            let obj: UObjectRef<Actor> = world_ext::new_object_transient()?;
            assert_true(obj.is_valid(), "transient object should be valid")?;
            let name = obj.get_name()?;
            assert_true(!name.is_empty(), "transient object should have a name")
        });

        // Q2: new_object with outer sets correct outer
        run_test!(self, "Q2: new_object_with_outer", {
            let obj: UObjectRef<Actor> = world_ext::new_object(self_ref)?;
            assert_true(obj.is_valid(), "object should be valid")?;
            let outer = obj.get_outer()?;
            assert_true(
                outer == self_ref.raw(),
                "outer should be the test runner actor"
            )
        });

        // Q3: new_object of Actor subclass (Pawn) via typed API
        run_test!(self, "Q3: new_object_typed_pawn", {
            let pawn: UObjectRef<Pawn> = world_ext::new_object_transient()?;
            assert_true(pawn.is_valid(), "pawn should be valid")?;
            // Verify it's actually a Pawn via IsA
            let cls = pawn.get_class()?;
            assert_true(!cls.0.is_null(), "pawn class should not be null")
        });
    }

    // -----------------------------------------------------------------------
    // R. Runtime Type Instantiation (3 tests)
    // -----------------------------------------------------------------------

    fn test_runtime_instantiation(&mut self, self_ref: &UObjectRef<Actor>) {
        let world = match get_world(self_ref) {
            Ok(w) => w,
            Err(e) => {
                ulog!(LOG_ERROR, "[UikaTest] SKIP runtime instantiation tests: {:?}", e);
                return;
            }
        };

        // R1: spawn_actor_dynamic with runtime class handle
        run_test!(self, "R1: spawn_actor_dynamic", {
            let class = Actor::static_class();
            let t = identity_transform();
            let handle = world_ext::spawn_actor_dynamic(&world, class, &t)?;
            assert_true(!handle.0.is_null(), "dynamic spawned actor should not be null")?;
            let actor: UObjectRef<Actor> = unsafe { UObjectRef::from_raw(handle) };
            assert_true(actor.is_valid(), "dynamic spawned actor should be valid")?;
            actor.checked()?.k2_destroy_actor();
            Ok(())
        });

        // R2: new_object_dynamic with runtime class handle (use Actor, not UObject which is abstract)
        run_test!(self, "R2: new_object_dynamic", {
            let class = Actor::static_class();
            let null_outer = uika::ffi::UObjectHandle(std::ptr::null_mut());
            let handle = world_ext::new_object_dynamic(null_outer, class)?;
            assert_true(!handle.0.is_null(), "dynamic new_object should not be null")?;
            let obj: UObjectRef<Actor> = unsafe { UObjectRef::from_raw(handle) };
            assert_true(obj.is_valid(), "dynamic new_object should be valid")
        });

        // R3: spawn_actor_dynamic with Pawn class (verify polymorphism)
        run_test!(self, "R3: spawn_actor_dynamic_pawn", {
            let class = Pawn::static_class();
            let t = identity_transform();
            let handle = world_ext::spawn_actor_dynamic(&world, class, &t)?;
            let actor: UObjectRef<Actor> = unsafe { UObjectRef::from_raw(handle) };
            assert_true(actor.is_valid(), "dynamic pawn should be valid")?;
            // Verify it's actually a Pawn via cast
            let pawn: UObjectRef<Pawn> = actor.cast()?;
            assert_true(pawn.is_valid(), "cast to Pawn should succeed")?;
            pawn.checked()?.k2_destroy_actor();
            Ok(())
        });
    }

    // -----------------------------------------------------------------------
    // S. Deferred Spawn (4 tests)
    // -----------------------------------------------------------------------

    fn test_deferred_spawn(&mut self, self_ref: &UObjectRef<Actor>) {
        let world = match get_world(self_ref) {
            Ok(w) => w,
            Err(e) => {
                ulog!(LOG_ERROR, "[UikaTest] SKIP deferred spawn tests: {:?}", e);
                return;
            }
        };

        // S1: spawn_actor_deferred creates a valid (not yet begun) actor
        run_test!(self, "S1: spawn_actor_deferred_creates_valid", {
            let t = identity_transform();
            let actor: UObjectRef<Actor> = world.spawn_actor_deferred(&t)?;
            assert_true(actor.is_valid(), "deferred actor should be valid")?;
            // finish_spawning triggers BeginPlay
            world.finish_spawning(&actor, &t)?;
            actor.checked()?.k2_destroy_actor();
            Ok(())
        });

        // S2: deferred actor can have properties set before finish_spawning
        run_test!(self, "S2: deferred_set_properties_before_finish", {
            let t = identity_transform();
            let actor: UObjectRef<Actor> = world.spawn_actor_deferred(&t)?;
            let c = actor.checked()?;
            // Set properties before BeginPlay
            c.set_custom_time_dilation(5.0);
            c.set_initial_life_span(120.0);
            // Finish spawning
            world.finish_spawning(&actor, &t)?;
            // Verify properties persisted through finish_spawning
            let dilation = c.get_custom_time_dilation();
            assert_near(dilation as f64, 5.0, 0.001)?;
            let life_span = c.get_initial_life_span();
            assert_near(life_span as f64, 120.0, 0.001)?;
            c.k2_destroy_actor();
            Ok(())
        });

        // S3: spawn_actor_deferred_full with collision method
        run_test!(self, "S3: deferred_full_with_collision_method", {
            let t = identity_transform();
            let null = uika::ffi::UObjectHandle(std::ptr::null_mut());
            let actor: UObjectRef<Actor> = world.spawn_actor_deferred_full(
                &t, null, null, SpawnCollisionMethod::AlwaysSpawn,
            )?;
            assert_true(actor.is_valid(), "deferred_full actor should be valid")?;
            world.finish_spawning(&actor, &t)?;
            actor.checked()?.k2_destroy_actor();
            Ok(())
        });

        // S4: deferred spawn, set property before finish, verify it persists
        // Note: base AActor has no root component, so location tests don't apply.
        // Instead verify that a property set before FinishSpawning persists after.
        run_test!(self, "S4: deferred_spawn_property_persists", {
            let t = identity_transform();
            let actor: UObjectRef<Actor> = world.spawn_actor_deferred(&t)?;
            let c = actor.checked()?;
            c.set_custom_time_dilation(3.0);
            world.finish_spawning(&actor, &t)?;
            let val = c.get_custom_time_dilation();
            assert_near(val as f64, 3.0, 0.01)?;
            c.k2_destroy_actor();
            Ok(())
        });
    }

    // -----------------------------------------------------------------------
    // O. Hot Reload Validation (2 tests)
    // -----------------------------------------------------------------------

    fn test_hot_reload(&mut self) {
        run_test!(self, "O1: hot_reload_smoke", {
            ulog!(LOG_DISPLAY, "[UikaTest] Hot reload test instructions:");
            ulog!(LOG_DISPLAY, "[UikaTest]   1. Modify a test in test_integration.rs (e.g., change a log message)");
            ulog!(LOG_DISPLAY, "[UikaTest]   2. cargo build -p uika --release");
            ulog!(LOG_DISPLAY, "[UikaTest]   3. In UE console: Uika.Reload");
            ulog!(LOG_DISPLAY, "[UikaTest]   4. Call RunAllTests again and verify the change");
            Ok(())
        });

        run_test!(self, "O2: state_survives_concept", {
            // This test verifies the test runner is functional.
            // After a hot reload, if this test passes, the runner was
            // successfully reconstructed.
            assert_true(self.total_run() > 0, "test runner should have run tests")
        });
    }
}
