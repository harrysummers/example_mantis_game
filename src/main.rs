use mantis::{
    draw_character, draw_crosshair, draw_line_3d, draw_text, draw_weapon, ray_aabb_intersection,
    text_width, text_height, compute_muzzle_world, AmmoState, AssaultRifle, BlockFigure, Bounds, Camera,
    CharacterBody, CharacterController, Engine, Game, Input, OtsCameraConfig, ProjectileConfig,
    ProjectileManager, Vec3, Weapon, compute_ots_camera,
};

const LIME_GREEN: u32 = 0x00FF00;
const MOUSE_SENSITIVITY: f32 = 0.003;
const MOVE_SPEED: f32 = 0.15;
const SPRINT_MULTIPLIER: f32 = 2.0;
const JUMP_FORCE: f32 = 0.3;
const GRAVITY: f32 = 0.015;
const CROUCH_OFFSET: f32 = 0.5;
const CHARACTER_HEIGHT: f32 = 3.0;

// Room dimensions
const ROOM_W: f32 = 40.0;
const ROOM_H: f32 = 10.0;
const ROOM_D: f32 = 40.0;

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
            color: 0xFF0000,
            length: 0.8,
            splash_color: 0xFFFF00,
            splash_duration: 30,
            splash_size: 0.5,
        });

        let rifle = AssaultRifle::new(0xFFFFFF);
        let ammo = AmmoState::new(rifle.magazine_size(), rifle.reload_time());

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
        }
    }
}

impl Game for MyGame {
    fn update(&mut self, input: &Input) {
        self.body.yaw += input.mouse_dx * MOUSE_SENSITIVITY;
        self.camera_pitch += input.mouse_dy * MOUSE_SENSITIVITY;
        self.camera_pitch = self.camera_pitch.clamp(
            self.camera_config.pitch_min,
            self.camera_config.pitch_max,
        );

        let floor_y = -ROOM_H / 2.0;
        let ceiling_y = ROOM_H / 2.0;
        self.controller.update(&mut self.body, input, floor_y, ceiling_y);

        self.bounds.clamp(&mut self.body.position);

        let eff_height = self.controller.effective_height(self.body.height);
        compute_ots_camera(
            &self.body,
            eff_height,
            self.camera_pitch,
            &self.camera_config,
            &mut self.camera,
        );

        // Compute aim pitch and aim point
        let ray_origin = self.camera.position;
        let ray_dir = self.camera.direction();

        let crouch_factor = self.controller.crouch_factor();
        let upper_leg_len = self.body.height * 0.22;
        let hip_drop = upper_leg_len * crouch_factor;
        let shoulder_y = self.body.height * 0.78 - hip_drop;
        let hip_y = self.body.height * 0.45 - hip_drop;
        let character_yaw_offset = -std::f32::consts::FRAC_PI_2;

        self.aim_point = ray_aabb_intersection(ray_origin, ray_dir, self.room_min, self.room_max);

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

        // Fire projectile while mouse held and ammo available
        if input.mouse_left_down && self.ammo.can_fire() {
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

        // Tick ammo cooldown and reload timer
        self.ammo.tick();

        // Update projectiles
        self.projectiles.update(self.room_min, self.room_max);
    }

    fn render(&mut self, buffer: &mut Vec<u32>, width: usize, height: usize) {
        // Draw room wireframe
        for &(a, b) in &self.edges {
            draw_line_3d(
                buffer,
                width,
                height,
                &self.camera,
                self.vertices[a],
                self.vertices[b],
                LIME_GREEN,
            );
        }

        let character_yaw_offset = -std::f32::consts::FRAC_PI_2;
        let mut arm_pose = self.rifle.arm_pose();

        // Apply reload animation: right arm bobs up and down twice
        if self.ammo.reloading {
            let t = self.ammo.reload_progress();
            let offset = (t * 2.0 * std::f32::consts::PI).sin().abs() * 0.4;
            arm_pose.right_upper_pitch -= offset;
        }

        // Draw character model
        draw_character(
            buffer,
            width,
            height,
            &self.camera,
            &self.body,
            self.controller.crouch_factor(),
            character_yaw_offset,
            Some(&arm_pose),
            self.aim_pitch,
            self.controller.walk_cycle(),
            &self.model,
        );

        // Draw weapon
        let crouch_factor = self.controller.crouch_factor();
        let upper_leg_len = self.body.height * 0.22;
        let hip_drop = upper_leg_len * crouch_factor;
        let shoulder_y = self.body.height * 0.78 - hip_drop;
        let hip_y = self.body.height * 0.45 - hip_drop;
        draw_weapon(
            buffer,
            width,
            height,
            &self.camera,
            &self.body,
            shoulder_y,
            hip_y,
            character_yaw_offset,
            self.aim_pitch,
            &self.rifle,
        );

        // Draw projectiles
        self.projectiles.draw(buffer, width, height, &self.camera);

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
    }
}

fn main() {
    let mut engine = Engine::new("Example Mantis Game");
    let aspect = engine.width() as f32 / engine.height() as f32;
    let mut game = MyGame::new(aspect);
    engine.run(&mut game);
}
