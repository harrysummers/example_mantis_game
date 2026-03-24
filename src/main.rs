use mantis::{
    draw_button, draw_character, draw_character_filled, draw_character_filled_tilted, draw_crosshair, draw_filled_quad_3d,
    draw_line, draw_line_3d, draw_text, draw_weapon, draw_weapon_filled, draw_weapon_filled_ex, draw_weapon_filled_tilted,
    ray_aabb_intersection, text_width, text_height, compute_muzzle_world, AmmoState, AssaultRifle,
    BlockFigure, Bounds, Camera, CharacterBody, CharacterController, Engine, Game, Input,
    OtsCameraConfig, ProjectileConfig, ProjectileManager, Vec3, Weapon, compute_ots_camera_bounded,
};

const LIME_GREEN: u32 = 0x00FF00;
const MOUSE_SENSITIVITY: f32 = 0.003;
const MOVE_SPEED: f32 = 0.15;
const SPRINT_MULTIPLIER: f32 = 2.0;
const JUMP_FORCE: f32 = 0.38;
const GRAVITY: f32 = 0.015;
const CROUCH_OFFSET: f32 = 0.5;
const CHARACTER_HEIGHT: f32 = 3.0;

// Room dimensions
const ROOM_W: f32 = 40.0;
const ROOM_H: f32 = 10.0;
const ROOM_D: f32 = 40.0;

const ORANGE: u32 = 0xFF8800;
const BODY_DAMAGE: i32 = 20;
const HEAD_DAMAGE: i32 = 80;
const ENEMY_DAMAGE: i32 = 10;
const ENEMY_HP: i32 = 100;
const PLAYER_HP: i32 = 500;
const HIT_FLASH_FRAMES: u32 = 10;
const COUNTDOWN_SECONDS: u32 = 3;
const COUNTDOWN_FRAMES: u32 = COUNTDOWN_SECONDS * 60;

const MELEE_DAMAGE: i32 = 50;
const MELEE_RANGE: f32 = 3.5; // max distance for melee hit
const MELEE_FRAMES: u32 = 18; // total animation frames
const MELEE_COOLDOWN: u32 = 10; // cooldown after swing before another

const CRATE_SIZE: f32 = 1.5; // half-extent of cube crate
const CRATE_HEIGHT: f32 = 2.4; // tall enough to hide crouching body, head visible when standing
const CRATE_COLOR: u32 = 0x4A2A12; // dark opaque brown
const NUM_CRATES: usize = 2;

const HEALTH_PICKUP_AMOUNT: i32 = 100;
const HEALTH_SPAWN_MIN: u32 = 30 * 60; // 30 seconds
const HEALTH_SPAWN_MAX: u32 = 60 * 60; // 60 seconds
const HEALTH_PICKUP_RADIUS: f32 = 1.0;
const HEAL_FLASH_FRAMES: u32 = 10;
const GREEN: u32 = 0x00FF00;

const RESPAWN_FRAMES: u32 = 120; // ~2 seconds at 60fps
const AI_MIN_FRAMES: u32 = 12;  // 0.2s at 60fps
const AI_MAX_FRAMES: u32 = 120; // 2.0s at 60fps

#[derive(Clone, Copy)]
struct Crate {
    position: Vec3, // center of base on the floor
}

impl Crate {
    fn aabb(&self) -> (Vec3, Vec3) {
        (
            Vec3::new(self.position.x - CRATE_SIZE, self.position.y, self.position.z - CRATE_SIZE),
            Vec3::new(self.position.x + CRATE_SIZE, self.position.y + CRATE_HEIGHT, self.position.z + CRATE_SIZE),
        )
    }
}

#[derive(Clone, Copy)]
struct EnemyAction {
    shooting: bool,
    crouching: bool,
    walking: bool,
    sprinting: bool,
    turning: bool,
    melee: bool,
}

const DEATH_FALL_FRAMES: u32 = 30;  // frames to fall backward
const DEATH_LAY_FRAMES: u32 = 60;   // frames to lay on ground before disappearing
const DEATH_TOTAL_FRAMES: u32 = DEATH_FALL_FRAMES + DEATH_LAY_FRAMES;

struct Enemy {
    body: CharacterBody,
    controller: CharacterController,
    hp: i32,
    max_hp: i32,
    alive: bool,
    dying: bool,        // playing death animation
    death_anim: u32,    // counts up from 0 to DEATH_TOTAL_FRAMES
    death_timer: u32,   // respawn countdown (starts after death anim)
    action: EnemyAction,
    action_timer: u32,
    turn_rate: f32,
    aim_pitch: f32,
    ammo: AmmoState,
    heal_flash: u32,
    melee_timer: u32,
    melee_cooldown: u32,
    melee_hit: bool,
}

impl Enemy {
    fn eff_height(&self) -> f32 {
        self.controller.effective_height(self.body.height)
    }

    fn body_aabb(&self) -> (Vec3, Vec3) {
        let hw = self.body.height * 0.18;
        let hd = self.body.height * 0.18;
        let h = self.eff_height();
        let neck_y = self.body.position.y + h * 0.84;
        (
            Vec3::new(self.body.position.x - hw, self.body.position.y, self.body.position.z - hd),
            Vec3::new(self.body.position.x + hw, neck_y, self.body.position.z + hd),
        )
    }

    fn head_aabb(&self) -> (Vec3, Vec3) {
        let r = self.body.height * 0.1;
        let h = self.eff_height();
        let neck_y = self.body.position.y + h * 0.84;
        let top_y = self.body.position.y + h;
        (
            Vec3::new(self.body.position.x - r, neck_y, self.body.position.z - r),
            Vec3::new(self.body.position.x + r, top_y, self.body.position.z + r),
        )
    }

    fn full_aabb(&self) -> (Vec3, Vec3) {
        let hw = self.body.height * 0.18;
        let hd = self.body.height * 0.18;
        let h = self.eff_height();
        (
            Vec3::new(self.body.position.x - hw, self.body.position.y, self.body.position.z - hd),
            Vec3::new(self.body.position.x + hw, self.body.position.y + h, self.body.position.z + hd),
        )
    }
}

struct HealthPickup {
    position: Vec3,
    active: bool,
    rotation: f32, // current spin angle
}

struct MyGame {
    camera: Camera,
    body: CharacterBody,
    controller: CharacterController,
    camera_config: OtsCameraConfig,
    camera_pitch: f32,
    aim_pitch: f32,
    aim_point: Option<Vec3>,
    model: BlockFigure,
    rifle: AssaultRifle,
    ammo: AmmoState,
    projectiles: ProjectileManager,
    vertices: [Vec3; 8],
    edges: [(usize, usize); 12],
    bounds: Bounds,
    room_min: Vec3,
    room_max: Vec3,
    enemies: Vec<Enemy>,
    enemy_model: BlockFigure,
    enemy_rifle: AssaultRifle,
    player_hp: i32,
    player_max_hp: i32,
    score: u32,
    spawn_seed: u32,
    enemy_projectiles: ProjectileManager,
    hit_flash: u32,
    game_over: bool,
    started: bool,
    dev_mode: bool,
    cursor_shown: bool,
    aspect: f32,
    last_mouse_x: f32,
    last_mouse_y: f32,
    last_mouse_click: bool,
    last_shift: bool,
    countdown: u32,
    hit_model: BlockFigure,
    zbuf: Vec<f32>,
    crates: Vec<Crate>,
    health_pickup: HealthPickup,
    health_spawn_timer: u32,
    player_heal_flash: u32,
    melee_timer: u32,
    melee_cooldown: u32,
    melee_hit: bool,
    player_dying: bool,
    player_death_anim: u32,
}

impl MyGame {
    fn new(aspect: f32) -> Self {
        let hw = ROOM_W / 2.0;
        let hh = ROOM_H / 2.0;
        let hd = ROOM_D / 2.0;

        let vertices = [
            Vec3::new(-hw, -hh, -hd),
            Vec3::new( hw, -hh, -hd),
            Vec3::new( hw,  hh, -hd),
            Vec3::new(-hw,  hh, -hd),
            Vec3::new(-hw, -hh,  hd),
            Vec3::new( hw, -hh,  hd),
            Vec3::new( hw,  hh,  hd),
            Vec3::new(-hw,  hh,  hd),
        ];

        let edges = [
            (0, 1), (1, 2), (2, 3), (3, 0),
            (4, 5), (5, 6), (6, 7), (7, 4),
            (0, 4), (1, 5), (2, 6), (3, 7),
        ];

        let camera = Camera::new(Vec3::new(0.0, 0.0, 0.0), aspect);

        let floor_y = -hh;
        let body = CharacterBody::new(Vec3::new(0.0, floor_y, 0.0), CHARACTER_HEIGHT);

        let controller = CharacterController::new(
            MOVE_SPEED,
            SPRINT_MULTIPLIER,
            JUMP_FORCE,
            GRAVITY,
            CROUCH_OFFSET,
        );

        let camera_config = OtsCameraConfig {
            distance: 3.0,
            height_offset: 1.8,
            shoulder_offset: 2.5,
            look_at_height: 0.75,
            pitch_min: -1.2,
            pitch_max: 1.2,
        };

        let margin = 0.5;
        let bounds = Bounds::new(
            Vec3::new(-hw + margin, f32::NEG_INFINITY, -hd + margin),
            Vec3::new( hw - margin, f32::INFINITY,      hd - margin),
        );

        let room_min = Vec3::new(-hw, -hh, -hd);
        let room_max = Vec3::new(hw, hh, hd);

        let projectiles = ProjectileManager::new(ProjectileConfig {
            speed: 1.5,
            color: 0xFFFFFF,
            length: 0.8,
            splash_color: 0xFFFF00,
            splash_duration: 30,
            splash_size: 0.5,
        });

        let rifle = AssaultRifle::new(0xFFFFFF);
        let ammo = AmmoState::new(rifle.magazine_size(), rifle.reload_time());

        let enemies = Vec::new();

        let enemy_projectiles = ProjectileManager::new(ProjectileConfig {
            speed: 1.5,
            color: ORANGE,
            length: 0.8,
            splash_color: 0xFF4400,
            splash_duration: 30,
            splash_size: 0.5,
        });

        MyGame {
            camera,
            body,
            controller,
            camera_config,
            camera_pitch: 0.15,
            aim_pitch: 0.0,
            aim_point: None,
            model: BlockFigure::new(0xFFFFFF),
            rifle,
            ammo,
            projectiles,
            vertices,
            edges,
            bounds,
            room_min,
            room_max,
            enemies,
            enemy_model: BlockFigure::new(ORANGE),
            enemy_rifle: AssaultRifle::new(ORANGE),
            player_hp: PLAYER_HP,
            player_max_hp: PLAYER_HP,
            score: 0,
            spawn_seed: 12345,
            enemy_projectiles,
            hit_flash: 0,
            game_over: false,
            started: false,
            dev_mode: false,
            cursor_shown: true,
            aspect,
            last_mouse_x: 0.0,
            last_mouse_y: 0.0,
            last_mouse_click: false,
            last_shift: false,
            countdown: 0,
            hit_model: BlockFigure::new(0xFF0000),
            zbuf: Vec::new(),
            crates: Vec::new(),
            health_pickup: HealthPickup {
                position: Vec3::new(0.0, 0.0, 0.0),
                active: false,
                rotation: 0.0,
            },
            health_spawn_timer: 0,
            player_heal_flash: 0,
            melee_timer: 0,
            melee_cooldown: 0,
            melee_hit: false,
            player_dying: false,
            player_death_anim: 0,
        }
    }
    fn start_game(&mut self) {
        let floor_y = -ROOM_H / 2.0;
        let margin = 2.0;
        let spawn_hw = ROOM_W / 2.0 - margin;
        let spawn_hd = ROOM_D / 2.0 - margin;

        // Spawn crates (non-overlapping, away from player spawn)
        self.crates.clear();
        for _ in 0..NUM_CRATES {
            loop {
                let rx = self.rand_f32() * 2.0 - 1.0;
                let rz = self.rand_f32() * 2.0 - 1.0;
                let pos = Vec3::new(rx * (spawn_hw - CRATE_SIZE), floor_y, rz * (spawn_hd - CRATE_SIZE));

                // Check not too close to player spawn (0,0)
                let dist_player = (pos.x * pos.x + pos.z * pos.z).sqrt();
                if dist_player < 5.0 { continue; }

                // Check not overlapping other crates
                let mut overlaps = false;
                for existing in &self.crates {
                    let dx = (pos.x - existing.position.x).abs();
                    let dz = (pos.z - existing.position.z).abs();
                    if dx < CRATE_SIZE * 3.0 && dz < CRATE_SIZE * 3.0 {
                        overlaps = true;
                        break;
                    }
                }
                if overlaps { continue; }

                self.crates.push(Crate { position: pos });
                break;
            }
        }

        let enemy_rifle_ref = AssaultRifle::new(ORANGE);

        for _ in 0..2 {
            let rx = self.rand_f32() * 2.0 - 1.0;
            let rz = self.rand_f32() * 2.0 - 1.0;
            let mut body = CharacterBody::new(
                Vec3::new(rx * spawn_hw, floor_y, rz * spawn_hd), CHARACTER_HEIGHT,
            );
            body.yaw = (-body.position.x).atan2(-body.position.z);
            self.enemies.push(Enemy {
                body,
                controller: CharacterController::new(MOVE_SPEED, SPRINT_MULTIPLIER, JUMP_FORCE, GRAVITY, CROUCH_OFFSET),
                hp: ENEMY_HP, max_hp: ENEMY_HP, alive: true, dying: false, death_anim: 0, death_timer: 0,
                action: EnemyAction { shooting: false, crouching: false, walking: false, sprinting: false, turning: false, melee: false },
                action_timer: 30, turn_rate: 0.0, aim_pitch: 0.0,
                ammo: AmmoState::new(enemy_rifle_ref.magazine_size(), enemy_rifle_ref.reload_time()),
                heal_flash: 0,
                melee_timer: 0, melee_cooldown: 0, melee_hit: false,
            });
        }

        // Spawn first health pickup immediately
        self.health_spawn_timer = 0;
        self.health_pickup.active = false;

        self.started = true;
        self.countdown = if self.dev_mode { 0 } else { COUNTDOWN_FRAMES };
        self.cursor_shown = false;
        Engine::hide_cursor();
    }

    fn reset(&mut self) {
        let mut new_game = MyGame::new(self.aspect);
        new_game.spawn_seed = self.spawn_seed;
        *self = new_game;
        self.start_game();
    }

    fn rand(&mut self) -> u32 {
        self.spawn_seed ^= self.spawn_seed << 13;
        self.spawn_seed ^= self.spawn_seed >> 17;
        self.spawn_seed ^= self.spawn_seed << 5;
        self.spawn_seed
    }

    /// Returns a float in [0.0, 1.0)
    fn rand_f32(&mut self) -> f32 {
        self.rand() as f32 / u32::MAX as f32
    }

    fn random_action(&mut self) -> (EnemyAction, u32, f32) {
        let duration = AI_MIN_FRAMES + (self.rand_f32() * (AI_MAX_FRAMES - AI_MIN_FRAMES) as f32) as u32;
        let turn_rate = (self.rand_f32() - 0.5) * 0.06;

        // 12 equally likely combos (includes melee)
        let action = match self.rand() % 12 {
            0  => EnemyAction { shooting: false, crouching: false, walking: false, sprinting: false, turning: false, melee: false }, // standing
            1  => EnemyAction { shooting: false, crouching: true,  walking: false, sprinting: false, turning: false, melee: false }, // crouching
            2  => EnemyAction { shooting: false, crouching: false, walking: false, sprinting: true,  turning: false, melee: false }, // sprinting
            3  => EnemyAction { shooting: false, crouching: false, walking: true,  sprinting: false, turning: false, melee: false }, // walking
            4  => EnemyAction { shooting: false, crouching: false, walking: false, sprinting: false, turning: true,  melee: false }, // turning
            5  => EnemyAction { shooting: false, crouching: false, walking: true,  sprinting: false, turning: true,  melee: false }, // turning + walking
            6  => EnemyAction { shooting: false, crouching: false, walking: false, sprinting: true,  turning: true,  melee: false }, // turning + sprinting
            7  => EnemyAction { shooting: true,  crouching: false, walking: false, sprinting: false, turning: false, melee: false }, // standing + firing
            8  => EnemyAction { shooting: true,  crouching: true,  walking: false, sprinting: false, turning: false, melee: false }, // firing + crouching
            9  => EnemyAction { shooting: true,  crouching: false, walking: true,  sprinting: false, turning: false, melee: false }, // firing + walking
            10 => EnemyAction { shooting: true,  crouching: false, walking: false, sprinting: true,  turning: false, melee: false }, // firing + sprinting
            _  => EnemyAction { shooting: false, crouching: false, walking: false, sprinting: false, turning: false, melee: true  }, // melee
        };

        let turn_rate = if action.shooting || action.melee { 0.0 } else { turn_rate };
        (action, duration, turn_rate)
    }
}

impl Game for MyGame {
    fn update(&mut self, input: &Input) {
        self.last_mouse_x = input.mouse_x;
        self.last_mouse_y = input.mouse_y;
        self.last_mouse_click = input.mouse_left_click;
        self.last_shift = input.key_shift;

        // Start screen: wait for button click (handled in render)
        if !self.started {
            return;
        }

        // Game over: show cursor, wait for play again
        if self.game_over {
            if !self.cursor_shown {
                Engine::show_cursor();
                self.cursor_shown = true;
            }
            return;
        }

        // Player death animation
        if self.player_dying {
            self.player_death_anim += 1;
            if self.player_death_anim >= DEATH_TOTAL_FRAMES {
                self.game_over = true;
            }
            return; // no input while dying
        }

        // Countdown: tick down, allow camera movement but no gameplay
        if self.countdown > 0 {
            self.countdown -= 1;
            // Allow looking around during countdown
            self.body.yaw += input.mouse_dx * MOUSE_SENSITIVITY;
            self.camera_pitch += input.mouse_dy * MOUSE_SENSITIVITY;
            self.camera_pitch = self.camera_pitch.clamp(
                self.camera_config.pitch_min,
                self.camera_config.pitch_max,
            );
            let eff_height = self.controller.effective_height(self.body.height);
            compute_ots_camera_bounded(
                &self.body, eff_height, self.camera_pitch,
                &self.camera_config, &mut self.camera,
                Some(self.room_min), Some(self.room_max),
            );
            return;
        }

        self.body.yaw += input.mouse_dx * MOUSE_SENSITIVITY;
        self.camera_pitch += input.mouse_dy * MOUSE_SENSITIVITY;
        self.camera_pitch = self.camera_pitch.clamp(
            self.camera_config.pitch_min,
            self.camera_config.pitch_max,
        );

        let base_floor_y = -ROOM_H / 2.0;
        let ceiling_y = ROOM_H / 2.0;

        // Compute effective floor: check if player is above a crate
        let mut floor_y = base_floor_y;
        for cr in &self.crates {
            let (cmin, cmax) = cr.aabb();
            let margin = CHARACTER_HEIGHT * 0.2; // same as collision radius
            if self.body.position.x + margin > cmin.x && self.body.position.x - margin < cmax.x
                && self.body.position.z + margin > cmin.z && self.body.position.z - margin < cmax.z
                && self.body.position.y >= cmax.y - 0.1 // feet are at or above crate top
            {
                floor_y = floor_y.max(cmax.y);
            }
        }

        self.controller.update(&mut self.body, input, floor_y, ceiling_y);

        self.bounds.clamp(&mut self.body.position);

        // Tick hit flash
        if self.hit_flash > 0 { self.hit_flash -= 1; }

        // Capture player target for enemy aiming
        let player_pos = self.body.position;
        let player_eff_h = self.controller.effective_height(self.body.height);
        let player_center = Vec3::new(player_pos.x, player_pos.y + player_eff_h * 0.5, player_pos.z);
        let player_head = Vec3::new(player_pos.x, player_pos.y + player_eff_h * 0.92, player_pos.z);
        let character_yaw_offset = -std::f32::consts::FRAC_PI_2;

        // Precompute crate AABBs for line-of-sight checks
        let crate_aabbs: Vec<(Vec3, Vec3)> = self.crates.iter().map(|c| c.aabb()).collect();

        // Update enemy AI
        let mut new_actions: Vec<Option<(EnemyAction, u32, f32)>> = Vec::new();
        for enemy in &self.enemies {
            if enemy.alive && enemy.action_timer == 0 {
                new_actions.push(Some((EnemyAction { shooting: false, crouching: false, walking: false, sprinting: false, turning: false, melee: false }, 0, 0.0)));
            } else {
                new_actions.push(None);
            }
        }
        for item in &mut new_actions {
            if item.is_some() {
                *item = Some(self.random_action());
            }
        }
        for (i, enemy) in self.enemies.iter_mut().enumerate() {
            if !enemy.alive { continue; }
            if enemy.dying { continue; } // skip AI for dying enemies

            if let Some(Some((action, duration, turn_rate))) = new_actions.get(i) {
                enemy.action = *action;
                enemy.action_timer = *duration;
                enemy.turn_rate = *turn_rate;
            }

            if enemy.action_timer > 0 {
                enemy.action_timer -= 1;
            }

            enemy.ammo.tick();

            let act = enemy.action;

            // Shooting: face player, check line-of-sight through crates, aim, fire
            if act.shooting && !self.dev_mode && enemy.melee_timer == 0 {
                let dx = player_center.x - enemy.body.position.x;
                let dz = player_center.z - enemy.body.position.z;
                enemy.body.yaw = dz.atan2(dx);

                let e_crouch = enemy.controller.crouch_factor();
                let e_upper_leg = enemy.body.height * 0.22;
                let e_hip_drop = e_upper_leg * e_crouch;
                let e_shoulder_y = enemy.body.height * 0.78 - e_hip_drop;
                let e_hip_y = enemy.body.height * 0.45 - e_hip_drop;

                // Check line-of-sight: try body center first, then head
                let eye_pos = Vec3::new(
                    enemy.body.position.x,
                    enemy.body.position.y + enemy.controller.effective_height(enemy.body.height) * 0.8,
                    enemy.body.position.z,
                );

                let can_see = |target: Vec3| -> bool {
                    let dir = target.sub(eye_pos);
                    let dist_sq = dir.x * dir.x + dir.y * dir.y + dir.z * dir.z;
                    let dist = dist_sq.sqrt();
                    if dist < 0.01 { return true; }
                    let dir_n = dir.scale(1.0 / dist);
                    for (cmin, cmax) in &crate_aabbs {
                        if let Some(hit) = ray_aabb_intersection(eye_pos, dir_n, *cmin, *cmax) {
                            let hit_dist = {
                                let d = hit.sub(eye_pos);
                                (d.x * d.x + d.y * d.y + d.z * d.z).sqrt()
                            };
                            if hit_dist < dist {
                                return false; // crate is between enemy and target
                            }
                        }
                    }
                    true
                };

                // Pick best visible target
                let aim_target = if can_see(player_center) {
                    Some(player_center)
                } else if can_see(player_head) {
                    Some(player_head)
                } else {
                    None
                };

                if let Some(target) = aim_target {
                    let rotation = enemy.body.yaw + character_yaw_offset;
                    let relative = target.sub(enemy.body.position);
                    let local_aim = relative.rotate_y(-rotation);
                    let muzzle_local = self.enemy_rifle.muzzle_position(enemy.body.height, e_shoulder_y);
                    let dz_aim = local_aim.z - muzzle_local.z;
                    let dy_aim = local_aim.y - muzzle_local.y;
                    if dz_aim.abs() > 0.01 {
                        enemy.aim_pitch = -(dy_aim / dz_aim).atan();
                    }

                    if enemy.ammo.can_fire() {
                        let muzzle_world = compute_muzzle_world(
                            &enemy.body, &self.enemy_rifle,
                            e_shoulder_y, e_hip_y,
                            character_yaw_offset, enemy.aim_pitch,
                        );
                        self.enemy_projectiles.fire(muzzle_world, target);
                        enemy.ammo.fire(self.enemy_rifle.fire_interval());
                    }
                } else {
                    enemy.aim_pitch = 0.0; // can't see player, don't fire
                }
            } else {
                enemy.aim_pitch = 0.0;
            }

            // Turning (only when not shooting — shooting faces player instead)
            if act.turning && !act.shooting {
                enemy.body.yaw += enemy.turn_rate;
            }

            // Build fake input
            let fake_input = Input {
                mouse_dx: 0.0, mouse_dy: 0.0, mouse_x: 0.0, mouse_y: 0.0,
                mouse_left_click: false, mouse_left_down: false,
                key_w: act.walking || act.sprinting,
                key_a: false, key_s: false, key_d: false,
                key_space: false,
                key_shift: act.sprinting,
                key_ctrl: act.crouching,
                key_r: false,
                key_f: false,
            };

            enemy.controller.update(&mut enemy.body, &fake_input, base_floor_y, ceiling_y);
            self.bounds.clamp(&mut enemy.body.position);

            // Enemy melee: if melee action and near player, start a swing
            if act.melee && !self.dev_mode {
                let dx = self.body.position.x - enemy.body.position.x;
                let dz = self.body.position.z - enemy.body.position.z;
                let dist = (dx * dx + dz * dz).sqrt();
                // Face the player when doing melee
                enemy.body.yaw = dz.atan2(dx);
                if dist <= MELEE_RANGE && enemy.melee_timer == 0 && enemy.melee_cooldown == 0 {
                    enemy.melee_timer = MELEE_FRAMES;
                    enemy.melee_hit = false;
                }
            }
            // Tick enemy melee
            if enemy.melee_timer > 0 {
                enemy.melee_timer -= 1;
                // Deal damage at the midpoint of the swing
                if enemy.melee_timer == MELEE_FRAMES / 2 && !enemy.melee_hit {
                    let dx = self.body.position.x - enemy.body.position.x;
                    let dz = self.body.position.z - enemy.body.position.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist <= MELEE_RANGE {
                        self.player_hp -= MELEE_DAMAGE;
                        self.hit_flash = HIT_FLASH_FRAMES;
                        enemy.melee_hit = true;
                        if self.player_hp <= 0 && !self.player_dying {
                            self.player_hp = 0;
                            self.player_dying = true;
                            self.player_death_anim = 0;
                        }
                    }
                }
                if enemy.melee_timer == 0 {
                    enemy.melee_cooldown = MELEE_COOLDOWN;
                }
            }
            if enemy.melee_cooldown > 0 {
                enemy.melee_cooldown -= 1;
            }
        }

        // Character-to-character collision separation
        let collision_radius = CHARACTER_HEIGHT * 0.2;
        // Player vs enemies
        for enemy in &mut self.enemies {
            if !enemy.alive { continue; }
            let dx = self.body.position.x - enemy.body.position.x;
            let dz = self.body.position.z - enemy.body.position.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let min_dist = collision_radius * 2.0;
            if dist < min_dist && dist > 0.001 {
                let overlap = (min_dist - dist) * 0.5;
                let nx = dx / dist;
                let nz = dz / dist;
                self.body.position.x += nx * overlap;
                self.body.position.z += nz * overlap;
                enemy.body.position.x -= nx * overlap;
                enemy.body.position.z -= nz * overlap;
            }
        }
        // Enemy vs enemy
        let enemy_count = self.enemies.len();
        for i in 0..enemy_count {
            for j in (i + 1)..enemy_count {
                if !self.enemies[i].alive || !self.enemies[j].alive { continue; }
                let dx = self.enemies[i].body.position.x - self.enemies[j].body.position.x;
                let dz = self.enemies[i].body.position.z - self.enemies[j].body.position.z;
                let dist = (dx * dx + dz * dz).sqrt();
                let min_dist = collision_radius * 2.0;
                if dist < min_dist && dist > 0.001 {
                    let overlap = (min_dist - dist) * 0.5;
                    let nx = dx / dist;
                    let nz = dz / dist;
                    self.enemies[i].body.position.x += nx * overlap;
                    self.enemies[i].body.position.z += nz * overlap;
                    self.enemies[j].body.position.x -= nx * overlap;
                    self.enemies[j].body.position.z -= nz * overlap;
                }
            }
        }
        // Character-to-crate collision (push characters out of crate AABBs)
        // Only push in XZ when feet are below the crate top (not standing/jumping on top)
        // For player: also check a forward-projected point to account for crouch lean
        let player_crouch = self.controller.crouch_factor();
        let lean_forward = player_crouch * CHARACTER_HEIGHT * 0.3; // how far torso extends forward
        let character_yaw_offset_col = -std::f32::consts::FRAC_PI_2;
        let player_facing_yaw = self.body.yaw + character_yaw_offset_col;
        // The torso leans in the character's local +Z which maps to this world direction
        let lean_dx = lean_forward * player_facing_yaw.sin();
        let lean_dz = lean_forward * player_facing_yaw.cos();

        for cr in &self.crates {
            let (cmin, cmax) = cr.aabb();
            // Player: check both feet position and lean tip against crate
            if self.body.position.y < cmax.y - 0.1 {
                // Check two points: feet and torso tip
                let check_points = [
                    (0.0f32, 0.0f32),          // feet
                    (lean_dx, lean_dz),        // lean tip
                ];
                for &(ox, oz) in &check_points {
                    let px = self.body.position.x + ox;
                    let pz = self.body.position.z + oz;
                    let char_r = collision_radius;
                    if px + char_r > cmin.x && px - char_r < cmax.x
                        && pz + char_r > cmin.z && pz - char_r < cmax.z
                    {
                        let push_xp = cmax.x + char_r - px;
                        let push_xn = px - (cmin.x - char_r);
                        let push_zp = cmax.z + char_r - pz;
                        let push_zn = pz - (cmin.z - char_r);
                        let min_push = push_xp.min(push_xn).min(push_zp).min(push_zn);
                        // Push the feet (not the check point) so the whole body moves
                        if min_push == push_xp { self.body.position.x = cmax.x + char_r - ox; }
                        else if min_push == push_xn { self.body.position.x = cmin.x - char_r - ox; }
                        else if min_push == push_zp { self.body.position.z = cmax.z + char_r - oz; }
                        else { self.body.position.z = cmin.z - char_r - oz; }
                        break; // re-check on next frame
                    }
                }
            }
            // Enemies with standard radius
            let mut push_out = |pos: &mut Vec3| {
                if pos.y >= cmax.y - 0.1 { return; }
                let char_r = collision_radius;
                if pos.x + char_r > cmin.x && pos.x - char_r < cmax.x
                    && pos.z + char_r > cmin.z && pos.z - char_r < cmax.z
                {
                    let push_xp = cmax.x + char_r - pos.x;
                    let push_xn = pos.x - (cmin.x - char_r);
                    let push_zp = cmax.z + char_r - pos.z;
                    let push_zn = pos.z - (cmin.z - char_r);
                    let min_push = push_xp.min(push_xn).min(push_zp).min(push_zn);
                    if min_push == push_xp { pos.x = cmax.x + char_r; }
                    else if min_push == push_xn { pos.x = cmin.x - char_r; }
                    else if min_push == push_zp { pos.z = cmax.z + char_r; }
                    else { pos.z = cmin.z - char_r; }
                }
            };
            for enemy in &mut self.enemies {
                if enemy.alive { push_out(&mut enemy.body.position); }
            }
        }

        // Re-clamp all after separation
        self.bounds.clamp(&mut self.body.position);
        for enemy in &mut self.enemies {
            self.bounds.clamp(&mut enemy.body.position);
        }

        // Tick heal flashes
        if self.player_heal_flash > 0 { self.player_heal_flash -= 1; }
        for enemy in &mut self.enemies {
            if enemy.heal_flash > 0 { enemy.heal_flash -= 1; }
        }

        // Health pickup spawning and collection
        if !self.health_pickup.active {
            if self.health_spawn_timer > 0 {
                self.health_spawn_timer -= 1;
            } else {
                // Spawn health pickup at random location
                let margin = 3.0;
                let hw = ROOM_W / 2.0 - margin;
                let hd = ROOM_D / 2.0 - margin;
                let floor_y = -ROOM_H / 2.0;
                let rx = self.rand_f32() * 2.0 - 1.0;
                let rz = self.rand_f32() * 2.0 - 1.0;
                self.health_pickup.position = Vec3::new(rx * hw, floor_y + 1.5, rz * hd);
                self.health_pickup.active = true;
                self.health_pickup.rotation = 0.0;
            }
        }

        if self.health_pickup.active {
            self.health_pickup.rotation += 0.03; // slow spin

            // Check player pickup
            let dx = self.body.position.x - self.health_pickup.position.x;
            let dz = self.body.position.z - self.health_pickup.position.z;
            if (dx * dx + dz * dz).sqrt() < HEALTH_PICKUP_RADIUS {
                self.player_hp = (self.player_hp + HEALTH_PICKUP_AMOUNT).min(self.player_max_hp);
                self.player_heal_flash = HEAL_FLASH_FRAMES;
                self.health_pickup.active = false;
                self.health_spawn_timer = HEALTH_SPAWN_MIN + (self.rand_f32() * (HEALTH_SPAWN_MAX - HEALTH_SPAWN_MIN) as f32) as u32;
            }

            // Check enemy pickup
            if self.health_pickup.active {
                for enemy in &mut self.enemies {
                    if !enemy.alive { continue; }
                    let dx = enemy.body.position.x - self.health_pickup.position.x;
                    let dz = enemy.body.position.z - self.health_pickup.position.z;
                    if (dx * dx + dz * dz).sqrt() < HEALTH_PICKUP_RADIUS {
                        enemy.hp = (enemy.hp + HEALTH_PICKUP_AMOUNT).min(enemy.max_hp);
                        enemy.heal_flash = HEAL_FLASH_FRAMES;
                        self.health_pickup.active = false;
                        self.health_spawn_timer = HEALTH_SPAWN_MIN + (self.rand_f32() * (HEALTH_SPAWN_MAX - HEALTH_SPAWN_MIN) as f32) as u32;
                        break;
                    }
                }
            }
        }

        let eff_height = self.controller.effective_height(self.body.height);
        let ideal_dir = compute_ots_camera_bounded(
            &self.body,
            eff_height,
            self.camera_pitch,
            &self.camera_config,
            &mut self.camera,
            Some(self.room_min),
            Some(self.room_max),
        );

        // Compute aim pitch and aim point
        // Use the actual camera position for raycasting (so we hit what's on screen)
        // but use the ideal (unclamped) direction for aim_pitch so the character
        // doesn't look up/down when the camera is pushed by wall collision.
        let ray_origin = self.camera.position;
        let ray_dir = ideal_dir;

        let crouch_factor = self.controller.crouch_factor();
        let upper_leg_len = self.body.height * 0.22;
        let hip_drop = upper_leg_len * crouch_factor;
        let shoulder_y = self.body.height * 0.78 - hip_drop;
        let hip_y = self.body.height * 0.45 - hip_drop;
        let character_yaw_offset = -std::f32::consts::FRAC_PI_2;

        // Find closest aim target: check enemies first, then room walls
        self.aim_point = ray_aabb_intersection(ray_origin, ray_dir, self.room_min, self.room_max);
        for enemy in &self.enemies {
            if !enemy.alive || enemy.dying { continue; }
            let (amin, amax) = enemy.full_aabb();
            if let Some(hit) = ray_aabb_intersection(ray_origin, ray_dir, amin, amax) {
                let dist_hit = {
                    let d = hit.sub(ray_origin);
                    d.x * d.x + d.y * d.y + d.z * d.z
                };
                let dist_aim = if let Some(ap) = self.aim_point {
                    let d = ap.sub(ray_origin);
                    d.x * d.x + d.y * d.y + d.z * d.z
                } else {
                    f32::INFINITY
                };
                if dist_hit < dist_aim {
                    self.aim_point = Some(hit);
                }
            }
        }

        if let Some(aim_world) = self.aim_point {
            let rotation = self.body.yaw + character_yaw_offset;
            let relative = aim_world.sub(self.body.position);
            let local_aim = relative.rotate_y(-rotation);

            let muzzle = self.rifle.muzzle_position(self.body.height, shoulder_y);

            let dz = local_aim.z - muzzle.z;
            let dy = local_aim.y - muzzle.y;
            if dz.abs() > 0.01 {
                self.aim_pitch = -(dy / dz).atan();
            }
        }

        // Reload on R key
        if input.key_r {
            self.ammo.start_reload();
        }

        // Fire projectile while mouse held and ammo available (not during melee)
        if input.mouse_left_down && self.ammo.can_fire() && self.melee_timer == 0 {
            if let Some(aim_world) = self.aim_point {
                let muzzle_world = compute_muzzle_world(
                    &self.body,
                    &self.rifle,
                    shoulder_y,
                    hip_y,
                    character_yaw_offset,
                    self.aim_pitch,
                );
                self.projectiles.fire(muzzle_world, aim_world);
                self.ammo.fire(self.rifle.fire_interval());
            }
        }

        // Player melee attack on F key
        if input.key_f && self.melee_timer == 0 && self.melee_cooldown == 0 {
            self.melee_timer = MELEE_FRAMES;
            self.melee_hit = false;
        }
        if self.melee_timer > 0 {
            self.melee_timer -= 1;
            // Deal damage at the midpoint of the swing
            if self.melee_timer == MELEE_FRAMES / 2 && !self.melee_hit {
                // Check each enemy in front of the player and within range
                let facing_x = self.body.yaw.cos();
                let facing_z = self.body.yaw.sin();
                for enemy in &mut self.enemies {
                    if !enemy.alive || enemy.dying { continue; }
                    let dx = enemy.body.position.x - self.body.position.x;
                    let dz = enemy.body.position.z - self.body.position.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > MELEE_RANGE { continue; }
                    // Check if enemy is roughly in front (dot product > 0)
                    let dot = dx * facing_x + dz * facing_z;
                    if dot > 0.0 {
                        enemy.hp -= MELEE_DAMAGE;
                        self.melee_hit = true;
                        if enemy.hp <= 0 && !enemy.dying {
                            enemy.hp = 0;
                            enemy.dying = true;
                            enemy.death_anim = 0;
                            self.score += 1;
                        }
                        break; // only hit one enemy per swing
                    }
                }
            }
            if self.melee_timer == 0 {
                self.melee_cooldown = MELEE_COOLDOWN;
            }
        }
        if self.melee_cooldown > 0 {
            self.melee_cooldown -= 1;
        }

        // Tick ammo cooldown and reload timer
        self.ammo.tick();

        // Update projectiles
        self.projectiles.update(self.room_min, self.room_max);

        // Projectiles vs crates (player projectiles)
        let crate_targets: Vec<(Vec3, Vec3)> = self.crates.iter().map(|c| c.aabb()).collect();
        self.projectiles.check_hits(&crate_targets); // just kill them, no damage

        // Check headshots first, then body shots
        let dead_aabb = |e: &Enemy| { let p = e.body.position; (p, p) };

        let head_targets: Vec<(Vec3, Vec3)> = self.enemies.iter()
            .map(|e| if e.alive && !e.dying { e.head_aabb() } else { dead_aabb(e) })
            .collect();
        let head_hits = self.projectiles.check_hits_styled(&head_targets, Some(0xFF0000), None);
        for hit in &head_hits {
            if let Some(enemy) = self.enemies.get_mut(hit.target_index) {
                if enemy.alive && !enemy.dying {
                    enemy.hp -= HEAD_DAMAGE;
                    if enemy.hp <= 0 {
                        enemy.dying = true;
                        enemy.death_anim = 0;
                        self.score += 1;
                    }
                }
            }
        }

        let body_targets: Vec<(Vec3, Vec3)> = self.enemies.iter()
            .map(|e| if e.alive && !e.dying { e.body_aabb() } else { dead_aabb(e) })
            .collect();
        let body_hits = self.projectiles.check_hits_styled(&body_targets, Some(0xFF0000), Some(0.25));
        for hit in &body_hits {
            if let Some(enemy) = self.enemies.get_mut(hit.target_index) {
                if enemy.alive && !enemy.dying {
                    enemy.hp -= BODY_DAMAGE;
                    if enemy.hp <= 0 {
                        enemy.dying = true;
                        enemy.death_anim = 0;
                        self.score += 1;
                    }
                }
            }
        }

        // Update enemy projectiles and check hits against player
        self.enemy_projectiles.update(self.room_min, self.room_max);
        self.enemy_projectiles.check_hits(&crate_targets); // enemy bullets stop on crates too
        {
            let player_hw = self.body.height * 0.18;
            let player_h = self.controller.effective_height(self.body.height);
            let p = self.body.position;
            let player_target = vec![(
                Vec3::new(p.x - player_hw, p.y, p.z - player_hw),
                Vec3::new(p.x + player_hw, p.y + player_h, p.z + player_hw),
            )];
            let hits = self.enemy_projectiles.check_hits(&player_target);
            for _ in &hits {
                self.player_hp -= ENEMY_DAMAGE;
                self.hit_flash = HIT_FLASH_FRAMES;
                if self.player_hp <= 0 && !self.player_dying {
                    self.player_hp = 0;
                    self.player_dying = true;
                    self.player_death_anim = 0;
                }
            }
        }

        // Respawn dead enemies after timer expires
        let margin = 2.0;
        let spawn_hw = ROOM_W / 2.0 - margin;
        let spawn_hd = ROOM_D / 2.0 - margin;
        let floor_y = -ROOM_H / 2.0;
        // Pre-generate random values to avoid borrow conflict
        let spawn_randoms: Vec<(f32, f32)> = (0..self.enemies.len())
            .map(|_| (self.rand_f32() * 2.0 - 1.0, self.rand_f32() * 2.0 - 1.0))
            .collect();
        // Tick enemy death animations
        for enemy in self.enemies.iter_mut() {
            if enemy.dying && enemy.alive {
                enemy.death_anim += 1;
                if enemy.death_anim >= DEATH_TOTAL_FRAMES {
                    enemy.alive = false;
                    enemy.dying = false;
                    enemy.death_anim = 0;
                    enemy.death_timer = 120; // 2 seconds before respawn
                }
            }
        }

        for (i, enemy) in self.enemies.iter_mut().enumerate() {
            if !enemy.alive {
                if enemy.death_timer > 0 {
                    enemy.death_timer -= 1;
                } else {
                    let (rx, rz) = spawn_randoms[i];
                    enemy.body.position = Vec3::new(rx * spawn_hw, floor_y, rz * spawn_hd);
                    enemy.body.yaw = (-enemy.body.position.x).atan2(-enemy.body.position.z);
                    enemy.hp = enemy.max_hp;
                    enemy.alive = true;
                    enemy.dying = false;
                    enemy.death_anim = 0;
                    enemy.controller = CharacterController::new(
                        MOVE_SPEED, SPRINT_MULTIPLIER, JUMP_FORCE, GRAVITY, CROUCH_OFFSET,
                    );
                    enemy.action = EnemyAction { shooting: false, crouching: false, walking: false, sprinting: false, turning: false, melee: false };
                    enemy.action_timer = 30;
                    enemy.turn_rate = 0.0;
                    enemy.aim_pitch = 0.0;
                    enemy.ammo = AmmoState::new(self.enemy_rifle.magazine_size(), self.enemy_rifle.reload_time());
                    enemy.heal_flash = 0;
                    enemy.melee_timer = 0;
                    enemy.melee_cooldown = 0;
                    enemy.melee_hit = false;
                }
            }
        }
    }

    fn render(&mut self, buffer: &mut Vec<u32>, width: usize, height: usize) {
        // Initialize/clear z-buffer
        let buf_size = width * height;
        if self.zbuf.len() != buf_size {
            self.zbuf.resize(buf_size, 0.0f32);
        }
        self.zbuf.fill(0.0f32);

        let character_yaw_offset = -std::f32::consts::FRAC_PI_2;

        // === FILLED SURFACES (z-buffered) ===

        // Room surfaces
        let v = &self.vertices;
        draw_filled_quad_3d(buffer, &mut self.zbuf, width, height, &self.camera,
            v[0], v[1], v[5], v[4], 0xC4A46C); // floor (light brown)
        draw_filled_quad_3d(buffer, &mut self.zbuf, width, height, &self.camera,
            v[3], v[2], v[6], v[7], 0x555555); // ceiling (dark gray)
        draw_filled_quad_3d(buffer, &mut self.zbuf, width, height, &self.camera,
            v[0], v[1], v[2], v[3], 0x888888); // wall -Z
        draw_filled_quad_3d(buffer, &mut self.zbuf, width, height, &self.camera,
            v[5], v[4], v[7], v[6], 0x888888); // wall +Z
        draw_filled_quad_3d(buffer, &mut self.zbuf, width, height, &self.camera,
            v[4], v[0], v[3], v[7], 0x888888); // wall -X
        draw_filled_quad_3d(buffer, &mut self.zbuf, width, height, &self.camera,
            v[1], v[5], v[6], v[2], 0x888888); // wall +X

        // Crates (filled + wireframe edges)
        for cr in &self.crates {
            let (cmin, cmax) = cr.aabb();
            let c = [
                Vec3::new(cmin.x, cmin.y, cmin.z), Vec3::new(cmax.x, cmin.y, cmin.z),
                Vec3::new(cmax.x, cmax.y, cmin.z), Vec3::new(cmin.x, cmax.y, cmin.z),
                Vec3::new(cmin.x, cmin.y, cmax.z), Vec3::new(cmax.x, cmin.y, cmax.z),
                Vec3::new(cmax.x, cmax.y, cmax.z), Vec3::new(cmin.x, cmax.y, cmax.z),
            ];
            // Filled faces
            draw_filled_quad_3d(buffer, &mut self.zbuf, width, height, &self.camera, c[0], c[1], c[2], c[3], CRATE_COLOR);
            draw_filled_quad_3d(buffer, &mut self.zbuf, width, height, &self.camera, c[5], c[4], c[7], c[6], CRATE_COLOR);
            draw_filled_quad_3d(buffer, &mut self.zbuf, width, height, &self.camera, c[3], c[2], c[6], c[7], CRATE_COLOR);
            draw_filled_quad_3d(buffer, &mut self.zbuf, width, height, &self.camera, c[4], c[5], c[1], c[0], CRATE_COLOR);
            draw_filled_quad_3d(buffer, &mut self.zbuf, width, height, &self.camera, c[4], c[0], c[3], c[7], CRATE_COLOR);
            draw_filled_quad_3d(buffer, &mut self.zbuf, width, height, &self.camera, c[1], c[5], c[6], c[2], CRATE_COLOR);
        }

        // Health pickup (spinning solid green heart)
        if self.health_pickup.active {
            let hp = self.health_pickup.position;
            let rot = self.health_pickup.rotation;
            let size = 0.4;
            let num_pts = 24;
            // Heart shape in 2D (local X, Y)
            let heart_points: Vec<(f32, f32)> = (0..=num_pts).map(|i| {
                let t = std::f32::consts::TAU * i as f32 / num_pts as f32;
                let x = size * t.sin().powi(3);
                let y = size * (13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos()) / 16.0;
                (x, y)
            }).collect();

            // Draw two filled heart planes (0 and 90 degrees) as triangle fans
            let cos_r = rot.cos();
            let sin_r = rot.sin();
            for offset in [0.0f32, std::f32::consts::FRAC_PI_2] {
                let cr = (rot + offset).cos();
                let sr = (rot + offset).sin();
                let center = hp;
                for i in 0..num_pts {
                    let (x0, y0) = heart_points[i];
                    let (x1, y1) = heart_points[i + 1];
                    let v0 = Vec3::new(hp.x + x0 * cr, hp.y + y0, hp.z + x0 * sr);
                    let v1 = Vec3::new(hp.x + x1 * cr, hp.y + y1, hp.z + x1 * sr);
                    // Filled triangle from center to edge
                    draw_filled_quad_3d(buffer, &mut self.zbuf, width, height, &self.camera,
                        center, v0, v1, center, GREEN);
                    // Outline edge
                    draw_line_3d(buffer, width, height, &self.camera, v0, v1, 0x00AA00);
                }
            }
        }

        // Player filled
        let mut arm_pose = self.rifle.arm_pose();

        // Melee swing animation: progress 0.0 (start) -> 1.0 (end)
        let player_melee_progress = if self.melee_timer > 0 {
            1.0 - (self.melee_timer as f32 / MELEE_FRAMES as f32)
        } else {
            -1.0 // no melee
        };

        let (weapon_pitch, weapon_roll) = if player_melee_progress >= 0.0 {
            // Swing animation: quickly pitch up (muzzle toward ceiling) and roll 90° (vertical rifle)
            // Swing follows an arc: start at aim, swing up-left, return
            let t = player_melee_progress;
            let swing = if t < 0.5 {
                t * 2.0 // 0..1 going up
            } else {
                (1.0 - t) * 2.0 // 1..0 coming back
            };
            // Raise arms along with the weapon swing
            arm_pose.left_upper_pitch += swing * 1.2;
            arm_pose.right_upper_pitch += swing * 1.2;
            arm_pose.left_lower_pitch -= swing * 0.3; // straighten elbows a bit
            arm_pose.right_lower_pitch -= swing * 0.3;
            let pitch = self.aim_pitch - swing * 1.2; // swing upward
            let roll = swing * std::f32::consts::FRAC_PI_2; // rotate 90° to vertical
            (pitch, roll)
        } else if self.ammo.reloading {
            (self.aim_pitch + std::f32::consts::FRAC_PI_4, 0.0)
        } else {
            (self.aim_pitch, 0.0)
        };

        let player_fill = if self.hit_flash > 0 { 0xFF0000 } else if self.player_heal_flash > 0 { GREEN } else { 0xFFFFFF };
        // Player death tilt
        let player_death_tilt = if self.player_dying || self.game_over {
            let t = (self.player_death_anim as f32 / DEATH_FALL_FRAMES as f32).min(1.0);
            t * std::f32::consts::FRAC_PI_2
        } else {
            0.0
        };

        draw_character_filled_tilted(
            buffer, &mut self.zbuf, width, height, &self.camera,
            &self.body, self.controller.crouch_factor(), character_yaw_offset,
            Some(&arm_pose), self.aim_pitch, self.controller.walk_cycle(),
            &self.model, player_fill, player_death_tilt,
        );

        let crouch_factor = self.controller.crouch_factor();
        let upper_leg_len = self.body.height * 0.22;
        let hip_drop = upper_leg_len * crouch_factor;
        let shoulder_y = self.body.height * 0.78 - hip_drop;
        let hip_y = self.body.height * 0.45 - hip_drop;
        draw_weapon_filled_tilted(
            buffer, &mut self.zbuf, width, height, &self.camera,
            &self.body, shoulder_y, hip_y, character_yaw_offset, weapon_pitch,
            weapon_roll, &self.rifle, 0x111111, player_death_tilt,
        );

        // Enemy filled
        let base_enemy_arm_pose = self.enemy_rifle.arm_pose();
        for enemy in &self.enemies {
            if !enemy.alive { continue; }
            let e_crouch = enemy.controller.crouch_factor();
            let e_walk = enemy.controller.walk_cycle();

            // Compute enemy melee swing factor
            let e_swing = if enemy.melee_timer > 0 {
                let t = 1.0 - (enemy.melee_timer as f32 / MELEE_FRAMES as f32);
                if t < 0.5 { t * 2.0 } else { (1.0 - t) * 2.0 }
            } else {
                0.0
            };

            // Modify arm pose for melee swing
            let mut e_arm_pose = base_enemy_arm_pose;
            if e_swing > 0.0 {
                e_arm_pose.left_upper_pitch += e_swing * 1.2;
                e_arm_pose.right_upper_pitch += e_swing * 1.2;
                e_arm_pose.left_lower_pitch -= e_swing * 0.3;
                e_arm_pose.right_lower_pitch -= e_swing * 0.3;
            }

            // Enemy death tilt
            let e_death_tilt = if enemy.dying {
                let t = (enemy.death_anim as f32 / DEATH_FALL_FRAMES as f32).min(1.0);
                t * std::f32::consts::FRAC_PI_2
            } else {
                0.0
            };

            let enemy_fill = if enemy.heal_flash > 0 { GREEN } else { 0x111111 };
            draw_character_filled_tilted(
                buffer, &mut self.zbuf, width, height, &self.camera,
                &enemy.body, e_crouch, character_yaw_offset,
                Some(&e_arm_pose), enemy.aim_pitch, e_walk,
                &self.enemy_model, enemy_fill, e_death_tilt,
            );

            let e_upper_leg = enemy.body.height * 0.22;
            let e_hip_drop = e_upper_leg * e_crouch;
            let e_shoulder_y = enemy.body.height * 0.78 - e_hip_drop;
            let e_hip_y = enemy.body.height * 0.45 - e_hip_drop;

            // Enemy melee swing animation for weapon
            let (e_wpitch, e_wroll) = if e_swing > 0.0 {
                (enemy.aim_pitch - e_swing * 1.2, e_swing * std::f32::consts::FRAC_PI_2)
            } else {
                (enemy.aim_pitch, 0.0)
            };
            draw_weapon_filled_tilted(
                buffer, &mut self.zbuf, width, height, &self.camera,
                &enemy.body, e_shoulder_y, e_hip_y,
                character_yaw_offset, e_wpitch,
                e_wroll, &self.enemy_rifle, 0x111111, e_death_tilt,
            );
        }

        // === WIREFRAME (on top, no z-test) ===

        // (Player wireframe removed — solid fill only)

        // Enemy health bars
        for enemy in &self.enemies {
            if !enemy.alive || enemy.dying { continue; }
            let bar_world = Vec3::new(
                enemy.body.position.x,
                enemy.body.position.y + enemy.body.height + 0.5,
                enemy.body.position.z,
            );
            if let Some((sx, sy)) = self.camera.project_point(bar_world, width, height) {
                let bar_w: i32 = 40;
                let bar_h: i32 = 4;
                let bx = sx as i32 - bar_w / 2;
                let by = sy as i32;
                let fill = (enemy.hp as f32 / enemy.max_hp as f32 * bar_w as f32) as i32;

                for dy in 0..bar_h {
                    draw_line(buffer, width, height, bx, by + dy, bx + bar_w, by + dy, 0x663300);
                }
                if fill > 0 {
                    for dy in 0..bar_h {
                        draw_line(buffer, width, height, bx, by + dy, bx + fill, by + dy, ORANGE);
                    }
                }
            }
        }

        // Draw projectiles (player and enemy)
        self.projectiles.draw(buffer, width, height, &self.camera);
        self.enemy_projectiles.draw(buffer, width, height, &self.camera);

        // Score HUD at top-right
        {
            let score_text = format!("Score: {}", self.score);
            let s_scale = 2;
            let s_padding = 20;
            let s_tw = text_width(&score_text, s_scale);
            let s_x = width - s_tw - s_padding;
            draw_text(buffer, width, height, &score_text, s_x, s_padding, 0xFFFFFF, s_scale);
        }

        // Crosshair
        draw_crosshair(buffer, width, height, 10, 0xFFFFFF);

        // Ammo HUD at bottom-right (2x scale)
        let ammo_text = format!("{}/{}", self.ammo.ammo, self.ammo.magazine_size);
        let scale = 2;
        let padding = 20;
        let tw = text_width(&ammo_text, scale);
        let th = text_height(scale);
        let x = width - tw - padding;
        let y = height - th - padding;
        draw_text(buffer, width, height, &ammo_text, x, y, 0xFFFFFF, scale);

        // Player health bar below ammo text (segmented per 100 HP)
        let segments = (self.player_max_hp / 100) as i32;
        let seg_w: i32 = 20; // pixels per 100 HP segment
        let seg_gap: i32 = 2;
        let hp_bar_h: i32 = 8;
        let total_bar_w = segments * seg_w + (segments - 1) * seg_gap;
        let hp_bar_x = width as i32 - total_bar_w - padding as i32;
        let hp_bar_y = (y + th + 8) as i32;

        for s in 0..segments {
            let seg_hp_start = s * 100;
            let sx = hp_bar_x + s * (seg_w + seg_gap);

            // Background
            for dy in 0..hp_bar_h {
                draw_line(buffer, width, height, sx, hp_bar_y + dy, sx + seg_w, hp_bar_y + dy, 0x333333);
            }

            // Fill proportional to HP in this segment
            if self.player_hp > seg_hp_start {
                let seg_fill_frac = ((self.player_hp - seg_hp_start) as f32 / 100.0).min(1.0);
                let fill_px = (seg_fill_frac * seg_w as f32) as i32;
                if fill_px > 0 {
                    for dy in 0..hp_bar_h {
                        draw_line(buffer, width, height, sx, hp_bar_y + dy, sx + fill_px, hp_bar_y + dy, 0xFFFFFF);
                    }
                }
            }
        }

        // Start screen
        if !self.started {
            let title = "Mantis Shooter";
            let title_scale = 6;
            let title_tw = text_width(title, title_scale);
            let title_th = text_height(title_scale);
            let title_x = (width - title_tw) / 2;
            let title_y = height / 2 - title_th - 40;
            draw_text(buffer, width, height, title, title_x, title_y, 0xFFFFFF, title_scale);

            let btn_cx = width / 2;
            let btn_cy = height / 2 + 20;
            let result = draw_button(
                buffer, width, height,
                "Start Game", btn_cx, btn_cy,
                0xFFFFFF, 4, 20,
                self.last_mouse_x, self.last_mouse_y, self.last_mouse_click,
            );
            if result.clicked {
                self.dev_mode = self.last_shift;
                self.start_game();
            }
            return;
        }

        // Countdown display
        if self.countdown > 0 {
            let secs_left = (self.countdown + 59) / 60; // ceiling division
            let cd_text = format!("{}", secs_left);
            let cd_scale = 10;
            let cd_tw = text_width(&cd_text, cd_scale);
            let cd_th = text_height(cd_scale);
            let cd_x = (width - cd_tw) / 2;
            let cd_y = (height - cd_th) / 2;
            draw_text(buffer, width, height, &cd_text, cd_x, cd_y, 0xFFFFFF, cd_scale);
        }

        // Game over screen
        if self.game_over {
            let go_scale = 6;
            let go_text = "GAME OVER";
            let go_tw = text_width(go_text, go_scale);
            let go_th = text_height(go_scale);
            let go_x = (width - go_tw) / 2;
            let go_y = (height - go_th) / 2;
            draw_text(buffer, width, height, go_text, go_x, go_y, 0xFF0000, go_scale);

            let btn_cx = width / 2;
            let btn_cy = go_y + go_th + 40;
            let result = draw_button(
                buffer, width, height,
                "Play Again", btn_cx, btn_cy,
                0xFF0000, 3, 16,
                self.last_mouse_x, self.last_mouse_y, self.last_mouse_click,
            );
            if result.clicked {
                self.reset();
            }
        }
    }
}

fn main() {
    let mut engine = Engine::new("Example Mantis Game");
    let aspect = engine.width() as f32 / engine.height() as f32;
    let mut game = MyGame::new(aspect);
    Engine::show_cursor(); // show cursor for start screen
    engine.run(&mut game);
}
