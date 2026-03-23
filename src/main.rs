use mantis::{
    draw_character, draw_crosshair, draw_line_3d, draw_weapon, AssaultRifle, BlockFigure,
    Bounds, Camera, CharacterBody, CharacterController, Engine, Game, Input, OtsCameraConfig,
    Vec3, Weapon, compute_ots_camera,
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
    model: BlockFigure,
    rifle: AssaultRifle,
    vertices: [Vec3; 8],
    edges: [(usize, usize); 12],
    bounds: Bounds,
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

        MyGame {
            camera,
            body,
            controller,
            camera_config,
            camera_pitch: 0.15,
            model: BlockFigure::new(0xFFFFFF),
            rifle: AssaultRifle::new(0xFFFFFF),
            vertices,
            edges,
            bounds,
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
        let arm_pose = self.rifle.arm_pose();

        // Draw character model with arms bent to hold weapon
        draw_character(
            buffer,
            width,
            height,
            &self.camera,
            &self.body,
            self.controller.crouch_factor(),
            character_yaw_offset,
            Some(&arm_pose),
            &self.model,
        );

        // Draw weapon
        let crouch_factor = self.controller.crouch_factor();
        let upper_leg_len = self.body.height * 0.22;
        let hip_drop = upper_leg_len * crouch_factor;
        let shoulder_y = self.body.height * 0.78 - hip_drop;
        draw_weapon(
            buffer,
            width,
            height,
            &self.camera,
            &self.body,
            shoulder_y,
            character_yaw_offset,
            &self.rifle,
        );

        // Crosshair
        draw_crosshair(buffer, width, height, 10, 0xFFFFFF);
    }
}

fn main() {
    let mut engine = Engine::new("Example Mantis Game");
    let aspect = engine.width() as f32 / engine.height() as f32;
    let mut game = MyGame::new(aspect);
    engine.run(&mut game);
}
